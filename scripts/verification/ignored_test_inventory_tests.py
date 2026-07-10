"""Focused contracts for the Phase 3 ignored-test inventory report."""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from .ignored_test_discovery import (
    IgnoredDiscoveryBatch,
    associate_vscode_runtime_skips,
    discover_excluded_node_skip_markers,
    discover_excluded_rust_ignore_markers,
    discover_playwright_skips,
)
from .ignored_test_models import (
    GENERATOR,
    GENERATOR_VERSION,
    IgnoredTestFact,
    InventoryDiagnostic,
    InventoryProvenance,
    IgnoredTestInventoryReport,
    render_markdown,
    write_reports,
)
from .ignored_test_live import _report_contract_paths, build_live_inventory_state
from .ignored_test_report import LIMITATIONS, build_inventory_payload
from .ignored_test_validation import (
    validate_markdown_binding,
    validate_report_payload,
    validate_report_files,
    validate_schema_contract,
)
from .test_catalog_common import input_digest, make_fact, stable_discovery_id
from .test_catalog_models import ScanDiagnostic


class IgnoredTestDiscoveryTests(unittest.TestCase):
    def test_playwright_literal_skip_is_discovered_without_comment_or_dynamic_inference(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "scripts/captures/vscode/runtime-panel-command.spec.mjs"
            source.parent.mkdir(parents=True)
            (root / "scripts/captures/package.json").write_text(
                '{"name":"trust-doc-captures"}\n'
            )
            source.write_text(
                """
// test.skip("comment only", async () => {});
const text = 'test.skip("string only", async () => {})';
test.skip("capture code-server runtime panel command palette", async ({ page }) => {});
test.skip(process.env.DYNAMIC_TITLE, async () => {});
test.describe.skip("unsupported suite skip", () => {});
""".lstrip()
            )

            batch = discover_playwright_skips(root)

        self.assertEqual(
            [fact.name for fact in batch.facts],
            ["capture code-server runtime panel command palette"],
        )
        fact = batch.facts[0]
        self.assertEqual(fact.discovery_source_kind, "playwright_test")
        self.assertEqual(fact.ignore_state, "ignored")
        self.assertEqual(fact.ignore_mechanism, "playwright_literal_skip")
        self.assertEqual(
            fact.path, "scripts/captures/vscode/runtime-panel-command.spec.mjs"
        )
        self.assertEqual(fact.line, 3)
        self.assertEqual(fact.package, "trust-doc-captures")
        self.assertEqual(
            fact.command_hint,
            "cd scripts/captures && npx playwright test vscode/runtime-panel-command.spec.mjs",
        )
        self.assertEqual(fact.discovery_id, "DISC_E45EDC8D2860AAECF144")
        self.assertRegex(fact.discovery_id, r"^DISC_[A-F0-9]{20}$")
        self.assertEqual(
            [(item.kind, item.severity) for item in batch.diagnostics],
            [
                ("dynamic_playwright_skip", "error"),
                ("dynamic_playwright_skip", "error"),
            ],
        )

    def test_vscode_runtime_skip_is_bound_to_unique_enclosing_literal_test(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "editors/vscode/src/test/suite/example.test.ts"
            source.parent.mkdir(parents=True)
            source.write_text(
                'test("first", async function () {\n  ok();\n});\n'
                'test("second", async function () {\n  this.skip();\n});\n'
            )
            first = make_fact(
                source_kind="vscode_test",
                name="first",
                path="editors/vscode/src/test/suite/example.test.ts",
                line=1,
                package="trust-vscode",
                command_hint="cd editors/vscode && npm test",
                command_hint_authority="package_only",
                discovery_confidence="literal_call",
            )
            second = make_fact(
                source_kind="vscode_test",
                name="second",
                path="editors/vscode/src/test/suite/example.test.ts",
                line=4,
                package="trust-vscode",
                command_hint="cd editors/vscode && npm test",
                command_hint_authority="package_only",
                discovery_confidence="literal_call",
            )
            runtime_skip = ScanDiagnostic(
                severity="warning",
                kind="conditional_runtime_skip",
                path=second.path,
                line=5,
                message="runtime this.skip() cannot be represented as a declared ignore attribute",
            )

            batch = associate_vscode_runtime_skips(root, [first, second], [runtime_skip])

        self.assertEqual(len(batch.facts), 1)
        fact = batch.facts[0]
        self.assertEqual(fact.discovery_id, second.stable_id)
        self.assertEqual(fact.name, "second")
        self.assertEqual(fact.line, 5)
        self.assertEqual(fact.ignore_state, "conditional")
        self.assertEqual(fact.ignore_mechanism, "vscode_runtime_skip")
        self.assertEqual(
            fact.ignore_reason,
            "runtime this.skip() cannot be represented as a declared ignore attribute",
        )
        self.assertEqual(batch.diagnostics, [])

    def test_unassociated_runtime_skip_is_an_error_not_a_fabricated_test(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "editors/vscode/src/test/suite/example.test.ts"
            source.parent.mkdir(parents=True)
            source.write_text('test("closed", function () { ok(); });\nthis.skip();\n')
            fact = make_fact(
                source_kind="vscode_test",
                name="closed",
                path="editors/vscode/src/test/suite/example.test.ts",
                line=1,
                package="trust-vscode",
                command_hint="cd editors/vscode && npm test",
                command_hint_authority="package_only",
                discovery_confidence="literal_call",
            )
            runtime_skip = ScanDiagnostic(
                severity="warning",
                kind="conditional_runtime_skip",
                path=fact.path,
                line=2,
                message="runtime skip",
            )

            batch = associate_vscode_runtime_skips(root, [fact], [runtime_skip])

        self.assertEqual(batch.facts, [])
        self.assertEqual(batch.diagnostics[0].kind, "unassociated_vscode_runtime_skip")
        self.assertEqual(batch.diagnostics[0].severity, "error")

    def test_excluded_rust_and_node_skip_markers_fail_visible_without_comment_inference(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rust = root / "xtask/src/lib.rs"
            rust.parent.mkdir(parents=True)
            rust.write_text(
                '// #[ignore]\n#[test]\n#[ignore = "real"]\nfn hidden() {}\n'
            )
            node = root / "tools/tests/example.test.ts"
            node.parent.mkdir(parents=True)
            node.write_text(
                '// test.skip("comment", () => {});\n'
                'const text = "this.skip()";\n'
                'test.skip("real", () => {});\n'
            )

            rust_batch = discover_excluded_rust_ignore_markers(root)
            node_batch = discover_excluded_node_skip_markers(root)

        self.assertEqual(
            [item.kind for item in rust_batch.diagnostics],
            ["excluded_rust_ignore_requires_identity_support"],
        )
        self.assertEqual(
            [item.kind for item in node_batch.diagnostics],
            ["excluded_node_skip_requires_identity_support"],
        )


class IgnoredTestReportTests(unittest.TestCase):
    def test_report_contract_binds_registry_schema_and_registry_data(self) -> None:
        root = Path(__file__).resolve().parents[2]
        paths = _report_contract_paths(root)
        self.assertIn("verification/ignored-tests.toml", paths)
        self.assertIn("verification/schemas/ignored-test.schema.json", paths)
        self.assertIn(
            "verification/schemas/ignored-test-inventory-report.schema.json", paths
        )

    def test_inventory_uses_only_scanner_ignore_states_and_extended_discoveries(self) -> None:
        ignored = make_fact(
            source_kind="rust_unit_test",
            name="ignored",
            path="crates/example/src/lib.rs",
            line=5,
            package="example",
            command_hint="cargo test -p example ignored",
            command_hint_authority="exact",
            discovery_confidence="exact_attribute",
            ignore_state="ignored",
            ignore_reason="fixture",
        )
        conditional = make_fact(
            source_kind="rust_integration_test",
            name="conditional",
            path="crates/example/tests/live.rs",
            line=8,
            package="example",
            command_hint="cargo test -p example --test live conditional",
            command_hint_authority="exact",
            discovery_confidence="exact_attribute",
            ignore_state="conditional",
            ignore_reason="non-linux",
        )
        ordinary = make_fact(
            source_kind="rust_unit_test",
            name="ordinary",
            path="crates/example/src/lib.rs",
            line=20,
            package="example",
            command_hint="cargo test -p example ordinary",
            command_hint_authority="exact",
            discovery_confidence="exact_attribute",
        )
        playwright = IgnoredTestFact(
            discovery_id=stable_discovery_id(
                source_kind="playwright_test",
                package="trust-doc-captures",
                native_id="scripts/captures/example.spec.mjs#paused",
            ),
            native_id="scripts/captures/example.spec.mjs#paused",
            discovery_source_kind="playwright_test",
            name="paused",
            path="scripts/captures/example.spec.mjs",
            line=2,
            package="trust-doc-captures",
            command_hint="cd scripts/captures && npx playwright test example.spec.mjs",
            ignore_state="ignored",
            ignore_mechanism="playwright_literal_skip",
            ignore_reason="literal test.skip declaration",
            reference_candidates=(),
        )

        analysis = build_inventory_payload(
            scanner_facts=[ignored, conditional, ordinary],
            scanner_diagnostics=[],
            playwright_facts=[playwright],
            playwright_diagnostics=[],
        )

        self.assertEqual(len(analysis.records), 3)
        self.assertEqual(
            [(row.name, row.ignore_state) for row in analysis.records],
            [("paused", "ignored"), ("conditional", "conditional"), ("ignored", "ignored")],
        )
        self.assertEqual(analysis.summary["ignored"], 2)
        self.assertEqual(analysis.summary["conditional"], 1)
        surfaces = {row["surface"]: row for row in analysis.surface_summary}
        self.assertEqual(surfaces["shell"]["records"], 0)
        self.assertEqual(surfaces["conformance"]["records"], 0)
        self.assertEqual(surfaces["shell"]["coverage"], "limitation")
        self.assertEqual(surfaces["conformance"]["coverage"], "limitation")

    def test_report_json_and_markdown_are_canonical_and_exactly_bound(self) -> None:
        report = fixture_report()
        json_bytes = report.to_json().encode()
        digest = hashlib.sha256(json_bytes).hexdigest()
        markdown = report.to_markdown(json_digest=digest)

        self.assertEqual(report.to_json(), json.dumps(report.to_dict(), indent=2, sort_keys=True) + "\n")
        self.assertEqual(validate_report_payload(report.to_dict()), [])
        self.assertEqual(validate_markdown_binding(report.to_dict(), json_bytes, markdown), [])
        self.assertIn("## Inventory", markdown)
        self.assertIn(f"`{report.records[0].discovery_id}`", markdown)
        self.assertIn("Shell source has no repository-wide static ignored-test identity convention", markdown)

        tampered = markdown.replace("paused", "changed", 1)
        self.assertIn("does not exactly match", " ".join(validate_markdown_binding(report.to_dict(), json_bytes, tampered)))

    def test_payload_rejects_dirty_or_abbreviated_commit_and_noncanonical_records(self) -> None:
        payload = fixture_report().to_dict()
        payload["commit"] = "dirty:1234567"
        payload["surface_summary"] = list(reversed(payload["surface_summary"]))

        failures = validate_report_payload(payload)

        self.assertTrue(any("clean full Git SHA" in item for item in failures))
        self.assertTrue(any("canonical" in item for item in failures))

    def test_closed_schema_matches_validator_contract(self) -> None:
        schema_path = (
            Path(__file__).resolve().parents[2]
            / "verification/schemas/ignored-test-inventory-report.schema.json"
        )
        schema = json.loads(schema_path.read_text())
        self.assertEqual(validate_schema_contract(schema), [])


class IgnoredTestAtRestTests(unittest.TestCase):
    def test_live_state_cannot_omit_excluded_surface_sentinel_errors(self) -> None:
        scanner = SimpleNamespace(
            provenance=SimpleNamespace(
                commit="1" * 40,
                timestamp="2026-07-10T12:00:00Z",
                platform="linux-aarch64",
                input_paths=(),
            ),
            inferred_facts=(),
            diagnostics=(),
            to_dict=lambda: {"scan_status": "complete"},
        )
        sentinel = IgnoredDiscoveryBatch(
            diagnostics=[
                InventoryDiagnostic(
                    "error",
                    "excluded_rust_ignore_requires_identity_support",
                    "xtask/src/lib.rs",
                    1,
                    "unsupported ignore",
                )
            ]
        )
        with tempfile.TemporaryDirectory() as directory, patch(
            "scripts.verification.ignored_test_live.scan_repository",
            return_value=scanner,
        ), patch(
            "scripts.verification.ignored_test_live.discover_playwright_skips",
            return_value=IgnoredDiscoveryBatch(),
        ), patch(
            "scripts.verification.ignored_test_live.discover_excluded_rust_ignore_markers",
            return_value=sentinel,
        ), patch(
            "scripts.verification.ignored_test_live.discover_excluded_node_skip_markers",
            return_value=IgnoredDiscoveryBatch(),
        ):
            with self.assertRaisesRegex(ValueError, "produced error diagnostics"):
                build_live_inventory_state(Path(directory))

    def test_at_rest_validator_reconstructs_live_report(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, json_path, markdown_path, schema_path, state = make_at_rest_fixture(
                Path(directory)
            )
            with patch(
                "scripts.verification.ignored_test_live.build_live_inventory_state",
                return_value=state,
            ):
                failures = validate_report_files(
                    root, json_path, markdown_path, schema_path
                )
        self.assertEqual(failures, [])

    def test_at_rest_validator_rejects_noncanonical_json_and_markdown_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, json_path, markdown_path, schema_path, state = make_at_rest_fixture(
                Path(directory)
            )
            payload = json.loads(json_path.read_text())
            json_path.write_text(json.dumps(payload, sort_keys=True))
            markdown_path.write_text(markdown_path.read_text().replace("paused", "tampered", 1))
            with patch(
                "scripts.verification.ignored_test_live.build_live_inventory_state",
                return_value=state,
            ):
                failures = validate_report_files(
                    root, json_path, markdown_path, schema_path
                )
        joined = " ".join(failures)
        self.assertIn("canonical serialization", joined)
        self.assertIn("does not exactly match", joined)
        self.assertIn("stale JSON digest", joined)

    def test_at_rest_validator_rejects_missing_entrypoint_and_source_drift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, json_path, markdown_path, schema_path, state = make_at_rest_fixture(
                Path(directory)
            )
            missing_state = SimpleNamespace(
                analysis=state.analysis,
                input_paths=("input.txt", "scripts/report_ignored_test_inventory.py"),
            )
            with patch(
                "scripts.verification.ignored_test_live.build_live_inventory_state",
                return_value=missing_state,
            ):
                missing = validate_report_files(root, json_path, markdown_path, schema_path)
            (root / "input.txt").write_text("changed ignore marker\n")
            with patch(
                "scripts.verification.ignored_test_live.build_live_inventory_state",
                return_value=state,
            ):
                drift = validate_report_files(root, json_path, markdown_path, schema_path)
        self.assertIn("complete live report input closure", " ".join(missing))
        self.assertIn("cannot be resolved", " ".join(missing))
        self.assertIn("current report inputs differ", " ".join(drift))
        self.assertIn("input_digest does not match", " ".join(drift))

    def test_at_rest_validator_rejects_short_commit_and_symlinked_input_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, json_path, markdown_path, schema_path, state = make_at_rest_fixture(
                Path(directory)
            )
            payload = json.loads(json_path.read_text())
            payload["commit"] = "1234567"
            json_bytes = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode()
            json_path.write_bytes(json_bytes)
            markdown_path.write_text(
                render_markdown(payload, json_digest=hashlib.sha256(json_bytes).hexdigest())
            )
            external = root.parent / f"{root.name}-external-input.txt"
            external.write_text("external\n")
            (root / "linked-input.txt").symlink_to(external)
            symlink_state = SimpleNamespace(
                analysis=state.analysis,
                input_paths=("input.txt", "linked-input.txt"),
            )
            output_dir = root.parent / f"{root.name}-external-output"
            json_path.parent.rename(output_dir)
            json_path.parent.symlink_to(output_dir, target_is_directory=True)
            with patch(
                "scripts.verification.ignored_test_live.build_live_inventory_state",
                return_value=symlink_state,
            ):
                failures = validate_report_files(
                    root, json_path, markdown_path, schema_path
                )
            external.unlink()
            json_path.parent.unlink()
            shutil.rmtree(output_dir)
        joined = " ".join(failures)
        self.assertIn("clean full Git SHA", joined)
        self.assertIn("symlink component", joined)
        self.assertIn("does not identify the validated JSON file", joined)


def make_at_rest_fixture(
    root: Path,
) -> tuple[Path, Path, Path, Path, SimpleNamespace]:
    subprocess.run(["git", "init", "-q", str(root)], check=True)
    subprocess.run(
        ["git", "-C", str(root), "config", "user.email", "fixture@example.invalid"],
        check=True,
    )
    subprocess.run(
        ["git", "-C", str(root), "config", "user.name", "Fixture"], check=True
    )
    (root / "input.txt").write_text("stable ignore marker\n")
    subprocess.run(["git", "-C", str(root), "add", "input.txt"], check=True)
    subprocess.run(
        ["git", "-C", str(root), "commit", "-qm", "fixture"], check=True
    )
    commit = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    base = fixture_report()
    json_relative = Path("target/gate-artifacts/verification/ignored-test-inventory.json")
    markdown_relative = Path("target/gate-artifacts/verification/ignored-test-inventory.md")
    report = IgnoredTestInventoryReport(
        provenance=InventoryProvenance(
            command=(
                "python3",
                "scripts/report_ignored_test_inventory.py",
                "--json-out",
                json_relative.as_posix(),
                "--markdown-out",
                markdown_relative.as_posix(),
                "--timestamp",
                "2026-07-10T12:00:00Z",
            ),
            commit=commit,
            timestamp="2026-07-10T12:00:00Z",
            platform="linux-aarch64",
            input_paths=("input.txt",),
            output_json=json_relative.as_posix(),
            output_markdown=markdown_relative.as_posix(),
        ),
        input_digest=input_digest(root, ["input.txt"]),
        records=base.records,
        diagnostics=base.diagnostics,
        surface_summary=base.surface_summary,
        limitations=base.limitations,
    )
    json_path = root / json_relative
    markdown_path = root / markdown_relative
    write_reports(report, json_path=json_path, markdown_path=markdown_path)
    state = SimpleNamespace(
        analysis=SimpleNamespace(
            records=report.records,
            diagnostics=report.diagnostics,
            surface_summary=tuple(
                sorted(report.surface_summary, key=lambda item: item["surface"])
            ),
        ),
        input_paths=("input.txt",),
    )
    schema_path = (
        Path(__file__).resolve().parents[2]
        / "verification/schemas/ignored-test-inventory-report.schema.json"
    )
    return root, json_path, markdown_path, schema_path, state


def fixture_report() -> IgnoredTestInventoryReport:
    record = IgnoredTestFact(
        discovery_id=stable_discovery_id(
            source_kind="playwright_test",
            package="trust-doc-captures",
            native_id="scripts/captures/example.spec.mjs#paused",
        ),
        native_id="scripts/captures/example.spec.mjs#paused",
        discovery_source_kind="playwright_test",
        name="paused",
        path="scripts/captures/example.spec.mjs",
        line=2,
        package="trust-doc-captures",
        command_hint="cd scripts/captures && npx playwright test example.spec.mjs",
        ignore_state="ignored",
        ignore_mechanism="playwright_literal_skip",
        ignore_reason="literal test.skip declaration",
        reference_candidates=(),
    )
    diagnostic = InventoryDiagnostic(
        severity="warning",
        kind="dynamic_playwright_skip",
        path="scripts/captures/dynamic.spec.mjs",
        line=3,
        message="dynamic Playwright skip cannot be assigned a stable test identity",
    )
    return IgnoredTestInventoryReport(
        provenance=InventoryProvenance(
            command=(
                "python3",
                "scripts/report_ignored_test_inventory.py",
                "--json-out",
                "target/gate-artifacts/verification/ignored-test-inventory.json",
                "--markdown-out",
                "docs/internal/testing/evidence/plc-verification-program/2026-07-10/p3-ignored-test-inventory.md",
                "--timestamp",
                "2026-07-10T12:00:00Z",
            ),
            commit="1" * 40,
            timestamp="2026-07-10T12:00:00Z",
            platform="linux-aarch64",
            input_paths=("scripts/captures/example.spec.mjs",),
            output_json="target/gate-artifacts/verification/ignored-test-inventory.json",
            output_markdown=(
                "docs/internal/testing/evidence/plc-verification-program/"
                "2026-07-10/p3-ignored-test-inventory.md"
            ),
        ),
        input_digest="sha256:" + "2" * 64,
        records=(record,),
        diagnostics=(diagnostic,),
        surface_summary=(
            {
                "surface": "playwright",
                "scanned_files": 1,
                "records": 1,
                "ignored": 1,
                "conditional": 0,
                "coverage": "mechanical",
                "note": "Only same-line literal test.skip calls in tracked capture specs are inventory facts.",
            },
            {
                "surface": "node",
                "scanned_files": 0,
                "records": 0,
                "ignored": 0,
                "conditional": 0,
                "coverage": "mechanical",
                "note": "Modeled VS Code facts and fail-closed excluded Node sentinel files are included in the scanned-file count.",
            },
            {
                "surface": "rust",
                "scanned_files": 0,
                "records": 0,
                "ignored": 0,
                "conditional": 0,
                "coverage": "mechanical",
                "note": "Modeled crate Rust facts and fail-closed xtask/fuzz sentinel files are included in the scanned-file count.",
            },
            {
                "surface": "shell",
                "scanned_files": 0,
                "records": 0,
                "ignored": 0,
                "conditional": 0,
                "coverage": "limitation",
                "note": "Shell source has no repository-wide static ignored-test identity convention.",
            },
            {
                "surface": "conformance",
                "scanned_files": 0,
                "records": 0,
                "ignored": 0,
                "conditional": 0,
                "coverage": "limitation",
                "note": "Runtime skipped results are not source ignore declarations.",
            },
        ),
        limitations=LIMITATIONS,
    )


if __name__ == "__main__":
    unittest.main()
