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
    System::Diagnostics::{Debug::ReadProcessMemory, ToolHelp::PROCESSENTRY32W},
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
        SystemInformation::{GetSystemInfo, SYSTEM_INFO, SYSTEM_INFO_0},
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
}

impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

pub enum PagePermission {
    READONLY,
    READWRITE,
}

pub struct PageEntry {
    pub range: Range<usize>,
    pub permission: PagePermission,
}

pub struct PageIterator<'a> {
    memory_basic_information: MEMORY_BASIC_INFORMATION,
    process: &'a Process,
    current_address: *mut c_void,
    page_size: usize,
}

impl<'a> PageIterator<'a> {
    pub fn new(process: &'a Process, starting_address: usize) -> Self {
        let mut system_info = SYSTEM_INFO {
            Anonymous: SYSTEM_INFO_0 { dwOemId: 0 },
            dwPageSize: 0,
            lpMinimumApplicationAddress: null_mut() as *mut c_void,
            lpMaximumApplicationAddress: null_mut() as *mut c_void,
            dwActiveProcessorMask: 0,
            dwNumberOfProcessors: 0,
            dwProcessorType: 0,
            dwAllocationGranularity: 0,
            wProcessorLevel: 0,
            wProcessorRevision: 0,
        };

        unsafe {
            GetSystemInfo(&mut system_info as *mut SYSTEM_INFO);
        }

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
            current_address: null_mut(),
            page_size: system_info.dwPageSize as usize,
        };
    }
}

impl Iterator for PageIterator<'_> {
    type Item = PageEntry;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            loop {
                if self.current_address.is_null() {
                    if VirtualQueryEx(
                        self.process.handle,
                        self.memory_basic_information.BaseAddress,
                        &mut self.memory_basic_information as *mut MEMORY_BASIC_INFORMATION,
                        size_of::<MEMORY_BASIC_INFORMATION>(),
                    ) <= 0
                    {
                        return None;
                    }

                    self.current_address = self.memory_basic_information.BaseAddress;
                }

                if self
                    .current_address
                    .offset_from(self.memory_basic_information.BaseAddress)
                    >= self.memory_basic_information.RegionSize as isize
                {
                    if VirtualQueryEx(
                        self.process.handle,
                        self.current_address,
                        &mut self.memory_basic_information as *mut MEMORY_BASIC_INFORMATION,
                        size_of::<MEMORY_BASIC_INFORMATION>(),
                    ) <= 0
                    {
                        return None;
                    }

                    self.current_address = self.memory_basic_information.BaseAddress;
                }

                let page_entry = PageEntry {
                    range: self.current_address as usize
                        ..self.current_address as usize + self.page_size as usize,
                    permission: match self.memory_basic_information {
                        MEMORY_BASIC_INFORMATION {
                            State: MEM_COMMIT,
                            Protect: PAGE_READONLY,
                            ..
                        } => PagePermission::READONLY,
                        MEMORY_BASIC_INFORMATION {
                            State: MEM_COMMIT,
                            Protect: PAGE_READWRITE,
                            ..
                        } => PagePermission::READWRITE,
                        MEMORY_BASIC_INFORMATION {
                            BaseAddress,
                            RegionSize,
                            ..
                        } => {
                            self.current_address = BaseAddress.offset(RegionSize as isize);
                            continue;
                        }
                    },
                };

                self.current_address = self.current_address.offset(self.page_size as isize);

                return Some(page_entry);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{PageIterator, Process, ProcessIterator};

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
        let entry = ProcessIterator::new()
            .find(|proc| proc.name() == "Doukutsu.exe")
            .expect("failed to find Doukutsu.exe");
        let process = Process::new(entry.id());
        let module = process.module();

        let pages = PageIterator::new(&process, module.modBaseAddr as usize);
        assert!(
            pages
                .take_while(
                    |p| (p.range.end - module.modBaseAddr as usize) < module.modBaseSize as usize
                )
                .count()
                > 0
        );
    }

    #[test]
    fn read_process_memory() {
        let entry = ProcessIterator::new()
            .find(|proc| proc.name() == "Doukutsu.exe")
            .expect("failed to find Doukutsu.exe");
        let process = Process::new(entry.id());
        let module = process.module();
        let pages = PageIterator::new(&process, process.module().modBaseAddr as usize);

        let mut count = 0;
        for page in pages {
            if (page.range.clone().end - module.modBaseAddr as usize) >= module.modBaseSize as usize
            {
                break;
            }

            let mut buffer = vec![0u8; page.range.len()];
            process.read_process_memory(page.range.start, &mut buffer);
            count = count + 1;
        }
        assert!(count > 0)
    }
}
