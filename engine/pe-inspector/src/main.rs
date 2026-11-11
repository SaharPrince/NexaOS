use nexa_inspect::inspect;
use std::{
    env, fs,
    io::{self, Read},
    process,
};

const MAX_INPUT_FILE_SIZE: usize = 512 * 1024 * 1024;

fn read_limited<R: Read>(reader: R, max_size: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(max_size as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("input file exceeds the configured inspection limit of {max_size} bytes"),
        ));
    }
    Ok(bytes)
}

fn parse_inspect_args(args: &[String]) -> Option<(bool, &str)> {
    if args.first().map(String::as_str) != Some("inspect") {
        return None;
    }

    match args {
        [_, path] if !path.starts_with('-') => Some((false, path)),
        [_, flag, path] if flag == "--json" && !path.starts_with('-') => Some((true, path)),
        _ => None,
    }
}

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

    let (json, path) = match parse_inspect_args(&args) {
        Some(parsed) => parsed,
        None => {
            usage();
            process::exit(2);
        }
    };

    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Unable to open {path}: {e}");
            process::exit(1);
        }
    };
    let bytes = match read_limited(file, MAX_INPUT_FILE_SIZE) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Unable to read {path}: {e}");
            process::exit(1);
        }
    };
    match inspect(&bytes) {
        Ok(report) if json => match serde_json::to_string_pretty(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("JSON error: {e}");
                process::exit(1);
            }
        },
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

#[cfg(test)]
mod tests {
    use super::{parse_inspect_args, read_limited};
    use std::io::{self, Cursor};

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn parses_plain_inspection_arguments() {
        assert_eq!(
            parse_inspect_args(&strings(&["inspect", "sample.exe"])),
            Some((false, "sample.exe"))
        );
    }

    #[test]
    fn parses_json_inspection_arguments() {
        assert_eq!(
            parse_inspect_args(&strings(&["inspect", "--json", "sample.exe"])),
            Some((true, "sample.exe"))
        );
    }

    #[test]
    fn rejects_missing_or_extra_inspection_arguments() {
        for args in [
            strings(&[]),
            strings(&["inspect"]),
            strings(&["inspect", "sample.exe", "extra.exe"]),
            strings(&["inspect", "--json", "sample.exe", "extra.exe"]),
            strings(&["inspect", "--json", "--json", "sample.exe"]),
            strings(&["other", "sample.exe"]),
        ] {
            assert_eq!(parse_inspect_args(&args), None);
        }
    }

    #[test]
    fn rejects_json_flag_after_path_when_usage_requires_it_before_path() {
        assert_eq!(
            parse_inspect_args(&strings(&["inspect", "sample.exe", "--json"])),
            None
        );
    }

    #[test]
    fn reads_input_at_size_limit() {
        let data = vec![0x5a; 16];
        let result = read_limited(Cursor::new(data.clone()), 16).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn rejects_input_over_size_limit() {
        let data = vec![0x5a; 17];
        let error = read_limited(Cursor::new(data), 16).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("16 bytes"));
    }
}
