#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

echo "[editor-smoke] validating editor reference files"
required_files=(
  "docs/guides/EDITOR_SETUP_NEOVIM_ZED.md"
  "editors/neovim/README.md"
  "editors/neovim/lspconfig.lua"
  "editors/zed/README.md"
  "editors/zed/settings.json"
)

for file in "${required_files[@]}"; do
  if [[ ! -f "${file}" ]]; then
    echo "[editor-smoke] missing required file: ${file}" >&2
    exit 1
  fi
done

python3 - <<'PY'
import json
from pathlib import Path

path = Path("editors/zed/settings.json")
payload = json.loads(path.read_text(encoding="utf-8"))
errors = []

lsp = payload.get("lsp", {}).get("trust-lsp")
if not isinstance(lsp, dict):
    errors.append("lsp.trust-lsp object is missing")
else:
    binary_path = lsp.get("binary", {}).get("path")
    if binary_path != "trust-lsp":
        errors.append(
            f"lsp.trust-lsp.binary.path must be 'trust-lsp' (found {binary_path!r})"
        )

st_lang = payload.get("languages", {}).get("Structured Text")
if not isinstance(st_lang, dict):
    errors.append("languages.'Structured Text' object is missing")
else:
    servers = st_lang.get("language_servers", [])
    if "trust-lsp" not in servers:
        errors.append(
            "languages.'Structured Text'.language_servers must include 'trust-lsp'"
        )
    if st_lang.get("formatter") != "language_server":
        errors.append("languages.'Structured Text'.formatter must be 'language_server'")
    format_on_save = st_lang.get("format_on_save")
    if format_on_save not in ("on", True):
        errors.append("languages.'Structured Text'.format_on_save must be 'on'")

if errors:
    for err in errors:
        print(f"[editor-smoke] {err}")
    raise SystemExit(1)

print("[editor-smoke] zed settings schema checks passed")
PY

python3 - <<'PY'
from pathlib import Path

text = Path("editors/neovim/lspconfig.lua").read_text(encoding="utf-8")
required_snippets = [
    'cmd = { "trust-lsp" }',
    'filetypes = { "st", "pou" }',
    'vim.lsp.buf.hover',
    'vim.lsp.buf.definition',
    'vim.lsp.buf.format',
    'v:lua.vim.lsp.omnifunc',
]
missing = [snippet for snippet in required_snippets if snippet not in text]
if missing:
    for snippet in missing:
        print(f"[editor-smoke] neovim config missing snippet: {snippet}")
    raise SystemExit(1)

print("[editor-smoke] neovim config checks passed")
PY

echo "[editor-smoke] running targeted trust-lsp workflow tests"
tests=(
  "handlers::tests::core::lsp_pull_diagnostics_returns_unchanged_and_explainer"
  "handlers::tests::core::lsp_hover_variable"
  "handlers::tests::completion_hover::lsp_completion_respects_stdlib_allowlist"
  "handlers::tests::formatting_and_navigation::lsp_formatting_snapshot"
  "handlers::tests::lsp_golden_multi_root_protocol_snapshot"
)

for test_name in "${tests[@]}"; do
  echo "[editor-smoke] cargo test -p trust-lsp ${test_name} -- --exact"
  cargo test -p trust-lsp "${test_name}" -- --exact
done

echo "[editor-smoke] all checks passed"
