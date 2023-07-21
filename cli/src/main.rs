use std::{
    io,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};

#[cfg(target_os = "linux")]
use linux::{Process, ProcessIterator, MemoryRegionIterator};
#[cfg(target_os = "windows")]
use windows::Process;

use core::{self, PID};
use scanner::Scanner;

use serde_json::Number;
use jsonrpsee::core::server::RpcModule;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(untagged)]
enum SelectProcessParams {
    ByPID { pid: PID },
    ByPath { path: String },
}

#[derive(Serialize, Clone)]
struct ProcessDTO {
    pid: PID,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ScanValueType {
    Byte,
    WORD,
    DWORD,
    QWORD,
    Float,
    Double,
}

#[derive(Debug, Deserialize)]
struct ScanValue {
    #[serde(rename = "type")]
    type_: ScanValueType,
    value: Number,
}

#[derive(Serialize, Clone)]
struct ScanCount {
    count: usize,
}

#[derive(Deserialize)]
struct ScanResultParams {
    offset: usize,
    limit: usize,
}

#[derive(Serialize, Clone)]
struct ScanResultEntry {
    address: usize,
    value: Number,
}

impl ProcessDTO {
    fn new<P: core::Process>(p: &P) -> Self {
        Self {
            pid: p.pid(),
            name: p.name(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ScanParam {
    value: ScanValue,
}

struct ScannerContext {
    scanner: Option<Scanner<Process>>,
    value_type: Option<ScanValueType>,
    signed: Option<bool>,
    child: Option<Child>,
}

impl ScannerContext {
    fn select_process(&mut self, pid: PID) -> ProcessDTO {
        let process = Arc::<Process>::new(core::Process::new(pid));
        let scanner = Scanner::new(process.clone());
        self.scanner = Some(scanner);
        ProcessDTO::new(process.as_ref())
    }

    fn open_process(&mut self, path: &str) -> ProcessDTO {
        let child = Command::new(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        self.child = Some(child);
        self.select_process(self.child.as_ref().unwrap().id() as PID)
    }

    fn new_scan(&mut self, scan_param: ScanParam) -> ScanCount {
        let scan_value = scan_param.value;

        let scanner = self.scanner.as_mut().unwrap();

        match scan_value {
            ScanValue {
                type_: ScanValueType::DWORD,
                value,
            } => {
                if value.is_i64() {
                    scanner.new_scan::<i32, _, MemoryRegionIterator>(|&x| {
                        x == i32::try_from(value.as_i64().unwrap()).unwrap()
                    });
                    self.signed = Some(true);
                } else {
                    scanner.new_scan::<u32, _, MemoryRegionIterator>(|&x| {
                        x == u32::try_from(value.as_u64().unwrap()).unwrap()
                    });
                    self.signed = Some(false);
                }
            }
            _ => panic!("{:#?} not supported", scan_value.type_),
        }
        self.value_type = Some(scan_value.type_);

        ScanCount {
            count: scanner.get_addresses().len(),
        }
    }

    fn next_scan(&mut self, scan_param: ScanParam) -> ScanCount {
        let scan_value = scan_param.value;

        let scanner = self.scanner.as_mut().unwrap();

        match scan_value {
            ScanValue {
                type_: ScanValueType::DWORD,
                value,
            } => {
                if value.is_i64() {
                    scanner.next_scan::<i32, _>(|&x| {
                        x == i32::try_from(value.as_i64().unwrap()).unwrap()
                    });
                    self.signed = Some(true);
                } else {
                    scanner.next_scan::<u32, _>(|&x| {
                        x == u32::try_from(value.as_u64().unwrap()).unwrap()
                    });
                    self.signed = Some(false);
                }
            }
            _ => panic!("{:#?} not supported", scan_value.type_),
        }
        self.value_type = Some(scan_value.type_);

        ScanCount {
            count: scanner.get_addresses().len(),
        }
    }

    fn scan_result(&self, scan_result_params: ScanResultParams) -> Vec<ScanResultEntry> {
        let ScanResultParams { offset, limit } = scan_result_params;

        match (self.value_type.as_ref().unwrap(), self.signed.unwrap()) {
            (ScanValueType::DWORD, true) => self
                .scanner
                .as_ref()
                .unwrap()
                .scan_result::<i32>(offset, limit)
                .into_iter()
                .map(|(address, value)| ScanResultEntry {
                    address,
                    value: Number::from(value),
                })
                .collect::<Vec<_>>(),
            x => panic!("({:#?}, {:#?}) not supported", x.0, x.1),
        }
    }
}

async fn cli() {
    let mut module = RpcModule::new(Mutex::new(ScannerContext {
        scanner: None,
        value_type: None,
        signed: None,
        child: None,
    }));

    // {"jsonrpc": "2.0", "method": "list_processes", "id": 1}
    module
        .register_method("list_processes", |_, _| {
            ProcessIterator::new()
                .map(|p| ProcessDTO::new(&p))
                .collect::<Vec<ProcessDTO>>()
        })
        .unwrap();

    // {"jsonrpc": "2.0", "method": "select_process", "params": { "pid": 359907 }, "id": 1}
    module
        .register_method("select_process", |params, context| {
            let parsed: SelectProcessParams = params.parse().unwrap();
            let process = match parsed {
                SelectProcessParams::ByPID { pid } => context.lock().unwrap().select_process(pid),
                SelectProcessParams::ByPath { path } => context.lock().unwrap().open_process(&path),
            };
            serde_json::to_value(process).unwrap()
        })
        .unwrap();

    // {"jsonrpc": "2.0", "method": "new_scan", "params": { "value": { "type": "dword", "value": 100 } }, "id": 1}
    module
        .register_blocking_method("new_scan", |params, context| {
            let parsed: ScanParam = params.parse().unwrap();
            let count = context.lock().unwrap().new_scan(parsed);
            serde_json::to_value(count).unwrap()
        })
        .unwrap();

    // {"jsonrpc": "2.0", "method": "next_scan", "params": { "value": { "type": "dword", "value": 103 } }, "id": 1}
    module
        .register_blocking_method("next_scan", |params, context| {
            let parsed: ScanParam = params.parse().unwrap();
            let count = context.lock().unwrap().next_scan(parsed);
            serde_json::to_value(count).unwrap()
        })
        .unwrap();

    // {"jsonrpc": "2.0", "method": "scan_result", "params": { "offset": 0, "limit": 2 }, "id": 1}
    module
        .register_blocking_method("scan_result", |params, context| {
            let parsed: ScanResultParams = params.parse().unwrap();
            context.lock().unwrap().scan_result(parsed)
        })
        .unwrap();

    for line in io::stdin().lines() {
        let (response, _) = module
            .raw_json_request(line.unwrap().as_str(), 1)
            .await
            .unwrap();
        println!("{}", response.result);
    }
}

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    rt.block_on(async {
        cli().await;
    })
}
