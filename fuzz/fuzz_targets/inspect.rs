#![no_main]

use libfuzzer_sys::fuzz_target;
use nexa_inspect::inspect;

fn decode_hex_seed(data: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(data).ok()?;
    let hex = text.strip_prefix("NEXA-PE-HEX\n")?;
    let digits: Vec<u8> = hex.bytes().filter(|byte| !byte.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }

    digits
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16)?;
            let low = (pair[1] as char).to_digit(16)?;
            Some(((high << 4) | low) as u8)
        })
        .collect()
}

fuzz_target!(|data: &[u8]| {
    // The checked-in corpus uses a text-safe hex envelope so the seed fixtures
    // can be reviewed in Git. Decode recognized fixtures before parsing;
    // arbitrary libFuzzer mutations are still passed to the parser directly.
    let decoded = decode_hex_seed(data);
    let input = decoded.as_deref().unwrap_or(data);
    let _ = inspect(input);
});
