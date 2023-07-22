use std::{
    io,
    sync::Mutex,
};

#[cfg(target_os = "linux")]
use linux::{Process, ProcessIterator, MemoryRegionIterator};
#[cfg(target_os = "windows")]
use windows::Process;

use context::{ScannerContext, ProcessDTO, SelectProcessParams, ScanParam, ScanResultParams};

use serde_json;
use jsonrpsee::core::server::RpcModule;

async fn cli() {
    let mut module: RpcModule<Mutex<ScannerContext<Process>>> = RpcModule::new(Mutex::new(ScannerContext::default()));

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
            let count = context.lock().unwrap().new_scan::<MemoryRegionIterator>(parsed);
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
