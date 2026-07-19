#![no_main]

use libfuzzer_sys::fuzz_target;
use trust_runtime::config::RuntimeConfig;

const MAX_CONFIG_BYTES: usize = 64 * 1024;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(&data[..data.len().min(MAX_CONFIG_BYTES)]) {
        let path = std::env::temp_dir().join(format!(
            "trust-runtime-config-fuzz-{}.toml",
            std::process::id()
        ));
        if std::fs::write(&path, text).is_ok() {
            let _ = RuntimeConfig::load(&path);
        }
        let _ = std::fs::remove_file(path);
    }
});
