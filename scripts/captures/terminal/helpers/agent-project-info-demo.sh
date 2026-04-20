#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"workspace.project_info","params":{}}' \
  | ./target/debug/trust-runtime agent serve --project ./examples/memory_marker_counter
