use std::fs::{self, File, ReadDir};
use std::io::{BufRead, BufReader};
use std::ops::Range;
use std::path::PathBuf;

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
            let cols: Vec<String> = self
                .lines
                .next()
                .unwrap()
                .unwrap()
                .split(" ")
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();

            let mut range = cols[0].split("-").map(|s| s.parse().unwrap());
            let range: Range<usize> = range.next().unwrap()..range.next().unwrap();

            return Some(MemoryRegionEntry {
                range,
                info: cols[5].clone(),
                permission: match &cols[1][0..2] {
                    "r-" => MemoryPermission::READONLY,
                    "rw" => MemoryPermission::READWRITE,
                    _ => continue,
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use crate::{MemoryRegionIterator, Process, ProcessIterator};

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
        let mut child = Command::new("/usr/games/moon-buggy")
            .spawn()
            .expect("failed to execute child");

        Process::new(child.id());

        child.kill().unwrap();
    }

    #[test]
    fn memory_region_iterator() {
        let mut child = Command::new("/usr/games/moon-buggy")
            .spawn()
            .expect("failed to execute child");

        for region in MemoryRegionIterator::new(&Process::new(child.id())) {
            println!("{:?}", region);
        }

        child.kill().unwrap();
    }
}
