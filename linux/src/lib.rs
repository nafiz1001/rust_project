use std::fs::{self, File, ReadDir};
use std::io::{BufRead, BufReader, IoSlice, IoSliceMut};
use std::mem::size_of;
use std::ops::Range;
use std::path::PathBuf;
use core::ProcessInterface;

use nix::sys::uio::{process_vm_readv, process_vm_writev, RemoteIoVec};

#[derive(Debug)]
pub struct Process {
    proc_path: PathBuf,
    pid: i64,
}

impl ProcessInterface for Process {
    fn new(pid: i64) -> Self {
        Self {
            proc_path: ["/proc", &pid.to_string()].iter().collect(),
            pid,
        }
    }

    fn pid(&self) -> i64 {
        self.pid as i64
    }

    fn name(&self) -> String {
        fs::read_to_string(self.proc_path.join("comm"))
            .unwrap()
            .trim()
            .to_string()
    }

    fn attach(&self) -> Result<(), String> {
        use nix::{sys::ptrace, unistd::Pid};
        ptrace::attach(Pid::from_raw(self.pid() as i32)).map_err(|op| op.desc().to_string())
    }

    fn detach(&self) -> Result<(), String> {
        use nix::{
            sys::{ptrace, signal::Signal},
            unistd::Pid,
        };
        ptrace::detach(Pid::from_raw(self.pid() as i32), Signal::SIGCONT)
            .map_err(|op| op.desc().to_string())
    }

    fn read_memory<T>(&self, start: usize, buffer: &mut [T]) -> Result<(), String> {
        use nix::unistd::Pid;

        unsafe {
            let bytes = std::slice::from_raw_parts_mut(
                buffer.as_ptr() as *mut u8,
                buffer.len() * size_of::<T>(),
            );
            let len = bytes.len();

            let mut local = [IoSliceMut::new(bytes); 1];
            let remote = [RemoteIoVec { base: start, len }; 1];

            match process_vm_readv(Pid::from_raw(self.pid() as i32), &mut local, &remote) {
                Ok(_) => Ok(()),
                Err(errno) => Err(errno.desc().to_string())
            }
        }
    }

    fn write_memory<T>(&self, start: usize, buffer: &[T]) -> Result<(), String> {
        use nix::unistd::Pid;

        unsafe {
            let bytes = std::slice::from_raw_parts(
                buffer.as_ptr() as *const u8,
                buffer.len() * size_of::<T>(),
            );

            let mut local = [IoSlice::new(bytes); 1];
            let remote = [RemoteIoVec {
                base: start,
                len: bytes.len(),
            }; 1];

            match process_vm_writev(Pid::from_raw(self.pid() as i32), &mut local, &remote) {
                Ok(_) => Ok(()),
                Err(errno) => {
                    Err(errno.desc().to_string())
                }
            }
        }
    }
}

pub struct ProcessIterator {
    dirs: ReadDir,
}

impl ProcessIterator {
    pub fn new() -> Self {
        Self {
            dirs: fs::read_dir("/proc").unwrap(),
        }
    }
}

impl Iterator for ProcessIterator {
    type Item = Process;

    fn next(&mut self) -> Option<Self::Item> {
        self.dirs
            .find_map(|dir| dir.ok()?.file_name().to_string_lossy().parse::<u32>().ok())
            .and_then(|pid| Some(Process::new(pid as i64)))
    }
}

#[derive(Debug)]
pub enum MemoryPermission {
    READONLY,
    READWRITE,
    NONE,
}

#[derive(Debug)]
pub struct MemoryRegionEntry {
    pub range: Range<usize>,
    pub permission: MemoryPermission,
}

pub struct MemoryRegionIterator<'a> {
    lines: std::io::Lines<BufReader<File>>,
    starting_address: usize,
    process: &'a Process,
}

impl<'a> MemoryRegionIterator<'a> {
    pub fn new(process: &'a Process, starting_address: usize) -> Self {
        Self {
            lines: BufReader::new(File::open(process.proc_path.join("maps")).unwrap()).lines(),
            starting_address,
            process,
        }
    }
}

impl<'a> Iterator for MemoryRegionIterator<'a> {
    type Item = MemoryRegionEntry;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line = self.lines.next()?.unwrap().trim().to_string();

            let mut range = line.split(" ").nth(0).unwrap().split("-");
            let range = usize::from_str_radix(range.next().unwrap(), 16).unwrap()
                ..usize::from_str_radix(range.next().unwrap(), 16).unwrap();

            if range.start >= self.starting_address {
                let permission = match &line.split(" ").nth(1).unwrap()[0..2] {
                    "r-" => MemoryPermission::READONLY,
                    "rw" => MemoryPermission::READWRITE,
                    _ => MemoryPermission::NONE,
                };

                let info = line
                    .split(" ")
                    .skip(5)
                    .skip_while(|s| s.is_empty())
                    .next()
                    .unwrap_or("");

                let patterns = ["stack".to_string(), "heap".to_string(), self.process.name()];
                if patterns.iter().any(|p| info.contains(p)) {
                    return Some(MemoryRegionEntry { range, permission });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Child, Command, Stdio};

    use core::ProcessInterface;
    use crate::{MemoryPermission, MemoryRegionIterator, Process, ProcessIterator};

    fn create_child() -> Child {
        Command::new("/usr/bin/sleep")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to execute child")
    }

    #[test]
    fn enumerate_processes() {
        assert!(
            ProcessIterator::new()
                .map(|p| {
                    println!("{:?}", p);
                    return p;
                })
                .count()
                > 0
        );
    }

    #[test]
    fn new_process() {
        let mut child = create_child();

        Process::new(child.id() as i64);

        child.kill().unwrap();
    }

    #[test]
    fn memory_region_iterator() {
        let mut child = create_child();

        let process = Process::new(child.id() as i64);

        //ptrace::attach(Pid::from_raw(child.id() as i32)).unwrap();
        for region in MemoryRegionIterator::new(&process, 0) {
            println!("{:?}", region);
        }
        //ptrace::detach(Pid::from_raw(child.id() as i32), Signal::SIGCONT).unwrap();

        child.kill().unwrap();
    }

    #[test]
    fn read_process_memory() {
        let mut child = create_child();

        let process = Process::new(child.id() as i64);

        process.attach().unwrap();
        for region in MemoryRegionIterator::new(&process, 0) {
            match region.permission {
                MemoryPermission::READONLY | MemoryPermission::READWRITE => {
                    let mut buffer = vec![0u8; region.range.len()];
                    process
                        .read_memory(region.range.start, &mut buffer)
                        .unwrap();
                    buffer.iter().count();
                }
                _ => {}
            }
        }
        process.detach().unwrap();

        child.kill().unwrap();
    }
}
