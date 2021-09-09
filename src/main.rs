extern crate winapi;
use std::io::{Error, ErrorKind};
use std::iter::once;
#[allow(unused_imports)] use log::error;

fn enumerate_processes() -> Result<Vec<u32>, Error> {
    use std::mem::size_of;
    use winapi::um::psapi::EnumProcesses;
    use winapi::shared::minwindef::DWORD;
    
    let mut a_processes: [DWORD; 1024] = [0; 1024];
    let mut cb_needed: DWORD = 0;

    unsafe {
        if EnumProcesses(&mut a_processes[0] as *mut DWORD, (1024 * size_of::<DWORD>()) as u32, &mut cb_needed) == 0 {
            error!("EnumProcesses failed");
            return Err(Error::new(ErrorKind::Other, "EnumProcesses failed"));
        }
    }

    let len: usize = cb_needed as usize / size_of::<DWORD>();

    Ok(a_processes[..len].iter().filter(|&&pid| pid != 0).cloned().collect())
}

fn entry() -> Result<i32, Error> {
    use std::mem::size_of;
    use std::ptr::null_mut;

    use winapi::um::psapi::{EnumProcessModules, GetModuleBaseNameA};
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winnt::{PROCESS_QUERY_INFORMATION, PROCESS_VM_READ, CHAR};
    use winapi::shared::minwindef::{DWORD, FALSE, HMODULE, MAX_PATH};

    match enumerate_processes() {
        Ok(process_ids) => {
            let mut processes: Vec<(u32, String)> = Vec::new();

            for process_id in process_ids {
                unsafe {
                    let h_process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, process_id);

                    if !h_process.is_null() {
                        let mut h_mod: HMODULE = null_mut();
                        let mut cb_needed: DWORD = 0;

                        if EnumProcessModules(h_process, &mut h_mod, size_of::<HMODULE>() as u32, &mut cb_needed) != 0 {
                            let mut process_name: [CHAR; MAX_PATH] = [0; MAX_PATH];

                            if GetModuleBaseNameA(h_process, h_mod, &mut process_name[0], process_name.len() as u32) != 0 {
                                let process_name: String = process_name.iter().filter(|&&c| c != 0).chain(once(&0)).map(|&c| c as u8 as char).collect();
                                processes.push((process_id, process_name));
                            } else {
                                error!("Could not get process name of {}",  process_id);
                            }
                        }
                    }
                }
            }

            processes.sort_by(|a, b| a.0.cmp(&b.0));

            for (pid, name) in processes {
                println!("{}\t{}", pid, name);
            }

            Ok(0)
        },
        Err(err) => Err(err),
    }
}

 fn main() -> Result<(), Error> {
    match entry() {
        Ok(_) => Ok(()),
        Err(err) => {
            error!("{}", err);
            Err(err)
        }
    }
}
