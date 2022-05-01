use std::{
    io::{self, BufRead, Write},
    mem::size_of,
    process::Command,
};

#[cfg(target_os = "windows")]
use windows::{MemoryRegionIterator, Process};

#[cfg(target_os = "linux")]
use linux::{MemoryRegionIterator, Process};

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

    pub fn new_scan<T: PartialEq>(&mut self, expected: T) {
        self.addresses.clear();

        for region in MemoryRegionIterator::new(self.process, 0) {
            let mut region_buffer = vec![0u8; region.range.len()];
            self.process
                .read_process_memory(region.range.start, &mut region_buffer)
                .unwrap();

            for offset in 0..region_buffer.len() - size_of::<T>() {
                unsafe {
                    let actual = std::slice::from_raw_parts(
                        region_buffer.as_ptr().offset(offset as isize) as *const T,
                        1,
                    );

                    if actual[0] == expected {
                        self.addresses.push(region.range.start + offset);
                    }
                }
            }
        }
    }

    pub fn next_scan<T: PartialEq>(&mut self, expected: T) {
        self.addresses = self
            .addresses
            .iter()
            .filter_map(|&address| {
                let mut buffer = vec![0u8; size_of::<T>()];
                self.process
                    .read_process_memory(address, &mut buffer)
                    .ok()?;

                unsafe {
                    let actual = std::slice::from_raw_parts(
                        buffer.as_ptr() as *const T,
                        1,
                    );

                    return if actual[0] == expected {
                        Some(address)
                    } else {
                        None
                    };
                }
            })
            .collect();
    }
}

fn cli() {
    print!("Enter process path: ");
    io::stdout().flush().unwrap();
    let stdin = io::stdin();
    let mut buf = String::new();
    stdin.read_line(&mut buf).unwrap();

    let path = buf.trim();
    let child = Command::new(path).spawn().unwrap();
    let process = Process::new(child.id());

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
                    .parse::<i32>()
                    .or(Err("int argument could not be parsed as 32 bit int".to_string()))?;
                scanner.new_scan(expected);
                Ok("Scan done!".to_string())
            }
            "next_scan" => {
                let expected = line
                    .split(" ")
                    .nth(1)
                    .ok_or("next_scan [int]".to_string())?
                    .parse::<i32>()
                    .or(Err("int argument could not be parsed as 32 bit int".to_string()))?;
                scanner.next_scan(expected);
                Ok("Scan done!".to_string())
            }
            "result_scan" => {
                for &address in scanner.get_addresses().iter() {
                    let mut value = [0];
                    let value = match process.read_process_memory(address, &mut value) {
                        Ok(_) => value[0],
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
                let value = [value];

                match  process.write_process_memory(address, &value) {
                    Ok(_) => Ok(format!("wrote {} at address {:#08x}", value[0], address)),
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

                let mut value = [0];
                match  process.read_process_memory(address, &mut value) {
                    Ok(_) => Ok(value[0].to_string()),
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
