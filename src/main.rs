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

    let mut buffer = vec![0u8; process.memory_len()].into_boxed_slice();
    let mut results: Vec<usize> = (0..(buffer.len() - size_of::<u32>())).collect();

    loop {
        let mut expected = String::new();
        io::stdin().read_line(&mut expected).unwrap();
        let expected: u32 = expected.trim().parse().unwrap();

        process.read_process_memory(0, &mut buffer);
        results.retain(|&addr| {
            let mut actual = [0u8; size_of::<u32>()];
            actual.copy_from_slice(&buffer[addr..addr + size_of::<u32>()]);

            let expected = expected.to_le_bytes();

            actual == expected
        });

        println!("{} remaining results", results.len());

        let mut file = LineWriter::new(File::create(Path::new("scan.txt")).unwrap());
        for k in &results {
            file.write_all(format!("{:#08x}\t{}", k, expected).as_bytes())
                .unwrap();
            file.write_all(b"\n").unwrap();
        }
    }
}
