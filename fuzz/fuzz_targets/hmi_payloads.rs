#![no_main]

use libfuzzer_sys::fuzz_target;
use trust_runtime::hmi::HmiEventStreamState;

const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

fuzz_target!(|data: &[u8]| {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(
        &data[..data.len().min(MAX_PAYLOAD_BYTES)],
    ) else {
        return;
    };
    let mut state = HmiEventStreamState::default();
    state.prime_schema(&value);
    let _ = state.values_request_params();
    let _ = state.observe_schema(&value);
    let _ = state.observe_values(&value);
    let _ = state.observe_alarms(&value);
});
