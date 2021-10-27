#![allow(non_snake_case)]

windows::include_bindings!();

use std::{
    ffi::{c_void, OsString},
    fmt,
    mem::size_of,
    ops::Range,
    os::windows::prelude::OsStringExt,
    ptr::null_mut,
};

use crate::Windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HINSTANCE, MAX_PATH},
    System::Diagnostics::{Debug::{ReadProcessMemory, WriteProcessMemory, DebugActiveProcess, DebugActiveProcessStop}, ToolHelp::PROCESSENTRY32W},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW,
            Process32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
            TH32CS_SNAPPROCESS,
        },
        Memory::{
            VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_PROTECTION_FLAGS,
            PAGE_READONLY, PAGE_READWRITE, PAGE_TYPE, VIRTUAL_ALLOCATION_TYPE,
        },
        Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ,
            PROCESS_VM_WRITE,
        },
    },
};

fn wide_chars_to_string(wide_chars: &[u16]) -> String {
    OsString::from_wide(wide_chars)
        .to_string_lossy()
        .trim_end_matches(char::from(0))
        .to_string()
}

impl fmt::Display for HANDLE {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct ProcessEntry {
    process_entry: PROCESSENTRY32W,
}

impl ProcessEntry {
    pub fn id(&self) -> u32 {
        self.process_entry.th32ProcessID
    }

    pub fn name(&self) -> String {
        wide_chars_to_string(&self.process_entry.szExeFile[..])
    }
}

pub struct ProcessIterator {
    handle: HANDLE,
    count: usize,
}

impl ProcessIterator {
    pub fn new() -> Self {
        unsafe {
            let handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if handle.is_invalid() {
                panic!("CreateToolhelp32Snapshot failed");
            } else {
                return Self {
                    handle: handle,
                    count: 0,
                };
            }
        }
    }
}

impl Iterator for ProcessIterator {
    type Item = ProcessEntry;

    // next() is the only required method
    fn next(&mut self) -> Option<Self::Item> {
        let mut process_entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            cntUsage: 0,
            th32ProcessID: 0,
            th32DefaultHeapID: 0,
            th32ModuleID: 0,
            cntThreads: 0,
            th32ParentProcessID: 0,
            pcPriClassBase: 0,
            dwFlags: 0,
            szExeFile: [0u16; MAX_PATH as usize],
        };

        unsafe {
            if self.count == 0 {
                if !Process32FirstW(self.handle, &mut process_entry).as_bool() {
                    panic!("Process32FirstW failed");
                } else {
                    self.count += 1;
                    return Some(ProcessEntry { process_entry });
                }
            } else {
                if !Process32NextW(self.handle, &mut process_entry).as_bool() {
                    return None;
                } else {
                    self.count += 1;
                    return Some(ProcessEntry { process_entry });
                }
            }
        }
    }
}

struct ModuleIterator {
    handle: HANDLE,
    count: usize,
}

impl ModuleIterator {
    fn new(pid: u32) -> Self {
        unsafe {
            let handle = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid);
            if handle.is_invalid() {
                panic!("CreateToolhelp32Snapshot failed");
            } else {
                return Self { handle, count: 0 };
            }
        }
    }
}

impl Iterator for ModuleIterator {
    type Item = MODULEENTRY32W;

    // next() is the only required method
    fn next(&mut self) -> Option<Self::Item> {
        let mut module_entry = MODULEENTRY32W {
            dwSize: size_of::<MODULEENTRY32W>() as u32,
            th32ProcessID: 0,
            th32ModuleID: 0,
            GlblcntUsage: 0,
            ProccntUsage: 0,
            modBaseAddr: null_mut(),
            modBaseSize: 0,
            hModule: HINSTANCE(0),
            szModule: [0u16; 256],
            szExePath: [0u16; MAX_PATH as usize],
        };

        unsafe {
            if self.count == 0 {
                if !Module32FirstW(self.handle, &mut module_entry).as_bool() {
                    panic!("Process32FirstW failed");
                } else {
                    self.count += 1;
                    return Some(module_entry);
                }
            } else {
                if !Module32NextW(self.handle, &mut module_entry).as_bool() {
                    return None;
                } else {
                    self.count += 1;
                    return Some(module_entry);
                }
            }
        }
    }
}

pub struct Process {
    handle: HANDLE,
    pid: u32,
}

impl Process {
    pub fn new(pid: u32) -> Self {
        let handle: HANDLE;
        unsafe {
            handle = OpenProcess(
                PROCESS_QUERY_INFORMATION
                    | PROCESS_VM_READ
                    | PROCESS_VM_WRITE
                    | PROCESS_VM_OPERATION,
                false,
                pid,
            );
        }

        Self { handle, pid }
    }

    fn module(&self) -> MODULEENTRY32W {
        ModuleIterator::new(self.pid).next().unwrap()
    }

    pub fn id(&self) -> u32 {
        self.module().th32ProcessID
    }

    pub fn name(&self) -> String {
        wide_chars_to_string(&self.module().szModule)
    }

