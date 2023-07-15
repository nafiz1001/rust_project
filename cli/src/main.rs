use std::{
    io,
    sync::{Arc, Mutex},
};

use core::{self, PID};
#[cfg(target_os = "linux")]
use linux::{Process, ProcessIterator};
#[cfg(target_os = "windows")]
use windows::Process;

use scanner::Scanner;

use jsonrpsee::core::server::RpcModule;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
struct ProcessDTO {
    pid: PID,
    name: String,
}

impl ProcessDTO {
    fn new<P: core::Process>(p: &P) -> Self {
        Self {
            pid: p.pid(),
            name: p.name(),
        }
    }
}

#[derive(Deserialize)]
struct SelectProcessParams {
    pid: PID,
}

async fn cli() {
    let mut module =
        RpcModule::<Arc<Mutex<Option<Scanner<Process>>>>>::new(Arc::new(Mutex::new(None)));
    module
        .register_method("list_processes", |_, _| {
            ProcessIterator::new()
                .map(|p| ProcessDTO::new(&p))
                .collect::<Vec<ProcessDTO>>()
        })
        .unwrap();

    module
        .register_method("select_process", |params, context| {
            let parsed: SelectProcessParams = params.parse().unwrap();
            let process = Arc::<Process>::new(core::Process::new(parsed.pid));
            let scanner = Scanner::new(process.clone());
            *(*context).lock().unwrap() = Some(scanner);
            Some(ProcessDTO::new(process.as_ref()))
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
