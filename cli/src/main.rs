use std::{
    io::{self, BufRead, Write},
    process::Command,
};

#[cfg(target_os = "linux")]
use linux::Process;
#[cfg(target_os = "windows")]
use windows::Process;

use scanner::Scanner;

fn cli() {
    print!("Enter process path: ");
    io::stdout().flush().unwrap();
    let stdin = io::stdin();
    let mut buf = String::new();
    stdin.read_line(&mut buf).unwrap();

    let path = buf.trim();
    let child = Command::new(path).spawn().unwrap();
    let process = Process::new(child.id());

    let mut scanner = Scanner::new(&process);

    let mut line_processor = |line: &str| -> Result<String, String> {
        match line
        .split(" ")
        .nth(0)
        .ok_or("expected at least one argument".to_string())? {
            "new_scan" => {
                let expected = line
                    .split(" ")
                    .nth(1)
                    .ok_or("new_scan [int]".to_string())?
                    .parse::<i32>()
                    .or(Err("int argument could not be parsed as 32 bit int".to_string()))?;
                scanner.new_scan(expected);
                Ok("Scan done!".to_string())
            }
            "next_scan" => {
                let expected = line
                    .split(" ")
                    .nth(1)
                    .ok_or("next_scan [int]".to_string())?
                    .parse::<i32>()
                    .or(Err("int argument could not be parsed as 32 bit int".to_string()))?;
                scanner.next_scan(expected);
                Ok("Scan done!".to_string())
            }
            "result_scan" => {
                for &address in scanner.get_addresses().iter() {
                    let mut value = [0];
                    let value = match process.read_process_memory(address, &mut value) {
                        Ok(_) => value[0],
                        Err(_) => continue,
                    };
                    println!("{:#08x}\t{}", address, value);
                }
                Ok("All result printed!".to_string())
            }
            "set_value" => {
                let address = usize::from_str_radix(
                    line
                        .split(" ")
                        .nth(1)
                        .ok_or("set_value [address in hex] [int]".to_string())?,
                    16)
                    .or(Err("address argument could not be parsed as hexadecimal int"))?;

                let value: i32 = line
                    .split(" ")
                    .nth(2)
                    .ok_or("set_value [address] [int]".to_string())?
                    .parse()
                    .or(Err("int argument could not be parsed as 32 bit int".to_string()))?;
                let value = [value];

                match  process.write_process_memory(address, &value) {
                    Ok(_) => Ok(format!("wrote {} at address {:#08x}", value[0], address)),
                    Err(_) => Err(format!("could not write to {:#08x}", address)),
                }
            }
            "get_value" => {
                let address = usize::from_str_radix(
                    line
                        .split(" ")
                        .nth(1)
                        .ok_or("get_value [address in hex]".to_string())?,
                    16)
                    .or(Err("address argument could not be parsed as hexadecimal int"))?;

                let mut value = [0];
                match  process.read_process_memory(address, &mut value) {
                    Ok(_) => Ok(value[0].to_string()),
                    Err(_) => Err(format!("could not read at {:#08x}", address)),
                }
            }
            _ => {
                return Err("The only supported operations are: new_scan [int], next_scan [int], result_scan, get_value [address in hex] and set_value [address in hex] [int]".to_string())
            }
        }
    };

    println!("The supported operations are: new_scan [int], next_scan [int], result_scan, get_value [address in hex] and set_value [address in hex] [int]");
    print!("> ");
    io::stdout().flush().unwrap();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        match line_processor(&line) {
            Ok(s) => {
                println!("{}", s)
            }
            Err(err) => {
                println!("{}", err)
            }
        }
        print!("> ");
        io::stdout().flush().unwrap();
    }
}

fn main() {
    cli();
}
