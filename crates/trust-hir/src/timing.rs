//! Opt-in compiler timing diagnostics. Enable with `TRUST_COMPILER_TRACE=1`.
//! Records stage names, counts and durations, never source text or PLC values.
use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};
use std::time::Instant;

/// Whether compiler timing events are enabled for this process.
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("TRUST_COMPILER_TRACE").as_deref() == Ok("1"))
}

/// A synchronous timing span; its end event is emitted on success, error or unwind.
/// Nested stages inherit the enclosing request and compilation spans.
pub struct CompilerStage {
    started: Option<Instant>,
    _span: Option<tracing::span::EnteredSpan>,
}

impl CompilerStage {
    /// Begin an opt-in stage with a fixed label and non-sensitive item count.
    pub fn new(stage: &'static str, items: usize) -> Self {
        if !enabled() {
            return Self {
                started: None,
                _span: None,
            };
        }
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let span = tracing::info_span!(target: "trust::compiler_timing", "compiler_stage", id, stage, items).entered();
        tracing::info!(target: "trust::compiler_timing", event = "begin");
        Self {
            started: Some(Instant::now()),
            _span: Some(span),
        }
    }
}

impl Drop for CompilerStage {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            tracing::info!(target: "trust::compiler_timing", event = "end",
                elapsed_us = started.elapsed().as_micros() as u64,
                unwinding = std::thread::panicking());
        }
    }
}

/// Record a fixed diagnostic event, such as a cache hit or selected source ID.
pub fn event(name: &'static str, count: usize) {
    if enabled() {
        tracing::info!(target: "trust::compiler_timing", event = name, count);
    }
}
