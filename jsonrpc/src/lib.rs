use core::{MemoryRegionIterator, Process, ProcessIterator};

use std::{marker::PhantomData, sync::Mutex};

use context::{ProcessDTO, ScanParam, ScanResultParams, ScannerContext, SelectProcessParams};

use jsonrpsee::core::server::RpcModule;
use serde_json;

pub struct ScannerModule<P, ProcessIter, MemoryRegionIter>
where
    P: Process,
    ProcessIter: ProcessIterator<P>,
    MemoryRegionIter: MemoryRegionIterator<P>,
{
    pub module: RpcModule<Mutex<ScannerContext<P>>>,
    process_iter: PhantomData<ProcessIter>,
    memory_region_iter: PhantomData<MemoryRegionIter>,
}

impl<P, ProcessIter, MemoryRegionIter> Default for ScannerModule<P, ProcessIter, MemoryRegionIter>
where
    P: Process + 'static,
    ProcessIter: ProcessIterator<P>,
    MemoryRegionIter: MemoryRegionIterator<P>,
{
    fn default() -> Self
    where
        ProcessIter: ProcessIterator<P>,
        MemoryRegionIter: MemoryRegionIterator<P>,
    {
        let mut module: RpcModule<Mutex<ScannerContext<P>>> =
            RpcModule::new(Mutex::new(ScannerContext::default()));

        // {"jsonrpc": "2.0", "method": "list_processes", "id": 1}
        module
            .register_blocking_method("list_processes", |_, _| {
                ProcessIter::new()
                    .map(|p| ProcessDTO::new(&p))
                    .collect::<Vec<ProcessDTO>>()
            })
            .unwrap();

        // {"jsonrpc": "2.0", "method": "select_process", "params": { "pid": 359907 }, "id": 1}
        module
            .register_blocking_method("select_process", |params, context| {
                let parsed: SelectProcessParams = params.parse().unwrap();
                let process = match parsed {
                    SelectProcessParams::ByPID { pid } => {
                        context.lock().unwrap().select_process(pid)
                    }
                    SelectProcessParams::ByPath { path } => {
                        context.lock().unwrap().open_process(&path)
                    }
                };
                serde_json::to_value(process).unwrap()
            })
            .unwrap();

        // {"jsonrpc": "2.0", "method": "new_scan", "params": { "value": { "type": "dword", "value": 100 } }, "id": 1}
        module
            .register_blocking_method("new_scan", |params, context| {
                let parsed: ScanParam = params.parse().unwrap();
                let count = context.lock().unwrap().new_scan::<MemoryRegionIter>(parsed);
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

        Self { module, process_iter: PhantomData, memory_region_iter: PhantomData }
    }
}
