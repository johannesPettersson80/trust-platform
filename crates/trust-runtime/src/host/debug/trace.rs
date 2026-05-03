//! Debug trace helpers.

#![allow(missing_docs)]

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::sync::OnceLock;

pub(crate) fn trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("ST_DEBUG_TRACE").is_some())
}

fn trace_log_file() -> Option<&'static Mutex<std::fs::File>> {
    static FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    FILE.get_or_init(|| {
        let path = std::env::var("ST_DEBUG_TRACE_LOG")
            .ok()
            .or_else(|| std::env::var("ST_DEBUG_DAP_LOG").ok())?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        Some(Mutex::new(file))
    })
    .as_ref()
}

pub(crate) fn trace_debug(message: &str) {
    if trace_enabled() {
        eprintln!("[trust-runtime][debug] {message}");
        if let Some(file) = trace_log_file() {
            if let Ok(mut file) = file.lock() {
                let _ = writeln!(file, "## [trust-runtime][debug] {message}");
                let _ = file.flush();
            }
        }
    }
}
