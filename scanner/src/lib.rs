use core::{MemoryRegionIterator, Process};
use std::{mem::size_of, sync::Arc};

pub struct Scanner<P>
where
    P: Process,
{
    process: Arc<P>,
    addresses: Vec<usize>,
    value_size: usize,
}

impl<P: Process> Scanner<P> {
    pub fn new(process: Arc<P>) -> Self {
        Self {
            process,
            addresses: Vec::new(),
            value_size: 0,
        }
    }

    pub fn get_addresses(&self) -> &[usize] {
        &self.addresses[..]
    }

    pub fn new_scan<'a, T: PartialEq, F: FnMut(&T) -> bool, M: MemoryRegionIterator<'a, P>>(
        &'a mut self,
        mut predicate: F,
    ) {
        self.addresses.clear();
        self.value_size = size_of::<T>();

        for region in M::new(self.process.as_ref(), 0, usize::MAX) {
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
        assert!(size_of::<T>() <= self.value_size);

        self.addresses = self
            .addresses
            .iter()
            .filter_map(|&address| {
                let mut buffer = vec![0u8; size_of::<T>()];
                self.process.read_memory_slice(address, &mut buffer).ok()?;

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
