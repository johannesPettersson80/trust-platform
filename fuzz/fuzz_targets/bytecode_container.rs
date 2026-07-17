#![no_main]

use libfuzzer_sys::fuzz_target;
use trust_runtime::bytecode::BytecodeModule;

const MAX_CONTAINER_BYTES: usize = 1024 * 1024;

fuzz_target!(|data: &[u8]| {
    let _ = BytecodeModule::decode(&data[..data.len().min(MAX_CONTAINER_BYTES)]);
});
