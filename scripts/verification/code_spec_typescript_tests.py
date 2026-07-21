"""Focused tests for the Phase 17 TypeScript/JavaScript product scanner."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCANNER = ROOT / "scripts/verification/code_spec_ts_scan.mjs"


def scan_fixture(
    path: Path,
    source_kind: str = "typescript",
    scan_mode: str = "ast",
) -> tuple[dict[str, object], subprocess.CompletedProcess[str]]:
    request = {
        "schema_version": 1,
        "root": str(path.parents[1]),
        "files": [
            {
                "path": path.relative_to(path.parents[1]).as_posix(),
                "module": "fixture::panel",
                "source_kind": source_kind,
                "scan_mode": scan_mode,
            }
        ],
    }
    completed = subprocess.run(
        ["node", str(SCANNER)],
        input=json.dumps(request),
        text=True,
        capture_output=True,
        check=False,
    )
    payload = json.loads(completed.stdout) if completed.stdout else {}
    return payload, completed


class TypeScriptExtractorTests(unittest.TestCase):
    def test_functions_methods_exports_and_anonymous_registrations_are_distinct(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "src/panel.ts"
            path.parent.mkdir(parents=True)
            path.write_text(
                """
export interface Message { kind: string }
export const CHANNEL = 'trust.panel';
export function activate(value: string): void { consume(value); }
class Panel {
  public render(): string { return CHANNEL; }
}
commands.registerCommand('trust.open', () => activate('open'));
commands.registerCommand('trust.close', () => activate('close'));
panel.webview.onDidReceiveMessage((message: Message) => consume(message));
"""
            )
            payload, completed = scan_fixture(path)

        self.assertEqual(0, completed.returncode, completed.stderr)
        self.assertEqual([], payload["diagnostics"])
        facts = payload["sources"][0]["facts"]
        symbols = {(fact["qualified_symbol"], fact["item_kind"]) for fact in facts}
        self.assertIn(("fixture::panel::activate", "typescript_function"), symbols)
        self.assertIn(("fixture::panel::Panel::render", "typescript_method"), symbols)
        self.assertIn(("fixture::panel::Message", "typescript_export"), symbols)
        self.assertIn(("fixture::panel::CHANNEL", "typescript_export"), symbols)
        registrations = [fact for fact in facts if fact["item_kind"] == "typescript_registration"]
        self.assertEqual(3, len(registrations))
        self.assertEqual(3, len({fact["qualified_symbol"] for fact in registrations}))
        self.assertTrue(any("trust.open" in fact["qualified_symbol"] for fact in registrations))
        self.assertTrue(any("trust.close" in fact["qualified_symbol"] for fact in registrations))

    def test_javascript_allow_js_and_unsupported_syntax_diagnostic(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "src/module.js"
            path.parent.mkdir(parents=True)
            path.write_text("export function shipped(value) { return value + 1; }\n")
            payload, completed = scan_fixture(path, "javascript")
            self.assertEqual(0, completed.returncode, completed.stderr)
            self.assertEqual("javascript_function", payload["sources"][0]["facts"][0]["item_kind"])

            path.write_text("export function broken( {\n")
            payload, completed = scan_fixture(path, "javascript")
            self.assertNotEqual(0, completed.returncode)
            self.assertTrue(payload["diagnostics"])

    def test_bound_object_and_class_expression_methods_keep_lexical_owners(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "src/bundle.js"
            path.parent.mkdir(parents=True)
            path.write_text(
                """
const first = define({ run(value) { return value + 1; } });
const second = define({ run(value) { return value + 2; } });
const FirstView = wrap(class { render() { return 'first'; } });
const SecondView = wrap(class { render() { return 'second'; } });
"""
            )
            payload, completed = scan_fixture(path, "javascript")

        self.assertEqual(0, completed.returncode, completed.stderr)
        methods = [
            fact["qualified_symbol"]
            for fact in payload["sources"][0]["facts"]
            if fact["item_kind"] == "javascript_method"
        ]
        self.assertEqual(4, len(methods))
        self.assertEqual(4, len(set(methods)))
        self.assertTrue(any("::first::run" in symbol for symbol in methods))
        self.assertTrue(any("::second::run" in symbol for symbol in methods))
        self.assertTrue(any("::FirstView::render" in symbol for symbol in methods))
        self.assertTrue(any("::SecondView::render" in symbol for symbol in methods))

    def test_generated_or_vendored_bundle_is_one_parse_checked_asset_fact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "src/generated.js"
            path.parent.mkdir(parents=True)
            path.write_text(
                "/*! governed provenance */\nclass First { run() {} } class Second { run() {} }\n"
            )
            expected_size = len(path.read_bytes())
            payload, completed = scan_fixture(path, "javascript", "asset")

        self.assertEqual(0, completed.returncode, completed.stderr)
        self.assertEqual([], payload["diagnostics"])
        facts = payload["sources"][0]["facts"]
        self.assertEqual(1, len(facts))
        self.assertEqual("javascript_asset", facts[0]["item_kind"])
        self.assertEqual("fixture::panel::<asset>", facts[0]["qualified_symbol"])
        self.assertEqual(0, facts[0]["body_start"])
        self.assertEqual(expected_size, facts[0]["body_end"])

    def test_parser_derived_browser_risk_policy_is_bound_to_each_fact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "src/boundary.ts"
            path.parent.mkdir(parents=True)
            path.write_text(
                """
export async function boundary() {
  const now = Date.now();
  const token = process.env.TOKEN;
  await fetch('/status');
  state.set('ready', true);
  authorizeRole('operator');
  persistAndRestart();
  try { await work(); } catch (_error) {}
}
"""
            )
            payload, completed = scan_fixture(path)

        self.assertEqual(0, completed.returncode, completed.stderr)
        fact = next(
            fact
            for fact in payload["sources"][0]["facts"]
            if fact["qualified_symbol"].endswith("::boundary")
        )
        for expected in (
            "host_time",
            "filesystem_or_environment",
            "network_io",
            "state_mutation",
            "authorization",
            "protocol_response_or_status",
            "broad_error_suppression",
            "persistence_retain_reload_restart",
        ):
            self.assertIn(expected, fact["risk_signals"])


if __name__ == "__main__":
    unittest.main()
