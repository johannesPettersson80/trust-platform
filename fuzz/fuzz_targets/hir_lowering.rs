#![no_main]

use libfuzzer_sys::fuzz_target;

const MAX_SOURCE_BYTES: usize = 16 * 1024;

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(&data[..data.len().min(MAX_SOURCE_BYTES)]);
    let _ = trust_runtime::harness::bytecode_module_from_source(&source);
});
