//! Spec 34 §4.3 HOST-05: the ADS and OPC UA client cycle ports, which run on the
//! control thread, take no lock and wait on nothing in production code.

use std::path::{Path, PathBuf};

include!("../../../tests/support/repository_source_oracle.rs");

const BLOCKING_TOKENS: [&str; 10] = [
    "lock(",
    "recv(",
    "recv_timeout(",
    "sleep(",
    "park(",
    "block_on(",
    "Mutex",
    "RwLock",
    "Condvar",
    "Barrier",
];

const CYCLE_PORTS: [&str; 2] = [
    "crates/trust-runtime/src/host/ads/client/cycle_port.rs",
    "crates/trust-runtime/src/host/opcua/client_cycle_port.rs",
];

fn read_runtime_source(relative: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repository root")
        .to_path_buf();
    repository_source_tree_read_to_string!(
        (root.join(relative), &root),
        roots = ["crates/trust-runtime/src"],
        extension = "rs",
    )
    .unwrap_or_else(|err| panic!("read {relative}: {err}"))
    .replace("\r\n", "\n")
}

/// Production code is everything before the first `#[cfg(test)]` module.
fn production_source(source: &str) -> &str {
    source
        .split_once("#[cfg(test)]")
        .map_or(source, |(production, _)| production)
}

/// Finds a call or type named `token`, ignoring longer identifiers such as `try_recv(`.
fn first_token_line(source: &str, token: &str) -> Option<(usize, String)> {
    source.lines().enumerate().find_map(|(index, line)| {
        let found = line.match_indices(token).any(|(at, _)| {
            line[..at]
                .chars()
                .next_back()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
        });
        found.then(|| (index + 1, line.trim().to_owned()))
    })
}

/// Spec 34 §4.3 HOST-05 (line 353): ADS and OPC UA cycle-port production code takes no lock, no blocking receive and no sleep.
#[test]
fn client_cycle_port_production_code_has_no_blocking_waits() {
    for file in CYCLE_PORTS {
        let source = read_runtime_source(file);
        let production = production_source(&source);
        assert!(
            production.contains("pub(super) fn queue_output("),
            "{file}: production cycle-port code must be present for this gate"
        );
        for token in BLOCKING_TOKENS {
            if let Some((line, text)) = first_token_line(production, token) {
                panic!(
                    "{file}:{line} uses `{token}` in control-thread production code: `{text}`; cycle ports must not wait (spec 34 §4.3 HOST-05)"
                );
            }
        }
    }
}
