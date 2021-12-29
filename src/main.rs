use std::mem::size_of;

use windows_core::{
    MemoryRegionIterator, Process,
};

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

fn main() {
    println!("Hello World!");
}
