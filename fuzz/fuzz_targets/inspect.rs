#![no_main]

use libfuzzer_sys::fuzz_target;
use nexa_inspect::inspect;

fuzz_target!(|data: &[u8]| {
    let _ = inspect(data);
});
