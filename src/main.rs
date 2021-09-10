#[allow(unused_imports)]
use log::{error, warn, Level, Metadata, Record};
use std::{ffi::OsString, os::windows::prelude::OsStringExt};
use windows_bindings::Windows::Win32::{Foundation::{CloseHandle, HANDLE}, System::Threading::{OpenProcess, PROCESS_ACCESS_RIGHTS, PROCESS_VM_WRITE}};

struct Handle(HANDLE);

impl Handle {
    fn new(pid: u32, desired_access: PROCESS_ACCESS_RIGHTS) -> Handle {
        unsafe {
            return Handle (
                OpenProcess(desired_access, false, pid),
            );
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn process_ids() -> Vec<u32> {
    use std::mem::{size_of, size_of_val};
    use windows_bindings::Windows::Win32::System::ProcessStatus::K32EnumProcesses;

    let mut process_ids: [u32; 1024] = [0; 1024];
    let mut cb_needed: u32 = 0;

    unsafe {
        if !K32EnumProcesses(
            &mut process_ids[0],
            size_of_val(&process_ids) as u32,
            &mut cb_needed,
        )
        .as_bool()
        {
            error!("EnumProcesses failed");
            return Vec::new();
        }
    }

    let len: usize = cb_needed as usize / size_of::<u32>();

    return process_ids[..len]
        .iter()
        .filter(|&&pid| pid != 0)
        .cloned()
        .collect();
}

fn process_name(pid: u32) -> Option<OsString> {
    use std::mem::size_of_val;
    use windows_bindings::{
        Windows::Win32::Foundation::{HINSTANCE, MAX_PATH, PWSTR},
        Windows::Win32::System::ProcessStatus::{
            K32EnumProcessModulesEx, K32GetModuleBaseNameW, LIST_MODULES_ALL,
        },
        Windows::Win32::System::Threading::{
            PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
        },
    };

    let h_process = Handle::new(pid, PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE);

    if h_process.0.is_null() {
        warn!("Could not OpenProcess on {}", pid);
        return None;
    } else {
        let mut h_mod: HINSTANCE = HINSTANCE(0);
        let mut cb_needed: u32 = 0;
        unsafe {
            if !K32EnumProcessModulesEx(
                h_process.0,
                &mut h_mod,
                size_of_val(&h_mod) as u32,
                &mut cb_needed,
                LIST_MODULES_ALL,
            )
            .as_bool()
            {
                warn!("Could not K32EnumProcessModulesEx on {}", pid);
                return None;
            } else {
                let mut name: [u16; MAX_PATH as usize] = [0; MAX_PATH as usize];
                K32GetModuleBaseNameW(
                    h_process.0,
                    h_mod,
                    PWSTR(&mut name[0]),
                    size_of_val(&name) as u32,
                );

                return Some(OsString::from_wide(&name[..]));
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

use log::{LevelFilter, SetLoggerError};

static LOGGER: SimpleLogger = SimpleLogger;

pub fn init_log() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Error))
}

fn main() {
    init_log().expect("could not initialize log");

    let mut processes: Vec<(u32, String)> = process_ids()
        .iter()
        .map(|&pid|  (
            pid,
                match process_name(pid) {
                    Some(name) => name.to_string_lossy().to_string(),
                    None => String::from(format!("<PROCESS ID: {}>", pid)),
                }
        ))
        .collect();
    processes.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    let processes = processes;

    for ( id, name ) in processes {
        println!("{}\t{}", id, name);
    }
}
