use std::fs::{self, ReadDir};
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

#[cfg(test)]
mod tests {
    use std::{fs::File, process::Command};

    use crate::{ProcessIterator, Process};

    #[test]
    fn enumerate_processes() {
        assert!(ProcessIterator::new().map(|p| {
            println!("{:?}", p);
            return p
        }).count() > 0);
    }

    #[test]
    fn new_process() {
        let mut child = Command::new("/usr/games/moon-buggy")
                        .spawn()
                        .expect("failed to execute child");

        Process::new(child.id());

        child.kill().unwrap();
    }
}
