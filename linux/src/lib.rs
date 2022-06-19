use std::fs::{self, File, ReadDir};
use std::io::{BufRead, BufReader, IoSlice, IoSliceMut};
use std::mem::size_of;
use std::ops::Range;
use std::path::PathBuf;

use nix::sys::uio::{process_vm_readv, process_vm_writev, RemoteIoVec};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};

#[derive(Debug)]
pub struct Process {
    proc_path: PathBuf,
    pid: i64,
}

impl Process {
    pub fn new(pid: i64) -> Self {
        Self {
            proc_path: ["/proc", &pid.to_string()].iter().collect(),
            pid,
        }
    }

    pub fn pid(&self) -> i64 {
        self.pid as i64
    }

    pub fn name(&self) -> String {
        fs::read_to_string(self.proc_path.join("comm"))
            .unwrap()
            .trim()
            .to_string()
    }

    pub fn attach(&self) -> Result<(), String> {
        use nix::{sys::ptrace, unistd::Pid};

        let pid = Pid::from_raw(self.pid() as i32);

        ptrace::attach(pid).map_err(|op| op.desc().to_string())?;

        match waitpid(pid, Some(WaitPidFlag::WSTOPPED)) {
            Ok(WaitStatus::Stopped(_, _)) => Ok(()),
            Ok(x) => Err(format!("waitpid returned {:?}", x)),
            Err(x) => Err(format!("waitpid returned {:?}", x)),
        }
    }

    pub fn detach(&self) -> Result<(), String> {
        use nix::{
            sys::{ptrace, signal::Signal},
            unistd::Pid,
        };

        ptrace::detach(Pid::from_raw(self.pid() as i32), Signal::SIGCONT)
            .map_err(|op| op.desc().to_string())?;

        // TODO: properly waitpid
        // match waitpid(pid, Some(WaitPidFlag::WCONTINUED)) {
        //     Ok(WaitStatus::Continued(_)) => Ok(()),
        //     Ok(x) => Err(format!("waitpid returned {:?}", x)),
        //     Err(x) => Err(format!("waitpid returned {:?}", x)),
        // }
        return Ok(())
    }

    pub fn read_memory<T>(&self, start: usize, buffer: &mut [T]) -> Result<(), String> {
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
                Err(errno) => Err(errno.desc().to_string()),
            }
        }
    }

    pub fn write_memory<T>(&self, start: usize, buffer: &[T]) -> Result<(), String> {
        use nix::unistd::Pid;

        unsafe {
            let bytes = std::slice::from_raw_parts(
                buffer.as_ptr() as *const u8,
                buffer.len() * size_of::<T>(),
            );

            let local = [IoSlice::new(bytes); 1];
            let remote = [RemoteIoVec {
                base: start,
                len: bytes.len(),
            }; 1];

            match process_vm_writev(Pid::from_raw(self.pid() as i32), &local, &remote) {
                Ok(_) => Ok(()),
                Err(errno) => Err(errno.desc().to_string()),
            }
        }
    }
}

pub struct ProcessIterator {
    dirs: ReadDir,
}

impl Default for ProcessIterator {
    fn default() -> Self {
        Self::new()
    }
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
            .map(|pid| Process::new(pid as i64))
    }
}

pub enum MemoryPermission {
    READONLY,
    READWRITE,
    NONE,
}

pub enum MemoryKind {
    STACK,
    HEAP,
    UNKNOWN,
}

pub struct MemoryRegion {
    pub range: Range<usize>,
    pub permission: MemoryPermission,
    pub kind: MemoryKind,
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
    type Item = MemoryRegion;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line: String = self.lines.next()?.unwrap().trim().to_string();

            let mut range = line.split(' ').nth(0).unwrap().split('-');
            let range = usize::from_str_radix(range.next().unwrap(), 16).unwrap()
                ..usize::from_str_radix(range.next().unwrap(), 16).unwrap();

            if range.start >= self.starting_address {
                let permission = match &line.split(' ').nth(1).unwrap()[0..2] {
                    "r-" => MemoryPermission::READONLY,
                    "rw" => MemoryPermission::READWRITE,
                    _ => MemoryPermission::NONE,
                };

                let info = line
                    .split(' ')
                    .skip(5)
                    .find(|s| !s.is_empty())
                    .unwrap_or("");

                let kind = if info.contains("stack") {
                    MemoryKind::STACK
                } else if info.contains("heap") {
                    MemoryKind::HEAP
                } else if info.contains(self.process.name().as_str()) {
                    MemoryKind::UNKNOWN
                } else {
                    continue;
                };

                return Some(MemoryRegion {
                    range,
                    permission,
                    kind,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Child, Command, Stdio};

    use crate::{MemoryPermission, MemoryRegionIterator, Process, ProcessIterator};

    fn create_child() -> Child {
        Command::new("/usr/bin/sleep")
            .arg("10000")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to execute child")
    }

    #[test]
    fn enumerate_processes() {
        assert!(
            ProcessIterator::new()
                .inspect(|p| println!("{:?}", p))
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
    fn attach_detach_process() {
        let mut child = create_child();

        let process = Process::new(child.id() as i64);

        process.attach().unwrap();
        process.detach().unwrap();

        child.kill().unwrap();
    }

    #[test]
    fn memory_region_iterator() {
        let mut child = create_child();

        let process = Process::new(child.id() as i64);

        for _ in MemoryRegionIterator::new(&process, 0) {}

        child.kill().unwrap();
    }

    #[test]
    fn read_process_memory() {
        let mut child = create_child();

        let process = Process::new(child.id() as i64);

        // process.attach().unwrap();
        for region in MemoryRegionIterator::new(&process, 0) {
            match region.permission {
                MemoryPermission::READONLY | MemoryPermission::READWRITE => {
                    let mut buffer = vec![0u8; region.range.len()];
                    process
                        .read_memory(region.range.start, &mut buffer)
                        .unwrap();
                    buffer.len();
                }
                _ => {}
            }
        }
        // process.detach().unwrap();

        child.kill().unwrap();
    }
}
