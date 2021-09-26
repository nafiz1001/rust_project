use serde_json::{Map, Value, json};
use windows_bindings::ProcessIterator;
use std::{collections::HashMap, io};

fn make_rpc_response(result: Value, id: u64) -> Result<String, serde_json::Error> {
    let mut response = Map::new();
    response.insert("jsonrpc".to_owned(), json!("2.0"));
    response.insert("result".to_owned(), result);
    response.insert("id".to_owned(), json!(id));

    return serde_json::to_string(&response);
}

fn main() {
    loop {
        let mut rpc = String::new();
        io::stdin().read_line(&mut rpc).unwrap();

        let rpc: HashMap<String, Value> = serde_json::from_str(&rpc).unwrap();

        match rpc.get("method").unwrap().as_str().unwrap() {
            "enumerate_processes" => {
                let result: Value = ProcessIterator::new().map(|e| {
                    let mut process = Map::new();
                    process.insert("id".to_owned(), json!(e.id()));
                    process.insert("name".to_owned(), json!(e.name()));
                    return process
                }).collect();

                println!("{}", make_rpc_response(result, rpc.get("id").unwrap().as_u64().unwrap()).unwrap());
            },
            _ => {}
        }
    }
}
