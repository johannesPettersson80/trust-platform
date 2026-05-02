#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
ARTIFACT_DIR="$ROOT/target/gate-artifacts/unsafe-concurrency-baseline-${COMMIT}"

mkdir -p "$ARTIFACT_DIR"

rg -n "\bunsafe\b" "$ROOT/crates" "$ROOT/third_party" \
  > "$ARTIFACT_DIR/unsafe-rg.txt" || true

rg -n "unwrap\(|expect\(|panic!|todo!|unimplemented!" \
  "$ROOT/crates/trust-runtime/src" \
  "$ROOT/crates/trust-hir/src" \
  "$ROOT/crates/trust-lsp/src" \
  "$ROOT/crates/trust-ide/src" \
  > "$ARTIFACT_DIR/panic-like-rg.txt" || true

rg -n "thread::spawn|std::thread|tokio::spawn|spawn_blocking|JoinHandle|mpsc|channel\(|Mutex|RwLock|Arc<|Atomic|Ordering::|shared_memory|SharedMemory|WebSocket|tungstenite|crossbeam|parking_lot|Condvar" \
  "$ROOT/crates/trust-runtime/src" \
  "$ROOT/crates/trust-hir/src" \
  "$ROOT/crates/trust-lsp/src" \
  "$ROOT/crates/trust-ide/src" \
  > "$ARTIFACT_DIR/concurrency-rg.txt" || true

rg -n "\bunsafe\b" "$ROOT/crates" "$ROOT/third_party" \
  -g "*.rs" -g "!**/tests/**" -g "!**/test/**" -g "!**/*tests.rs" -g "!**/node_modules/**" \
  > "$ARTIFACT_DIR/unsafe-production-rust-rg.txt" || true

rg -n "unwrap\(|expect\(|panic!|todo!|unimplemented!" \
  "$ROOT/crates/trust-runtime/src" \
  "$ROOT/crates/trust-hir/src" \
  "$ROOT/crates/trust-lsp/src" \
  "$ROOT/crates/trust-ide/src" \
  -g "*.rs" -g "!**/tests/**" -g "!**/test/**" -g "!**/*tests.rs" \
  > "$ARTIFACT_DIR/panic-like-production-rust-rg.txt" || true

rg -n "thread::spawn|std::thread|tokio::spawn|spawn_blocking|JoinHandle|mpsc|channel\(|Mutex|RwLock|Arc<|Atomic|Ordering::|shared_memory|SharedMemory|WebSocket|tungstenite|crossbeam|parking_lot|Condvar" \
  "$ROOT/crates/trust-runtime/src" \
  "$ROOT/crates/trust-hir/src" \
  "$ROOT/crates/trust-lsp/src" \
  "$ROOT/crates/trust-ide/src" \
  -g "*.rs" -g "!**/tests/**" -g "!**/test/**" -g "!**/*tests.rs" \
  > "$ARTIFACT_DIR/concurrency-production-rust-rg.txt" || true

{
  echo "commit=$COMMIT"
  rustc -Vv
  cargo -V
  echo
  wc -l \
    "$ARTIFACT_DIR/unsafe-rg.txt" \
    "$ARTIFACT_DIR/panic-like-rg.txt" \
    "$ARTIFACT_DIR/concurrency-rg.txt" \
    "$ARTIFACT_DIR/unsafe-production-rust-rg.txt" \
    "$ARTIFACT_DIR/panic-like-production-rust-rg.txt" \
    "$ARTIFACT_DIR/concurrency-production-rust-rg.txt"
} > "$ARTIFACT_DIR/summary.txt"

echo "wrote $ARTIFACT_DIR"
