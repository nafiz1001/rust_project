use core::{Process, MemoryRegionIterator};
use std::mem::size_of;

pub struct Scanner<'a, P> where P: Process {
    process: &'a P,
    addresses: Vec<usize>,
}

impl<'a, P: Process> Scanner<'a, P> {
    pub fn new(process: &'a P) -> Self {
        Self {
            process,
            addresses: Vec::new(),
        }
    }

    pub fn get_addresses(&self) -> &[usize] {
        &self.addresses[..]
    }

    pub fn new_scan<T: PartialEq, F: FnMut(&T) -> bool, M: MemoryRegionIterator<'a, P>>(&mut self, mut predicate: F) {
        self.addresses.clear();

        for region in M::new(self.process, 0, usize::MAX) {
            let mut region_buffer = vec![0u8; region.range.len()];
            match self
                .process
                .read_memory_slice(region.range.start, &mut region_buffer)
            {
                Ok(_) => {}
                Err(_) => continue,
            }

            for offset in 0..region_buffer.len() - size_of::<T>() {
                unsafe {
                    let actual = std::slice::from_raw_parts(
                        region_buffer.as_ptr().offset(offset as isize) as *const T,
                        1,
                    );

                    if predicate(&actual[0]) {
                        self.addresses.push(region.range.start + offset);
                    }
                }
            }
        }
    }

    pub fn next_scan<T: PartialEq, F: FnMut(&T) -> bool>(&mut self, mut predicate: F) {
        self.addresses = self
            .addresses
            .iter()
            .filter_map(|&address| {
                let mut buffer = vec![0u8; size_of::<T>()];
                self.process
                    .read_memory_slice(address, &mut buffer)
                    .ok()?;

                unsafe {
                    let actual = std::slice::from_raw_parts(buffer.as_ptr() as *const T, 1);

                    return if predicate(&actual[0]) {
                        Some(address)
                    } else {
                        None
                    };
                }
            })
            .collect();
    }
}
