use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);
const CHILD_MARKER: &str = "TRUST_DEBUG_TRACE_CONTRACT_CHILD";

struct TempTrace {
    root: PathBuf,
}

impl TempTrace {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-debug-trace-{}-{label}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("create trace fixture");
        Self { root }
    }
}

impl Drop for TempTrace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn run_trace_child(
    enabled: bool,
    primary_log: Option<&Path>,
    compatibility_log: Option<&Path>,
    messages: &str,
) -> std::process::Output {
    let mut command = Command::new(std::env::current_exe().expect("test executable"));
    command
        .arg("--exact")
        .arg("debug::trace::contract_tests::trace_child")
        .arg("--nocapture")
        .env(CHILD_MARKER, "1")
        .env("TRUST_DEBUG_TRACE_MESSAGES", messages);
    if enabled {
        command.env("ST_DEBUG_TRACE", "");
    } else {
        command.env_remove("ST_DEBUG_TRACE");
    }
    if let Some(path) = primary_log {
        command.env("ST_DEBUG_TRACE_LOG", path);
    } else {
        command.env_remove("ST_DEBUG_TRACE_LOG");
    }
    if let Some(path) = compatibility_log {
        command.env("ST_DEBUG_DAP_LOG", path);
    } else {
        command.env_remove("ST_DEBUG_DAP_LOG");
    }
    command.output().expect("run trace child")
}

#[test]
fn trace_child() {
    if std::env::var_os(CHILD_MARKER).is_none() {
        let fixture = TempTrace::new("standalone-disabled-child");
        let log = fixture.root.join("trace.log");
        let output = run_trace_child(false, Some(&log), None, "hidden");
        assert!(output.status.success());
        assert!(!log.exists());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("hidden"));
        return;
    }
    for message in std::env::var("TRUST_DEBUG_TRACE_MESSAGES")
        .unwrap_or_default()
        .split('|')
    {
        trace_debug(message);
    }
}

#[test]
fn empty_trace_environment_value_enables_prefixed_append_and_flush() {
    let fixture = TempTrace::new("enabled");
    let log = fixture.root.join("trace.log");
    let output = run_trace_child(true, Some(&log), None, "first|second");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(log).expect("trace log"),
        "## [trust-runtime][debug] first\n## [trust-runtime][debug] second\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("[trust-runtime][debug] first"));
    assert!(stderr.contains("[trust-runtime][debug] second"));
}

#[test]
fn configured_log_path_does_not_write_when_tracing_is_disabled() {
    let fixture = TempTrace::new("disabled");
    let log = fixture.root.join("trace.log");
    let output = run_trace_child(false, Some(&log), None, "hidden");
    assert!(output.status.success());
    assert!(!log.exists());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("hidden"));
}

#[test]
fn primary_trace_log_variable_takes_precedence_over_compatibility_path() {
    let fixture = TempTrace::new("precedence");
    let primary = fixture.root.join("primary.log");
    let compatibility = fixture.root.join("compatibility.log");
    let output = run_trace_child(true, Some(&primary), Some(&compatibility), "message");
    assert!(output.status.success());
    assert!(std::fs::read_to_string(primary)
        .expect("primary")
        .contains("message"));
    assert!(!compatibility.exists());
}

#[test]
fn compatibility_trace_log_variable_is_used_when_primary_is_absent() {
    let fixture = TempTrace::new("compatibility");
    let compatibility = fixture.root.join("compatibility.log");
    let output = run_trace_child(true, None, Some(&compatibility), "message");
    assert!(output.status.success());
    assert!(std::fs::read_to_string(compatibility)
        .expect("compatibility")
        .contains("message"));
}

#[test]
fn trace_file_open_failure_is_nonfatal() {
    let fixture = TempTrace::new("open-failure");
    let output = run_trace_child(true, Some(&fixture.root), None, "message");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("[trust-runtime][debug] message"));
}
