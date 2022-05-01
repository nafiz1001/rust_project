use std::mem::size_of;

use core::ProcessTrait;

#[cfg(target_os = "linux")]
use linux::{MemoryRegionIterator, Process};
#[cfg(target_os = "windows")]
use windows::{MemoryRegionIterator, Process};

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

    pub fn new_scan<T: PartialEq, P: FnMut(&T) -> bool>(&mut self, mut predicate: P) {
        self.addresses.clear();

        for region in MemoryRegionIterator::new(self.process, 0) {
            let mut region_buffer = vec![0u8; region.range.len()];
            match self
                .process
                .read_memory(region.range.start, &mut region_buffer)
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

    pub fn next_scan<T: PartialEq, P: FnMut(&T) -> bool>(&mut self, mut predicate: P) {
        self.addresses = self
            .addresses
            .iter()
            .filter_map(|&address| {
                let mut buffer = vec![0u8; size_of::<T>()];
                self.process
                    .read_memory(address, &mut buffer)
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
