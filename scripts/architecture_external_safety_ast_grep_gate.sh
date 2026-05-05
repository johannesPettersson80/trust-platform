#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
ARTIFACT_DIR="$ROOT/target/gate-artifacts/architecture-external-safety-${COMMIT}"
POLICY="$ROOT/xtask/config/full_map_policy.json"
RAW_JSON="$ARTIFACT_DIR/ast-grep-unsafe-raw.json"
NORMALIZED_JSON="$ARTIFACT_DIR/ast-grep-unsafe-normalized.json"
SUMMARY_TXT="$ARTIFACT_DIR/ast-grep-unsafe-summary.txt"
UNREGISTERED_JSON="$ARTIFACT_DIR/ast-grep-unregistered-unsafe.json"
MISSING_REGISTERED_JSON="$ARTIFACT_DIR/ast-grep-missing-registered-unsafe.json"
UNSUPPORTED_TXT="$ARTIFACT_DIR/ast-grep-unsupported-unsafe-forms.txt"

mkdir -p "$ARTIFACT_DIR"

if command -v ast-grep >/dev/null 2>&1 && ast-grep --version 2>/dev/null | grep -qi 'ast-grep'; then
  AST_GREP_BIN="${AST_GREP_BIN:-ast-grep}"
elif command -v sg >/dev/null 2>&1 && sg --version 2>/dev/null | grep -qi 'ast-grep'; then
  AST_GREP_BIN="${AST_GREP_BIN:-sg}"
else
  echo "error: ast-grep is required for the external unsafe scanner gate" >&2
  echo "install: cargo install ast-grep --version 0.42.1 --locked" >&2
  exit 2
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required for the external unsafe scanner gate" >&2
  exit 2
fi

if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep (rg) is required for the external unsafe scanner gate" >&2
  exit 2
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
RAW_TMP_DIR="$TMP_DIR/raw"
NORMALIZED_TMP_DIR="$TMP_DIR/normalized"
mkdir -p "$RAW_TMP_DIR" "$NORMALIZED_TMP_DIR"

run_pattern() {
  local label="$1"
  local pattern="$2"
  local raw="$RAW_TMP_DIR/${label}.json"
  local normalized="$NORMALIZED_TMP_DIR/${label}.json"

  (
    cd "$ROOT"
    "$AST_GREP_BIN" run \
      --pattern "$pattern" \
      --lang rust \
      --json=compact \
      --globs "*.rs" \
      --globs "!**/tests/**" \
      --globs "!**/test/**" \
      --globs "!**/*tests.rs" \
      crates third_party
  ) >"$raw"

  jq --arg rule "$label" '
    .[]
    | {
        rule: $rule,
        path: .file,
        line: (.range.start.line + 1),
        column: (.range.start.column + 1),
        text: .text
      }
  ' "$raw" >"$normalized"
}

run_pattern "unsafe_block" 'unsafe { $$$BODY }'
run_pattern "unsafe_impl" 'unsafe impl $TRAIT for $TYPE { $$$BODY }'
run_pattern "unsafe_fn_no_return" 'unsafe fn $NAME($$$ARGS) { $$$BODY }'
run_pattern "unsafe_fn_return" 'unsafe fn $NAME($$$ARGS) -> $RET { $$$BODY }'
run_pattern "unsafe_fn_generic_return" 'unsafe fn $NAME<$GENERIC>($$$ARGS) -> $RET { $$$BODY }'

