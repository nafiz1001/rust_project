use std::{
    io::{self, BufRead, Write},
    mem::size_of,
};

#[cfg(target_os = "windows")]
use windows::{MemoryRegionIterator, Process, ProcessIterator};

#[cfg(target_os = "linux")]
use linux::{MemoryRegionIterator, Process, ProcessIterator};

// struct Address {
//     address: usize,
//     memory_type: MemoryType,
//     memory_permission: MemoryPermission,
// }

// struct AddressIterator<'a> {
//     region_iterator: MemoryRegionIterator<'a>,
//     region: MemoryRegionEntry,
// }

// impl<'a> AddressIterator<'a> {
//     pub fn new(process: &'a Process) -> Self {
//         Self {
//             region_iterator: MemoryRegionIterator::new(process, 0),
//             region: MemoryRegionEntry {
//                 range: 0..0,
//                 permission: MemoryPermission::READONLY,
//                 memory_type: MemoryType::UNKNOWN,
//             },
//         }
//     }
// }

// impl Iterator for AddressIterator<'_> {
//     type Item = Address;

//     fn next(&mut self) -> Option<Self::Item> {
//         loop {
//             match self.region.range.next() {
//                 Some(address) => Some(Address {
//                     address,
//                     memory_permission: self.region.permission,
//                     memory_type: self.region.memory_type,
//                 }),
//                 None => match self.region_iterator.next() {
//                     Some(region) => {
//                         self.region = region;
//                         continue;
//                     }
//                     None => {
//                         break;
//                     }
//                 },
//             };
//         }

//         return None;
//     }
// }

pub struct Scanner<'a> {
    process: &'a Process,
    addresses: Vec<usize>,
}

impl<'a> Scanner<'a> {
    pub fn new(process: &'a Process) -> Self {
        Self {
            process,
            addresses: Vec::new(),
        }
    }

    pub fn get_addresses(&self) -> &[usize] {
        &self.addresses[..]
    }

    pub fn new_scan(&mut self, expected: i32) {
        self.addresses.clear();

        for region in MemoryRegionIterator::new(self.process, 0) {
            let mut region_buffer = vec![0u8; region.range.len()];
            self.process
                .read_process_memory(region.range.start, &mut region_buffer)
                .unwrap();

            for offset in 0..region_buffer.len() - size_of::<i32>() {
                let mut int_buffer = [0u8; size_of::<i32>()];
                int_buffer.copy_from_slice(&region_buffer[offset..offset + size_of::<i32>()]);
                let actual = i32::from_le_bytes(int_buffer);

                if actual == expected {
                    self.addresses.push(region.range.start + offset);
                }
            }
        }
    }

    pub fn next_scan(&mut self, expected: i32) {
        self.addresses = self
            .addresses
            .iter()
            .filter_map(|&address| {
                let mut int_buffer = [0u8; size_of::<i32>()];
                self.process
                    .read_process_memory(address, &mut int_buffer[..])
                    .ok()?;
                let actual = i32::from_le_bytes(int_buffer);

                return if actual == expected {
                    Some(address)
                } else {
                    None
                };
            })
            .collect();
    }
}

fn cli() {
    let process = Process::new(
        ProcessIterator::new()
            .find(|proc| proc.name() == "Doukutsu.exe")
            .expect("could not find Doukutsu.exe")
            .pid(),
    );
    let mut scanner = Scanner::new(&process);

    let mut line_processor = |line: &str| -> Result<String, String> {
        match line
        .split(" ")
        .nth(0)
        .ok_or("expected at least one argument".to_string())? {
            "new_scan" => {
                let expected = line
                    .split(" ")
                    .nth(1)
                    .ok_or("new_scan [int]".to_string())?
                    .parse()
                    .or(Err("int argument could not be parsed as 32 bit int".to_string()))?;
                scanner.new_scan(expected);
                Ok("Scan done!".to_string())
            }
            "next_scan" => {
                let expected = line
                    .split(" ")
                    .nth(1)
                    .ok_or("next_scan [int]".to_string())?
                    .parse()
                    .or(Err("int argument could not be parsed as 32 bit int".to_string()))?;
                scanner.next_scan(expected);
                Ok("Scan done!".to_string())
            }
            "result_scan" => {
                for &address in scanner.get_addresses().iter() {
                    let mut value_buffer = i32::to_be_bytes(0);
                    let value = match process.read_process_memory(address, &mut value_buffer) {
                        Ok(_) => i32::from_le_bytes(value_buffer),
                        Err(_) => continue,
                    };
                    println!("{:#08x}\t{}", address, value);
                }
                Ok("All result printed!".to_string())
            }
            "set_value" => {
                let address = usize::from_str_radix(
                    line
                        .split(" ")
                        .nth(1)
                        .ok_or("set_value [address in hex] [int]".to_string())?,
                    16)
                    .or(Err("address argument could not be parsed as hexadecimal int"))?;

                let value: i32 = line
                    .split(" ")
                    .nth(2)
                    .ok_or("set_value [address] [int]".to_string())?
                    .parse()
                    .or(Err("int argument could not be parsed as 32 bit int".to_string()))?;

                let int_buffer = value.to_le_bytes();
                match  process.write_process_memory(address, &int_buffer) {
                    Ok(_) => Ok(format!("wrote {} at address {:#08x}", value, address)),
                    Err(_) => Err(format!("could not write to {:#08x}", address)),
                }
            }
            "get_value" => {
                let address = usize::from_str_radix(
                    line
                        .split(" ")
                        .nth(1)
                        .ok_or("get_value [address in hex]".to_string())?,
                    16)
                    .or(Err("address argument could not be parsed as hexadecimal int"))?;

                let mut int_buffer = i32::to_le_bytes(0);
                match  process.read_process_memory(address, &mut int_buffer) {
                    Ok(_) => Ok(i32::from_le_bytes(int_buffer).to_string()),
                    Err(_) => Err(format!("could not read at {:#08x}", address)),
                }
            }
            _ => {
                return Err("The only supported operations are: new_scan [int], next_scan [int], result_scan, get_value [address in hex] and set_value [address in hex] [int]".to_string())
            }
        }
    };

    println!("The supported operations are: new_scan [int], next_scan [int], result_scan, get_value [address in hex] and set_value [address in hex] [int]");
    print!("> ");
    io::stdout().flush().unwrap();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        match line_processor(&line) {
            Ok(s) => {
                println!("{}", s)
            }
            Err(err) => {
                println!("{}", err)
            }
        }
        print!("> ");
        io::stdout().flush().unwrap();
    }
}

fn main() {
    cli();
}
