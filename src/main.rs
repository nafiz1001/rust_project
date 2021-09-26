#![allow(non_snake_case)]

use std::{
    collections::HashMap,
    fs::File,
    io::{self, LineWriter, Write},
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

    let mut bytes = [0u8; 4];

    let mut matches = HashMap::<usize, u32>::new();

    for k in 0..(buffer.len() - 4) {
        bytes.copy_from_slice(&buffer[k..k + 4]);
        let v = u32::from_le_bytes(bytes);

        matches.insert(k, v);
    }

    loop {
        let mut expected = String::new();
        io::stdin().read_line(&mut expected).unwrap();
        let expected: u32 = expected.trim().parse().unwrap();

        process.read_process_memory(0, &mut buffer);

        for k in 0..(buffer.len() - 4) {
            bytes.copy_from_slice(&buffer[k..k + 4]);
            let v = u32::from_le_bytes(bytes);

            if v != expected {
                matches.remove(&k);
            } else {
                if matches.contains_key(&k) {
                    matches.insert(k, v);
                }
            }
        }

        println!("{} remaining results", matches.len());

        let mut file = LineWriter::new(File::create(Path::new("scan.txt")).unwrap());
        for (k, v) in matches.iter() {
            file.write_all(format!("{:#08x}\t{}", k, v).as_bytes())
                .unwrap();
            file.write_all(b"\n").unwrap();
        }
    }
}
