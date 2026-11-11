use nexa_inspect::inspect;
use std::{env, fs, process};

fn usage() {
    eprintln!("Nexa PE Inspector — read-only Windows PE metadata inspection");
    eprintln!("Usage: nexa-inspect inspect [--json] <file.exe>");
    eprintln!("Execution: NOT ATTEMPTED");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        usage();
        return;
    }
    if args.len() < 2 || args[0] != "inspect" {
        usage();
        process::exit(2);
    }
    let json = args.iter().any(|a| a == "--json");
    let path = match args.iter().skip(1).find(|a| a.as_str() != "--json") {
        Some(p) => p,
        None => {
            usage();
            process::exit(2);
        }
    };
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Unable to read {path}: {e}");
            process::exit(1);
        }
    };
    match inspect(&bytes) {
        Ok(report) if json => {
            match serde_json::to_string_pretty(&report) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("JSON error: {e}");
                    process::exit(1);
                }
            }
        }
        Ok(r) => {
            println!("Nexa PE Inspector");
            println!("File: {path}");
            println!("Format: {}", r.format);
            println!("Machine: {} (0x{:04X})", r.machine, r.machine_raw);
            println!("Subsystem: {}", r.subsystem);
            println!("Image base: 0x{:X}", r.image_base);
            println!("Entry point RVA: 0x{:X}", r.entry_point_rva);
            println!("Sections ({}):", r.sections.len());
            for s in &r.sections {
                println!(
                    "  {:<8} RVA 0x{:08X} raw 0x{:08X}+0x{:X} flags 0x{:08X}",
                    s.name, s.virtual_address, s.raw_offset, s.raw_size, s.characteristics
                );
            }
            println!("Imports: {} libraries", r.imports.len());
            for lib in &r.imports {
                println!("  {} ({} symbols)", lib.name, lib.symbols.len());
            }
            println!("Exports: {} named symbols", r.exports.len());
            println!(
                "Relocations: {} blocks, {} entries",
                r.relocation_blocks, r.relocation_entries
            );
            println!("Execution: {}", r.execution);
        }
        Err(e) => {
            eprintln!("Inspection failed: {e}");
            eprintln!("Execution: NOT ATTEMPTED");
            process::exit(1);
        }
    }
}
