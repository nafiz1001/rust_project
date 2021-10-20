use std::fs::{self, File, ReadDir};
use std::io::{BufRead, BufReader};
use std::ops::Range;
use std::path::PathBuf;

use nix::libc::{iovec, preadv};
use nix::sys::uio::IoVec;

#[derive(Debug)]
pub struct Process {
    path: PathBuf,
    pid: u32,
}

impl Process {
    pub fn new(pid: u32) -> Self {
        Self {
            path: ["/proc", &pid.to_string()].iter().collect(),
            pid,
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn name(&self) -> String {
        fs::read_to_string(self.path.join("comm"))
            .unwrap()
            .trim()
            .to_owned()
    }

    pub fn read_process_memory(&self, start: usize, buffer: &mut [u8]) {
        use std::os::unix::io::AsRawFd;

        let file =  File::open(self.path.join("mem")).unwrap();
        let iov = [IoVec::from_mut_slice(buffer); 1];

        unsafe {
            preadv(file.as_raw_fd(), iov.as_ptr() as *const iovec, 1, start as i64);
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
            .and_then(|pid| Some(Process::new(pid)))
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
    range: Range<usize>,
    permission: MemoryPermission,
    info: String,
}

pub struct MemoryRegionIterator {
    lines: std::io::Lines<BufReader<File>>,
}

impl MemoryRegionIterator {
    pub fn new(process: &Process) -> Self {
        Self {
            lines: BufReader::new(File::open(process.path.join("maps")).unwrap()).lines(),
        }
    }
}

impl Iterator for MemoryRegionIterator {
    type Item = MemoryRegionEntry;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line = self.lines.next()?.unwrap().trim().to_string();

            let mut range = line.split(" ").nth(0).unwrap().split("-");
            let range = usize::from_str_radix(range.next().unwrap(), 16).unwrap()
                ..usize::from_str_radix(range.next().unwrap(), 16).unwrap();

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
                .or(Some(""))
                .unwrap()
                .to_string();

            return Some(MemoryRegionEntry {
                range,
                permission,
                info,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Child, Command, Stdio};

    use nix::{sys::{ptrace, signal::Signal}, unistd::Pid};

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

        Process::new(child.id());

        child.kill().unwrap();
    }

    #[test]
    fn memory_region_iterator() {
        let mut child = create_child();

        let process = Process::new(child.id());

        //ptrace::attach(Pid::from_raw(child.id() as i32)).unwrap();
        for region in MemoryRegionIterator::new(&process) {
            println!("{:?}", region);
        }
        //ptrace::detach(Pid::from_raw(child.id() as i32), Signal::SIGCONT).unwrap();

        child.kill().unwrap();
    }

    #[test]
    fn read_process_memory() {
        let mut child = create_child();

        let process = Process::new(child.id());
        
        ptrace::attach(Pid::from_raw(child.id() as i32)).unwrap();
        for region in MemoryRegionIterator::new(&process) {
            match region.permission {
                MemoryPermission::READONLY | MemoryPermission::READWRITE => {
                    let mut buffer = vec![0u8; region.range.len()];
                    process.read_process_memory(region.range.start, &mut buffer);
                    buffer.iter().count();
                },
                _ => {}
            }
        }
        ptrace::detach(Pid::from_raw(child.id() as i32), Signal::SIGCONT).unwrap();

        child.kill().unwrap();
    }
}