    pub fn read_process_memory<T>(&self, start: usize, buffer: &mut [T]) {
        unsafe {
            if !ReadProcessMemory(
                self.handle,
                start as *const c_void,
                buffer.as_mut_ptr() as *mut c_void,
                buffer.len() * size_of::<T>(),
                null_mut() as *mut usize,
            )
            .as_bool()
            {
                panic!(
                    "ReadProcessMemory failed to read between the range {:#016x}..{:#016x}",
                    start,
                    (start + buffer.len())
                );
            }
        }
    }

    pub fn write_process_memory<T>(&self, start: usize, buffer: &[T]) {
        unsafe {
            if !WriteProcessMemory(
                self.handle,
                start as *mut c_void,
                buffer.as_ptr() as *const c_void,
                buffer.len() * size_of::<T>(),
                null_mut() as *mut usize,
            )
            .as_bool()
            {
                panic!(
                    "WriteProcessMemory failed to write between the range {:#016x}..{:#016x}",
                    start,
                    (start + buffer.len())
                );
            }
        }
    }

    pub fn suspend(&self) -> bool {
        unsafe {
            DebugActiveProcess(self.id()).as_bool()
        }
    }

    pub fn stop_suspend(&self) -> bool {
        unsafe {
            DebugActiveProcessStop(self.id()).as_bool()
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

pub enum MemoryPermission {
    READONLY,
    READWRITE,
}

pub struct MemoryRegionEntry {
    pub range: Range<usize>,
    pub permission: MemoryPermission,
    pub info: String,
}

pub struct MemoryRegionIterator<'a> {
    memory_basic_information: MEMORY_BASIC_INFORMATION,
    process: &'a Process,
}

impl<'a> MemoryRegionIterator<'a> {
    pub fn new(process: &'a Process, starting_address: usize) -> Self {
        return Self {
            process,
            memory_basic_information: MEMORY_BASIC_INFORMATION {
                BaseAddress: starting_address as *mut c_void,
                AllocationBase: 0 as *mut c_void,
                AllocationProtect: PAGE_PROTECTION_FLAGS(0),
                PartitionId: 0,
                RegionSize: 0,
                State: VIRTUAL_ALLOCATION_TYPE(0),
                Protect: PAGE_PROTECTION_FLAGS(0),
                Type: PAGE_TYPE(0),
            },
        };
    }
}

impl Iterator for MemoryRegionIterator<'_> {
    type Item = MemoryRegionEntry;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            loop {
                if VirtualQueryEx(
                    self.process.handle,
                    self.memory_basic_information.BaseAddress,
                    &mut self.memory_basic_information as *mut MEMORY_BASIC_INFORMATION,
                    size_of::<MEMORY_BASIC_INFORMATION>(),
                ) <= 0
                {
                    return None;
                } else {
                    let MEMORY_BASIC_INFORMATION {
                        BaseAddress,
                        RegionSize,
                        State,
                        Protect,
                        ..
                    } = self.memory_basic_information;

                    self.memory_basic_information.BaseAddress =
                        BaseAddress.offset(RegionSize as isize);

                    let range = BaseAddress as usize..BaseAddress as usize + RegionSize;
                    return Some(match State {
                        MEM_COMMIT => MemoryRegionEntry {
                            range,
                            permission: match Protect {
                                PAGE_READONLY => MemoryPermission::READONLY,
                                PAGE_READWRITE => MemoryPermission::READWRITE,
                                _ => continue,
                            },
                            info: self.process.name()
                        },
                        _ => continue,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{MemoryRegionIterator, Process, ProcessIterator};

    fn find_process() -> Process {
        let entry = ProcessIterator::new()
            .find(|proc| proc.name() == "Doukutsu.exe")
            .expect("failed to find Doukutsu.exe");
        Process::new(entry.id())
    }

    #[test]
    fn enumerate_processes() {
        ProcessIterator::new()
            .find(|proc| proc.name() == "Doukutsu.exe")
            .expect("failed to find Doukutsu.exe");
    }

    #[test]
    fn process() {
        let entry = ProcessIterator::new()
            .find(|proc| proc.name() == "Doukutsu.exe")
            .expect("failed to find Doukutsu.exe");
        let process = Process::new(entry.id());

        assert_eq!(entry.id(), process.id());
    }

    #[test]
    fn accessible_memory_region() {
        let process = find_process();
        let module = process.module();

        let regions = MemoryRegionIterator::new(&process, module.modBaseAddr as usize);
        assert!(regions.count() > 0);
    }

    #[test]
    fn read_process_memory() {
        let process = find_process();
        let regions = MemoryRegionIterator::new(&process, 0usize);

        assert!(
            regions
                .map(|r| {
                    let mut buffer = vec![0u8; r.range.len()];
                    process.read_process_memory(r.range.start, &mut buffer);
                    return 1;
                })
                .count()
                > 0
        );
    }

    #[test]
    fn write_process_memory() {
        let process = find_process();

        process.write_process_memory(0x0049E6CC, &[10u8, 0u8, 0u8, 0u8]);
    }

    #[test]
    fn suspend() {
        let process = find_process();
        assert!(process.suspend());
        assert!(process.stop_suspend());
    }
}