jq -s 'flatten' "$RAW_TMP_DIR"/*.json >"$RAW_JSON"
jq -s '
  flatten
  | unique_by(.rule, .path, .line, .column, .text)
  | sort_by(.path, .line, .column, .rule)
' "$NORMALIZED_TMP_DIR"/*.json >"$NORMALIZED_JSON"

EXACT_KEYS_JSON="$(
  jq -c '[.unsafe_concurrency.unsafe_site_register[] | .path + ":" + (.line | tostring)]' "$POLICY"
)"
DELEGATED_PREFIXES_JSON="$(
  jq -c '[.unsafe_concurrency.delegated_unsafe_path_register[].path_prefix]' "$POLICY"
)"
ACTUAL_KEYS_JSON="$(
  jq -c '[.[] | .path + ":" + (.line | tostring)] | unique' "$NORMALIZED_JSON"
)"

jq \
  --argjson exact "$EXACT_KEYS_JSON" \
  --argjson prefixes "$DELEGATED_PREFIXES_JSON" '
    def site_key($site): $site.path + ":" + ($site.line | tostring);
    [
      .[] as $site
      | select(
          (($exact | index(site_key($site))) == null)
          and ((any($prefixes[]; . as $prefix | $site.path | startswith($prefix))) | not)
        )
      | $site
    ]
  ' "$NORMALIZED_JSON" >"$UNREGISTERED_JSON"

jq \
  --argjson actual "$ACTUAL_KEYS_JSON" '
    [
      .unsafe_concurrency.unsafe_site_register[] as $site
      | select(($actual | index($site.path + ":" + ($site.line | tostring))) == null)
      | $site
    ]
  ' "$POLICY" >"$MISSING_REGISTERED_JSON"

(
  cd "$ROOT"
  rg -n '\bunsafe\s+(trait\b|extern\s*("[^"]+")?\s*\{)' \
    crates third_party \
    -g "*.rs" \
    -g "!**/tests/**" \
    -g "!**/test/**" \
    -g "!**/*tests.rs" \
    >"$UNSUPPORTED_TXT" || true
)

total_matches="$(jq 'length' "$NORMALIZED_JSON")"
registered_matches="$(
  jq --argjson exact "$EXACT_KEYS_JSON" '
    [.[] as $site | select(($exact | index($site.path + ":" + ($site.line | tostring))) != null)]
    | length
  ' "$NORMALIZED_JSON"
)"
delegated_matches="$(
  jq --argjson prefixes "$DELEGATED_PREFIXES_JSON" '
    [.[] as $site | select(any($prefixes[]; . as $prefix | $site.path | startswith($prefix)))]
    | length
  ' "$NORMALIZED_JSON"
)"
unregistered_matches="$(jq 'length' "$UNREGISTERED_JSON")"
missing_registered="$(jq 'length' "$MISSING_REGISTERED_JSON")"
unsupported_forms="$(wc -l <"$UNSUPPORTED_TXT" | tr -d ' ')"

{
  echo "# Architecture external safety ast-grep gate"
  echo "commit=$COMMIT"
  echo "generated_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "scanner=$("$AST_GREP_BIN" --version)"
  echo "policy=xtask/config/full_map_policy.json"
  echo "raw_json=$RAW_JSON"
  echo "normalized_json=$NORMALIZED_JSON"
  echo "unregistered_json=$UNREGISTERED_JSON"
  echo "missing_registered_json=$MISSING_REGISTERED_JSON"
  echo "unsupported_forms=$UNSUPPORTED_TXT"
  echo
  echo "matched_unsafe_constructs=$total_matches"
  echo "registered_first_party_matches=$registered_matches"
  echo "delegated_path_matches=$delegated_matches"
  echo "unregistered_matches=$unregistered_matches"
  echo "missing_registered_sites=$missing_registered"
  echo "unsupported_unsafe_forms=$unsupported_forms"
  echo
  echo "registered_policy=unsafe_site_register exact file:line entries"
  echo "delegated_policy=delegated_unsafe_path_register path prefixes"
  echo "decision=pass only when every AST unsafe construct is registered or delegated and every exact registered site still exists"
} >"$SUMMARY_TXT"

cat "$SUMMARY_TXT"

if [[ "$unregistered_matches" != "0" ]]; then
  echo "error: ast-grep found unregistered unsafe constructs" >&2
  jq -r '.[] | "\(.path):\(.line):\(.column) [\(.rule)] \(.text | gsub("\n"; " ") | .[0:180])"' \
    "$UNREGISTERED_JSON" >&2
  exit 1
fi

if [[ "$missing_registered" != "0" ]]; then
  echo "error: unsafe register contains stale exact file:line entries" >&2
  jq -r '.[] | "\(.path):\(.line) owner=\(.owner)"' "$MISSING_REGISTERED_JSON" >&2
  exit 1
fi

if [[ "$unsupported_forms" != "0" ]]; then
  echo "error: ast-grep gate needs a rule before accepting unsafe trait or unsafe extern blocks" >&2
  cat "$UNSUPPORTED_TXT" >&2
  exit 1
fi
