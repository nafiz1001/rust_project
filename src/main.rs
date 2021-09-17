use core::{panic, time};
use std::{
    ffi::{c_void, OsString},
    fmt,
    io::{self, Write},
    mem::size_of,
    ops::Range,
    os::windows::prelude::OsStringExt,
    ptr::null_mut,
    thread,
};

use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use windows_bindings::Windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HINSTANCE, INVALID_HANDLE_VALUE},
    System::{Diagnostics::Debug::ReadProcessMemory, Threading::PROCESS_VM_READ},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Module32FirstW, MODULEENTRY32W, TH32CS_SNAPMODULE,
            TH32CS_SNAPMODULE32,
        },
        Threading::{OpenProcess, PROCESS_ACCESS_RIGHTS},
    },
};

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

struct ModuleEntry {
    _module_id: u32,
    _process_id: u32,
    _glblcnt_usage: u32,
    _proccnt_usage: u32,
    mod_base_addr: *mut u8,
    _mod_base_size: u32,
    _h_module: HINSTANCE,
    _module_name: String,
    _exe_path: String,
}

impl ModuleEntry {
    pub fn from(&module_entry: &MODULEENTRY32W) -> Self {
        return Self {
            _module_id: module_entry.th32ModuleID,
            _process_id: module_entry.th32ProcessID,
            _glblcnt_usage: module_entry.GlblcntUsage,
            _proccnt_usage: module_entry.ProccntUsage,
            mod_base_addr: module_entry.modBaseAddr,
            _mod_base_size: module_entry.modBaseSize,
            _h_module: module_entry.hModule,
            _module_name: OsString::from_wide(&module_entry.szModule[..])
                .to_string_lossy()
                .trim_end_matches(char::from(0))
                .to_string(),
            _exe_path: OsString::from_wide(&module_entry.szExePath[..])
                .to_string_lossy()
                .trim_end_matches(char::from(0))
                .to_string(),
        };
    }
}

struct Process {
    handle: Handle,
    module: ModuleEntry,
}

impl Process {
    pub fn new(pid: u32, desired_access: PROCESS_ACCESS_RIGHTS) -> Self {
        let handle: Handle;
        unsafe {
            handle = Handle(OpenProcess(desired_access, false, pid));
        }

        let module: ModuleEntry;

        unsafe {
            let h_snapshot = Handle(CreateToolhelp32Snapshot(
                TH32CS_SNAPMODULE32 | TH32CS_SNAPMODULE,
                pid,
            ));
            if h_snapshot.0 == INVALID_HANDLE_VALUE {
                panic!("CreateToolhelp32Snapshot failed");
            }

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
                szExePath: [0u16; 260],
            };

            if Module32FirstW(h_snapshot.0, &mut module_entry).as_bool() {
                module = ModuleEntry::from(&module_entry);
            } else {
                panic!("Module32FirstW failed");
            }
        }

        Self {
            handle: handle,
            module: module,
        }
    }

    fn read_process_memory(&self, range: Range<usize>) -> Vec<u8> {
        let mut buffer: Vec<u8> = Vec::new();
        buffer.resize(range.len(), 0);

        unsafe {
            if !ReadProcessMemory(
                self.handle.0,
                (self.module.mod_base_addr as usize + range.start) as *const c_void,
                buffer.as_mut_ptr() as *mut c_void,
                buffer.len(),
                null_mut() as *mut usize,
            )
            .as_bool()
            {
                panic!(
                    "ReadProcessMemory failed to read between the range {:?}",
                    range
                );
            }
        }

        return buffer;
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

    print!("Enter Process ID: ");
    io::stdout().flush().unwrap();

    let mut pid = String::new();
    io::stdin().read_line(&mut pid).unwrap();
    let pid: u32 = pid.trim().parse().unwrap();

    let process = Process::new(pid, PROCESS_VM_READ);

    let mut bytes = [0u8; 4];
    loop {
        thread::sleep(time::Duration::new(1, 0));
        let data = process.read_process_memory(0x0009E6CC..(0x0009E6CC + 4));
        bytes.copy_from_slice(&data[0..4]);
        let health = u32::from_le_bytes(bytes);
        println!("{}", health);
    }
}
