#![allow(non_snake_case)]

use std::{
    fs::File,
    io::{self, LineWriter, Write},
    mem::size_of,
    path::Path,
};

use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use windows_bindings::{Process, ProcessIterator};

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

pub struct Scanner<'a> {
    process: &'a Process,
    last_scan: Box<[u8]>,
    memory: Box<[u8]>,
    result: Vec<usize>,
}

impl<'a> Scanner<'a> {
    pub fn new(process: &'a Process) -> Self {
        Self {
            process,
            last_scan: vec![0u8; process.memory_len()].into_boxed_slice(),
            memory: vec![0u8; process.memory_len()].into_boxed_slice(),
            result: Vec::with_capacity(process.memory_len()),
        }
    }

    pub fn last_scan(&self) -> &[u8] {
        &self.last_scan[..]
    }

    pub fn memory(&mut self) -> &[u8] {
        self.update_memory();
        return &self.memory[..];
    }

    fn update_memory(&mut self) {
        self.process.read_process_memory(0, &mut self.memory[..]);
    }

    pub fn result(&self) -> &[usize] {
        &self.result[..]
    }

    pub fn new_scan<F: Fn(&[u8], &[u8]) -> bool>(&mut self, f: F, item_len: usize) -> &[usize] {
        self.result.clear();
        self.result.extend(0..self.process.memory_len() - item_len);
        return self.next_scan(f, item_len);
    }

    pub fn next_scan<F: Fn(&[u8], &[u8]) -> bool>(
        &mut self,
        f: F,
        item_len: usize,
    ) -> &[usize] {
        self.update_memory();

        let result: Vec<usize> = self
            .result
            .iter()
            .filter(|&&addr| {
                f(
                    &self.last_scan[addr..addr + item_len],
                    &self.memory[addr..addr + item_len],
                )
            })
            .cloned()
            .collect();

        self.result.resize(result.len(), 0);
        self.result.copy_from_slice(&result[..]);

        self.last_scan.copy_from_slice(&self.memory[..]);

        return &self.result[..];
    }
}

fn main() {
    init_log().expect("could not initialize log");

    let mut processes: Vec<_> = ProcessIterator::new()
        .map(|entry| (entry.id(), entry.name()))
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

    let process = Process::new(pid);
    let mut scanner = Scanner::new(&process);

    for i in 0.. {
        let mut expected = String::new();
        io::stdin().read_line(&mut expected).unwrap();
        let expected: u32 = expected.trim().parse().unwrap();

        if i == 0 {
            scanner.new_scan(
                |_, new| {
                    let mut bytes = [0u8; 4];
                    bytes.copy_from_slice(new);
                    let actual = u32::from_le_bytes(bytes);

                    actual == expected
                },
                size_of::<u32>(),
            );
        } else {
            scanner.next_scan(
                |_, new| {
                    let mut bytes = [0u8; 4];
                    bytes.copy_from_slice(new);
                    let actual = u32::from_le_bytes(bytes);

                    actual == expected
                },
                size_of::<u32>(),
            );
        }

        let mut file = LineWriter::new(File::create(Path::new("scan.txt")).unwrap());
        for &k in scanner.result() {
            file.write_all(format!("{:#08x}\t{}", k, expected).as_bytes())
                .unwrap();
            file.write_all(b"\n").unwrap();
        }
    }
}
