#!/usr/bin/env bash
# Builder-only batch preparation; call once after formatting, before source freeze.
# Caller owns the Cargo target lease and records this step in the batch ledger.
set -euo pipefail
if [[ $# -ne 1 ]]; then
    echo 'usage: refresh_a4_provenance.sh TASK_EVIDENCE_DIRECTORY' >&2
    exit 2
fi
cargo run --locked -p verification-cases --example refresh_portability_provenance -- "$1"
# No format pass follows: only TOML hash strings and literal Rust pins are changed.
# Case/assertion/trace bodies and historical measured artifacts are never rewritten.
