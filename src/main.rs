use serde_json::{json, Map, Value};
use std::{collections::HashMap, io};
use windows_bindings::{Process, ProcessIterator};

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
        &self.last_scan
    }

    pub fn memory(&mut self) -> &[u8] {
        self.update_memory();
        return &self.memory;
    }

    fn update_memory(&mut self) {
        self.process.read_process_memory(0, &mut self.memory);
    }

    pub fn result(&self) -> &[usize] {
        &self.result
    }

    pub fn new_scan<F>(&mut self, f: F) -> &[usize]
    where
        F: Fn(usize, &[u8]) -> bool,
    {
        self.result.clear();
        self.result.extend(0..self.process.memory_len());
        return self.next_scan(|addr, _, mem| f(addr, mem));
    }

    pub fn next_scan<F>(&mut self, f: F) -> &[usize]
    where
        F: Fn(usize, &[u8], &[u8]) -> bool,
    {
        self.update_memory();

        let result: Vec<usize> = self
            .result
            .iter()
            .filter(|&&addr| f(addr, &self.last_scan, &self.memory))
            .cloned()
            .collect();

        self.result.resize(result.len(), 0);
        self.result.copy_from_slice(&result);

        self.last_scan.copy_from_slice(&self.memory);

        return &self.result;
    }
}

fn make_rpc_response(result: Value, id: u64) -> Result<String, serde_json::Error> {
    let mut response = Map::new();
    response.insert("jsonrpc".to_owned(), json!("2.0"));
    response.insert("result".to_owned(), result);
    response.insert("id".to_owned(), json!(id));

    return serde_json::to_string_pretty(&response);
}

fn process_mode(id: u32) {
    let process = Process::new(id);
    let mut scanner = Scanner::new(&process);

    let mut response = Map::new();
    response.insert("id".to_string(), json!(process.id()));
    response.insert("name".to_string(), json!(process.name()));
    println!(
        "{}",
        make_rpc_response(serde_json::to_value(response).unwrap(), 1).unwrap()
    );

    loop {
        let mut rpc = String::new();
        io::stdin().read_line(&mut rpc).unwrap();
        let rpc: HashMap<String, Value> = serde_json::from_str(&rpc).unwrap();

        match rpc.get("method").unwrap().as_str().unwrap() {
            // {"jsonrpc": "2.0", "method": "new_scan", "params": [3, 0, 0, 0], "id": 1}
            "new_scan" => {
                let expected: Vec<u8> = rpc
                    .get("params")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect();

                scanner.new_scan(|addr, mem| {
                    if addr < mem.len() - expected.len() {
                        expected == mem[addr..addr + expected.len()]
                    } else {
                        false
                    }
                });
                println!(
                    "{}",
                    make_rpc_response(
                        scanner
                            .result()
                            .iter()
                            .map(|&addr| serde_json::to_value(addr).unwrap())
                            .collect(),
                        rpc.get("id").unwrap().as_u64().unwrap()
                    )
                    .unwrap()
                );
            }
            // {"jsonrpc": "2.0", "method": "next_scan", "params": [1, 0, 0, 0], "id": 1}
            "next_scan" => {
                let expected: Vec<u8> = rpc
                    .get("params")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect();

                scanner.next_scan(|addr, _, mem| {
                    if addr < mem.len() - expected.len() {
                        expected == mem[addr..addr + expected.len()]
                    } else {
                        false
                    }
                });
                println!(
                    "{}",
                    make_rpc_response(
                        scanner
                            .result()
                            .iter()
                            .map(|&addr| serde_json::to_value(addr).unwrap())
                            .collect(),
                        rpc.get("id").unwrap().as_u64().unwrap()
                    )
                    .unwrap()
                );
            }
            _ => {
                println!("");
            }
        }
    }
}

fn main() {
    loop {
        let mut rpc = String::new();
        io::stdin().read_line(&mut rpc).unwrap();
        let rpc: HashMap<String, Value> = serde_json::from_str(&rpc).unwrap();

        match rpc.get("method").unwrap().as_str().unwrap() {
            // {"jsonrpc": "2.0", "method": "enumerate_processes", "params": [], "id": 1}
            "enumerate_processes" => {
                let result: Value = ProcessIterator::new()
                    .map(|e| {
                        let mut process = Map::new();
                        process.insert("id".to_owned(), json!(e.id()));
                        process.insert("name".to_owned(), json!(e.name()));
                        return process;
                    })
                    .collect();

                println!(
                    "{}",
                    make_rpc_response(result, rpc.get("id").unwrap().as_u64().unwrap()).unwrap()
                );
            }
            // {"jsonrpc": "2.0", "method": "select_process", "params": [pid], "id": 1}
            "select_process" => {
                let id = rpc
                    .get("params")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .next()
                    .unwrap()
                    .as_u64()
                    .unwrap() as u32;
                process_mode(id);
            }
            _ => {
                println!("")
            }
        }
    }
}
