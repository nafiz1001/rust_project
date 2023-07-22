use std::{
    process::{Child, Command, Stdio},
    sync::Arc,
};

use core::{PID, Process, MemoryRegionIterator};

use scanner::Scanner;
use serde::{Deserialize, Serialize};
use serde_json::Number;

#[derive(Deserialize)]
#[serde(untagged)]
pub enum SelectProcessParams {
    ByPID { pid: PID },
    ByPath { path: String },
}

#[derive(Serialize, Clone)]
pub struct ProcessDTO {
    pid: PID,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanValueType {
    Byte,
    WORD,
    DWORD,
    QWORD,
    Float,
    Double,
}

#[derive(Debug, Deserialize)]
pub struct ScanValue {
    #[serde(rename = "type")]
    pub type_: ScanValueType,
    pub value: Number,
}

#[derive(Serialize, Clone)]
pub struct ScanCount {
    count: usize,
}

#[derive(Deserialize)]
pub struct ScanResultParams {
    pub offset: usize,
    pub limit: usize,
}

#[derive(Serialize, Clone)]
pub struct ScanResultEntry {
    address: usize,
    value: Number,
}

impl ProcessDTO {
    pub fn new<P: Process>(p: &P) -> Self {
        Self {
            pid: p.pid(),
            name: p.name(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ScanParam {
    pub value: ScanValue,
}

pub struct ScannerContext<P>
where P: Process {
    scanner: Option<Scanner<P>>,
    value_type: Option<ScanValueType>,
    signed: Option<bool>,
    child: Option<Child>,
}

impl<P> Default for ScannerContext<P>
where P: Process {
    fn default() -> Self {
        Self { scanner: None, value_type: None, signed: None, child: None }
    }
}

impl<P> ScannerContext<P>
where P: Process {
    pub fn select_process(&mut self, pid: PID) -> ProcessDTO {
        let process = Arc::<P>::new(core::Process::new(pid));
        let scanner = Scanner::new(process.clone());
        self.scanner = Some(scanner);
        ProcessDTO::new(process.as_ref())
    }

    pub fn open_process(&mut self, path: &str) -> ProcessDTO {
        let child = Command::new(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        self.child = Some(child);
        self.select_process(self.child.as_ref().unwrap().id() as PID)
    }

    pub fn new_scan<'a, M>(&'a mut self, scan_param: ScanParam) -> ScanCount
    where M: MemoryRegionIterator<P> {
        let scan_value = scan_param.value;

        let scanner = self.scanner.as_mut().unwrap();

        match scan_value {
            ScanValue {
                type_: ScanValueType::DWORD,
                value,
            } => {
                if value.is_i64() {
                    scanner.new_scan::<i32, _, M>(|&x| {
                        x == i32::try_from(value.as_i64().unwrap()).unwrap()
                    });
                    self.signed = Some(true);
                } else {
                    scanner.new_scan::<u32, _, M>(|&x| {
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

    pub fn next_scan(&mut self, scan_param: ScanParam) -> ScanCount {
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

    pub fn scan_result(&self, scan_result_params: ScanResultParams) -> Vec<ScanResultEntry> {
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