# "Cheat Engine" in Rust (WIP)

Read and update bytes in other processes (currently only supports 32-bit integer).

## Words of warning

Currently, the Windows version will not run because I've been using mainly Linux while making significant refactoring.

## Getting Started

### Usage

```
$ cd cli
$ cargo build
$ cargo run
Enter process path: /path/to/program # in my case doukutsu-rs.x86_64.elf
> new_scan 4
Scan done!
> next_scan 3
Scan done!
> result_scan
0x562f5e2b8d8c  3
0x562f5e31056c  3
All result printed!
> set_value 562f5e2b8d8c 10
wrote 10 at address 0x562f5e2b8d8c
```
