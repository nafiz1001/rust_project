windows::include_bindings!();

use std::{
    ffi::{c_void, OsString},
    fmt,
    mem::size_of,
    os::windows::prelude::OsStringExt,
    ptr::null_mut,
};

use crate::Windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HINSTANCE, INVALID_HANDLE_VALUE, MAX_PATH},
    System::Diagnostics::{Debug::ReadProcessMemory, ToolHelp::PROCESSENTRY32W},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW,
            Process32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
            TH32CS_SNAPPROCESS,
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

pub struct Handle(HANDLE);

impl Handle {
    pub fn close(&self) {
        unsafe {
            CloseHandle(self.0);
        }
    }

    pub fn is_invalid(&self) -> bool {
        self.0 == INVALID_HANDLE_VALUE
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.close()
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

pub struct ProcessEntry(PROCESSENTRY32W);

impl ProcessEntry {
    pub fn id(&self) -> u32 {
        self.0.th32ProcessID
    }

    pub fn name(&self) -> String {
        wide_chars_to_string(&self.0.szExeFile[..])
    }
}

pub struct ProcessIterator {
    handle: Handle,
    count: usize,
}

impl ProcessIterator {
    pub fn new() -> Self {
        unsafe {
            let handle = Handle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0));
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
                if !Process32FirstW(self.handle.0, &mut process_entry).as_bool() {
                    panic!("Process32FirstW failed");
                } else {
                    self.count += 1;
                    return Some(ProcessEntry(process_entry));
                }
            } else {
                if !Process32NextW(self.handle.0, &mut process_entry).as_bool() {
                    return None;
                } else {
                    self.count += 1;
                    return Some(ProcessEntry(process_entry));
                }
            }
        }
    }
}

pub struct ModuleIterator {
    handle: Handle,
    count: usize,
}

impl ModuleIterator {
    pub fn new(pid: u32) -> Self {
        unsafe {
            let handle = Handle(CreateToolhelp32Snapshot(
                TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32,
                pid,
            ));
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
                if !Module32FirstW(self.handle.0, &mut module_entry).as_bool() {
                    panic!("Process32FirstW failed");
                } else {
                    self.count += 1;
                    return Some(module_entry);
                }
            } else {
                if !Module32NextW(self.handle.0, &mut module_entry).as_bool() {
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
    handle: Handle,
    module: MODULEENTRY32W,
}

impl Process {
    pub fn new(pid: u32) -> Self {
        let handle: Handle;
        unsafe {
            handle = Handle(OpenProcess(
                PROCESS_QUERY_INFORMATION
                    | PROCESS_VM_READ
                    | PROCESS_VM_WRITE
                    | PROCESS_VM_OPERATION,
                false,
                pid,
            ));
        }

        let module = ModuleIterator::new(pid).next().unwrap();

        Self { handle, module }
    }

    pub fn memory_len(&self) -> usize {
        return self.module.modBaseSize as usize;
    }

    pub fn read_process_memory<T>(&self, relative_start: usize, buffer: &mut [T]) {
        unsafe {
            if !ReadProcessMemory(
                self.handle.0,
                (self.module.modBaseAddr as usize + relative_start) as *const c_void,
                buffer.as_mut_ptr() as *mut c_void,
                buffer.len() * size_of::<T>(),
                null_mut() as *mut usize,
            )
            .as_bool()
            {
                panic!(
                    "ReadProcessMemory failed to read between the range {:?}",
                    relative_start..(relative_start + buffer.len())
                );
            }
        }
    }
}
