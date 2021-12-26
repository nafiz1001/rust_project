use std::{
    ffi::{c_void, OsString},
    mem::size_of,
    ops::Range,
    os::windows::prelude::OsStringExt,
    ptr::null_mut,
};
use windows::Win32::{
    Foundation::*,
    System::{
        Diagnostics::{Debug::*, ToolHelp::*},
        LibraryLoader::*,
        Memory::*,
        Threading::*,
    },
};

fn wide_chars_to_string(wide_chars: &[u16]) -> String {
    OsString::from_wide(wide_chars)
        .to_string_lossy()
        .trim_end_matches(char::from(0))
        .to_string()
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
    first: bool,
}

impl ProcessIterator {
    pub fn new() -> Self {
        unsafe {
            let handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if handle.is_invalid() {
                panic!("CreateToolhelp32Snapshot failed");
            } else {
                return Self {
                    handle,
                    first: true,
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
            ..Default::default()
        };

        unsafe {
            if self.first {
                if !Process32FirstW(self.handle, &mut process_entry).as_bool() {
                    panic!("Process32FirstW failed");
                } else {
                    self.first = false;
                    return Some(ProcessEntry { process_entry });
                }
            } else {
                if !Process32NextW(self.handle, &mut process_entry).as_bool() {
                    return None;
                } else {
                    return Some(ProcessEntry { process_entry });
                }
            }
        }
    }
}

struct ModuleIterator {
    handle: HANDLE,
    first: bool,
}

impl ModuleIterator {
    fn new(pid: u32) -> Self {
        unsafe {
            let handle = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid);
            if handle.is_invalid() {
                panic!("CreateToolhelp32Snapshot failed");
            } else {
                return Self {
                    handle,
                    first: false,
                };
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
            ..Default::default()
        };

        unsafe {
            if self.first {
                if !Module32FirstW(self.handle, &mut module_entry).as_bool() {
                    panic!("Process32FirstW failed");
                } else {
                    self.first = false;
                    return Some(module_entry);
                }
            } else {
                if !Module32NextW(self.handle, &mut module_entry).as_bool() {
                    return None;
                } else {
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
        let handle;
        unsafe {
            handle = OpenProcess(
                PROCESS_QUERY_INFORMATION
                    | PROCESS_VM_READ
                    | PROCESS_VM_WRITE
                    | PROCESS_VM_OPERATION,
                false,
                pid,
            );

            if handle.is_invalid() {
                panic!("OpenProcess failed for pid {}", pid);
            }
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

    pub fn read_process_memory<T>(&self, start: usize, buffer: &mut [T]) -> Result<(), i64> {
        unsafe {
            if ReadProcessMemory(
                self.handle,
                start as *const c_void,
                buffer.as_mut_ptr() as *mut c_void,
                buffer.len() * size_of::<T>(),
                null_mut() as *mut usize,
            )
            .as_bool()
            {
                Ok(())
            } else {
                Err(GetLastError() as i64)
            }
        }
    }

    pub fn write_process_memory<T>(&self, start: usize, buffer: &[T]) -> Result<(), i64> {
        unsafe {
            if WriteProcessMemory(
                self.handle,
                start as *mut c_void,
                buffer.as_ptr() as *const c_void,
                buffer.len() * size_of::<T>(),
                null_mut() as *mut usize,
            )
            .as_bool()
            {
                Ok(())
            } else {
                Err(GetLastError() as i64)
            }
        }
    }

    pub fn suspend(&self) -> Result<(), i64> {
        unsafe {
            if DebugActiveProcess(self.id()).as_bool() {
                Ok(())
            } else {
                Err(GetLastError() as i64)
            }
        }
    }

    pub fn resume(&self) -> Result<(), i64> {
        unsafe {
            if DebugActiveProcessStop(self.id()).as_bool() {
                Ok(())
            } else {
                Err(GetLastError() as i64)
            }
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
    pub module: String,
}

pub struct MemoryRegionIterator<'a> {
    memory_basic_information: MEMORY_BASIC_INFORMATION,
    process: &'a Process,
}

impl<'a> MemoryRegionIterator<'a> {
    pub fn new(process: &'a Process, starting_address: usize) -> Self {
        Self {
            process,
            memory_basic_information: MEMORY_BASIC_INFORMATION {
                BaseAddress: starting_address as *mut c_void,
                ..Default::default()
            },
        }
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

                    if State == MEM_COMMIT {
                        let permission = match Protect {
                            PAGE_READONLY | PAGE_EXECUTE_READ => MemoryPermission::READONLY,
                            PAGE_READWRITE
                            | PAGE_EXECUTE_READWRITE
                            | PAGE_WRITECOPY
                            | PAGE_EXECUTE_WRITECOPY => MemoryPermission::READWRITE,
                            _ => continue,
                        };

                        let mut module_handle = 0;
                        return Some(MemoryRegionEntry {
                            range: BaseAddress as usize..BaseAddress as usize + RegionSize,
                            permission,
                            module: if GetModuleHandleExW(
                                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                                PWSTR(BaseAddress as *mut u16),
                                &mut module_handle,
                            )
                            .as_bool()
                            {
                                let mut name = [0u16; MAX_PATH as usize];
                                GetModuleFileNameW(
                                    module_handle,
                                    PWSTR(name.as_mut_ptr()),
                                    MAX_PATH,
                                );
                                wide_chars_to_string(&name)
                            } else {
                                "".to_string()
                            },
                        });
                    } else {
                        continue;
                    }
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

        MemoryRegionIterator::new(&process, module.modBaseAddr as usize)
            .find(|region| region.range.contains(&0x0049E6CC))
            .expect("could not find 0x0049E6CC");

        assert!(
            MemoryRegionIterator::new(&process, 0)
                .filter(|r| {
                    let mut buffer = vec![0u8; r.range.len()];
                    process
                        .read_process_memory(r.range.start, &mut buffer)
                        .is_err()
                })
                .count()
                == 0
        );
    }

    #[test]
    fn read_write_process_memory() {
        let process = find_process();

        let mut old_hp_bytes = 0i32.to_le_bytes();
        process
            .read_process_memory(0x0049E6CC, &mut old_hp_bytes)
            .unwrap();
        let old_hp = i32::from_le_bytes(old_hp_bytes);

        let mut new_hp_bytes = (old_hp + 1).to_le_bytes();
        process
            .write_process_memory(0x0049E6CC, &new_hp_bytes)
            .unwrap();

        process
            .read_process_memory(0x0049E6CC, &mut new_hp_bytes)
            .unwrap();
        assert_eq!(old_hp + 1, i32::from_le_bytes(new_hp_bytes));
    }

    #[test]
    fn suspend_resume() {
        let process = find_process();
        process.suspend().unwrap();
        process.resume().unwrap();
    }
}
