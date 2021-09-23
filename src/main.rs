#![allow(non_snake_case)]

use core::panic;
use std::{
    collections::HashMap,
    ffi::{c_void, OsString},
    fmt,
    fs::File,
    io::{self, LineWriter, Write},
    mem::size_of,
    os::windows::prelude::OsStringExt,
    path::Path,
    ptr::null_mut,
};

use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use windows_bindings::Windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HINSTANCE, INVALID_HANDLE_VALUE, MAX_PATH},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW,
            Process32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
            TH32CS_SNAPPROCESS,
        },
        Threading::{OpenProcess, PROCESS_ACCESS_RIGHTS},
    },
    System::{
        Diagnostics::{Debug::ReadProcessMemory, ToolHelp::PROCESSENTRY32W},
        Threading::PROCESS_VM_READ,
    },
};

struct Handle(HANDLE);

impl Handle {
    fn close(&self) {
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

fn wide_chars_to_string(wide_chars: &[u16]) -> String {
    OsString::from_wide(wide_chars)
        .to_string_lossy()
        .trim_end_matches(char::from(0))
        .to_string()
}

struct ProcessEnumerator {
    handle: Handle,
    count: usize,
}

impl ProcessEnumerator {
    fn new() -> Self {
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

impl Iterator for ProcessEnumerator {
    type Item = PROCESSENTRY32W;

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
                    return Some(process_entry);
                }
            } else {
                if !Process32NextW(self.handle.0, &mut process_entry).as_bool() {
                    return None;
                } else {
                    self.count += 1;
                    return Some(process_entry);
                }
            }
        }
    }
}

struct ModuleEnumerator {
    handle: Handle,
    count: usize,
}

impl ModuleEnumerator {
    fn new(pid: u32) -> Self {
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

impl Iterator for ModuleEnumerator {
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
    pub fn new(pid: u32, desired_access: PROCESS_ACCESS_RIGHTS) -> Self {
        let handle: Handle;
        unsafe {
            handle = Handle(OpenProcess(desired_access, false, pid));
        }

        let module = ModuleEnumerator::new(pid).next().unwrap();

        Self { handle, module }
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

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

pub fn init_log() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Error))
}

fn main() {
    init_log().expect("could not initialize log");

    let mut processes: Vec<_> = ProcessEnumerator::new()
        .map(|entry| {
            (
                entry.th32ProcessID,
                wide_chars_to_string(&entry.szExeFile[..]),
            )
        })
        .collect();
    processes.sort_by(|(_, a), (_, b)| a.to_lowercase().cmp(&b.to_lowercase()));

    for (pid, name) in processes {
        println!("{}\t{}", pid, name,);
    }

    print!("Enter Process ID: ");
    io::stdout().flush().unwrap();

    let mut pid = String::new();
    io::stdin().read_line(&mut pid).unwrap();
    let pid: u32 = pid.trim().parse().unwrap();

    let process = Process::new(pid, PROCESS_VM_READ);
    let mut buffer = vec![0u8; process.module.modBaseSize as usize].into_boxed_slice();

    let mut bytes = [0u8; 4];

    let mut matches = HashMap::<usize, u32>::new();

    for k in 0..(buffer.len() - 4) {
        bytes.copy_from_slice(&buffer[k..k + 4]);
        let v = u32::from_le_bytes(bytes);

        matches.insert(k, v);
    }

    loop {
        let mut expected = String::new();
        io::stdin().read_line(&mut expected).unwrap();
        let expected: u32 = expected.trim().parse().unwrap();

        process.read_process_memory(0, &mut buffer);

        for k in 0..(buffer.len() - 4) {
            bytes.copy_from_slice(&buffer[k..k + 4]);
            let v = u32::from_le_bytes(bytes);

            if v != expected {
                matches.remove(&k);
            } else {
                if matches.contains_key(&k) {
                    matches.insert(k, v);
                }
            }
        }

        println!("{} remaining results", matches.len());

        let mut file = LineWriter::new(File::create(Path::new("scan.txt")).unwrap());
        for (k, v) in matches.iter() {
            file.write_all(format!("{:#08x}\t{}", k, v).as_bytes())
                .unwrap();
            file.write_all(b"\n").unwrap();
        }
    }
}
