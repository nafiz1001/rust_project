use std::io;

use jsonrpc::ScannerModule;
#[cfg(target_os = "linux")]
use linux::{Process, ProcessIterator, MemoryRegionIterator};
#[cfg(target_os = "windows")]
use windows::Process;

async fn cli() {
    for line in io::stdin().lines() {
        let module = ScannerModule::<Process, ProcessIterator, MemoryRegionIterator>::default();
        let (response, _) = module.module
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
