"""Tests for deterministic unmapped-test debt reporting."""

from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.test_catalog_common import make_fact
from scripts.verification.test_catalog_common import input_digest
from scripts.verification.test_catalog_debt import (
    UnmappedDebtProvenance,
    UnmappedTestDebtReport,
    analyze_unmapped_test_debt,
)
from scripts.verification.test_catalog_debt_cli import default_command, main
from scripts.verification.test_catalog_debt_validation import (
    _validate_input_binding,
    _validate_source_commit,
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
)
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance
from scripts.verification.test_catalog_scanner import scan_repository


class UnmappedTestDebtTests(unittest.TestCase):
    def test_default_command_rejects_missing_timestamp(self) -> None:
        with self.assertRaisesRegex(ValueError, "timestamp is required"):
            default_command(Path("debt.json"), Path("debt.md"), "")

    def test_exact_generated_discovery_ids_are_the_only_classification_basis(self) -> None:
        mapped = _fact("mapped", line=10)
        unmapped = _fact("unmapped", line=20, ignore_state="ignored")
        tests = [
            {
                "id": "TEST_MAPPED",
                "subject_kind": "generated_test",
                "discovery_id": mapped.stable_id,
            },
            {
                "id": "TEST_ARTIFACT",
                "subject_kind": "case_table_artifact",
                "discovery_id": unmapped.stable_id,
            },
        ]

        analysis = analyze_unmapped_test_debt(tests=tests, facts=[unmapped, mapped])

        self.assertEqual(analysis["summary"]["scanner_facts"], 2)
        self.assertEqual(analysis["summary"]["mapped_scanner_facts"], 1)
        self.assertEqual(analysis["summary"]["unmapped_scanner_facts"], 1)
        self.assertEqual(
            analysis["unmapped_tests"],
            [
                {
                    "discovery_id": unmapped.stable_id,
                    "source_kind": unmapped.source_kind,
                    "path": unmapped.path,
                    "name": unmapped.name,
                    "ignore_state": "ignored",
                }
            ],
        )

    def test_output_is_canonical_when_scanner_and_catalog_inputs_are_reordered(self) -> None:
        facts, tests = _fixture_inputs()

        forward = analyze_unmapped_test_debt(tests=tests, facts=facts)
        reverse = analyze_unmapped_test_debt(tests=list(reversed(tests)), facts=list(reversed(facts)))

        self.assertEqual(forward, reverse)
        identities = [
            (
                row["source_kind"],
                row["path"],
                row["name"],
                row["discovery_id"],
            )
            for row in forward["unmapped_tests"]
        ]
        self.assertEqual(identities, sorted(identities))

    def test_stale_and_duplicate_discovery_bindings_fail_closed(self) -> None:
        facts, tests = _fixture_inputs()
        stale = copy.deepcopy(tests)
        stale[0]["discovery_id"] = "DISC_00000000000000000000"
        with self.assertRaisesRegex(ValueError, "absent from current scanner facts"):
            analyze_unmapped_test_debt(tests=stale, facts=facts)

        duplicate_binding = copy.deepcopy(tests)
        duplicate_binding.append(
            {
                "id": "TEST_DUPLICATE_BINDING",
                "subject_kind": "generated_test",
                "discovery_id": facts[0].stable_id,
            }
        )
        with self.assertRaisesRegex(ValueError, "classified by both"):
            analyze_unmapped_test_debt(tests=duplicate_binding, facts=facts)

        with self.assertRaisesRegex(ValueError, "scanner duplicates discovery id"):
            analyze_unmapped_test_debt(tests=tests, facts=[facts[0], facts[0]])

    def test_debt_report_is_complete_and_lists_every_unmapped_identity(self) -> None:
        report = _fixture_report()
        rendered_json = report.to_json().encode()
        markdown = report.to_markdown(json_digest=hashlib.sha256(rendered_json).hexdigest())

        self.assertEqual(report.to_dict()["report_status"], "complete")
        self.assertFalse(report.to_dict()["scope"]["debt_is_report_failure"])
        for row in report.analysis["unmapped_tests"]:
            self.assertIn(row["discovery_id"], markdown)
            self.assertIn(row["path"], markdown)
            self.assertIn(row["name"], markdown)
            self.assertIn(row["ignore_state"], markdown)
        self.assertEqual(
            validate_report_payload(report.to_dict(), expected_analysis=report.analysis),
            [],
        )
        self.assertEqual(validate_markdown_binding(report.to_dict(), rendered_json, markdown), [])
        self.assertTrue(
            validate_markdown_binding(
                report.to_dict(),
                rendered_json,
                markdown + "\nContradictory appendix.\n",
            )
        )
        compact_json = json.dumps(report.to_dict(), sort_keys=True).encode()
        compact_markdown = report.to_markdown(
            json_digest=hashlib.sha256(compact_json).hexdigest()
        )
        self.assertTrue(
            validate_markdown_binding(
                report.to_dict(),
                compact_json,
                compact_markdown,
            )
        )

    def test_corrupt_or_tampered_report_fails_semantic_and_markdown_validation(self) -> None:
        report = _fixture_report()
        original_json = report.to_json().encode()
        markdown = report.to_markdown(json_digest=hashlib.sha256(original_json).hexdigest())
        tampered = copy.deepcopy(report.to_dict())
        tampered["unmapped_tests"].pop()
        tampered["summary"]["unmapped_scanner_facts"] -= 1
        tampered_json = (json.dumps(tampered, indent=2, sort_keys=True) + "\n").encode()

        failures = validate_report_payload(tampered, expected_analysis=report.analysis)
        binding_failures = validate_markdown_binding(tampered, tampered_json, markdown)

        self.assertTrue(any("does not match current debt analysis" in item for item in failures))
        self.assertTrue(any("Generated JSON SHA-256" in item for item in binding_failures))

        corrupt = copy.deepcopy(report.to_dict())
        corrupt["unmapped_tests"][0]["unexpected"] = True
        self.assertTrue(
            any("unexpected fields" in item for item in validate_report_payload(corrupt))
        )

    def test_report_rejects_noncanonical_unmapped_order(self) -> None:
        payload = _fixture_report().to_dict()
        payload["unmapped_tests"] = list(reversed(payload["unmapped_tests"]))

        self.assertIn(
            "unmapped_tests must use canonical source/path/name/discovery order",
            validate_report_payload(payload),
        )

    def test_closed_schema_accepts_report_and_rejects_extra_fields(self) -> None:
        payload = _fixture_report().to_dict()
        schema = json.loads(
            (ROOT / "verification/schemas/unmapped-test-debt-report.schema.json").read_text()
        )

        self.assertEqual(validate_schema_contract(schema), [])
        self.assertEqual(validate_json_schema_instance(payload, schema), [])
        dirty_payload = copy.deepcopy(payload)
        dirty_payload["commit"] = "dirty:" + "0" * 40
        self.assertTrue(
            any(
                "commit" in failure
                for failure in validate_json_schema_instance(dirty_payload, schema)
            )
        )
        payload["unexpected"] = True
        self.assertIn(
            "$: additional property unexpected is forbidden",
            validate_json_schema_instance(payload, schema),
        )

    def test_report_only_cli_returns_zero_when_debt_is_nonzero(self) -> None:
        report = _fixture_report()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            json_path = root / "debt.json"
            markdown_path = root / "debt.md"
            with (
                patch(
                    "scripts.verification.test_catalog_debt_cli.generate_report",
                    return_value=report,
                ),
                patch(
                    "scripts.verification.test_catalog_debt_cli.validate_report_files",
                    return_value=[],
                ),
            ):
                result = main(
                    [
                        "--root",
                        str(root),
                        "--json-out",
                        str(json_path),
                        "--markdown-out",
                        str(markdown_path),
                        "--timestamp",
                        "2026-07-10T12:00:00Z",
                    ]
                )

        self.assertEqual(result, 0)

    def test_provenance_command_timestamp_commit_and_input_tampering_fail(self) -> None:
        payload = _fixture_report().to_dict()
        payload["command"] = ["false"]
        self.assertIn(
            "command does not match canonical unmapped-test debt invocation",
            validate_report_payload(payload),
        )

        payload = _fixture_report().to_dict()
        payload["timestamp"] = "not-an-iso-time"
        payload["command"][-1] = "not-an-iso-time"
        self.assertIn(
            "timestamp must be an ISO-8601 value with a timezone",
            validate_report_payload(payload),
        )
        self.assertIn(
            "commit does not resolve in the repository: " + "f" * 40,
            _validate_source_commit(ROOT, "f" * 40, ["verification/test-catalog.toml"]),
        )
        self.assertEqual(
            _validate_source_commit(
                ROOT,
                "dirty:" + _git_head(),
                ["verification/test-catalog.toml"],
            ),
            ["commit must identify a clean source revision for at-rest validation"],
        )
        payload = _fixture_report().to_dict()
        payload["commit"] = "dirty:" + _git_head()
        self.assertIn("commit must be a clean full Git SHA", validate_report_payload(payload))

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "input.txt").write_text("bound\n")
            payload = _fixture_report().to_dict()
            payload["input_paths"] = ["input.txt"]
            payload["input_digest"] = input_digest(root, ["input.txt"])
            self.assertEqual(_validate_input_binding(root, payload, ["input.txt"]), [])
            payload["input_digest"] = "sha256:" + "f" * 64
            self.assertIn(
                "input_digest does not match current report inputs",
                _validate_input_binding(root, payload, ["input.txt"]),
            )

    def test_markdown_content_tampering_is_rejected(self) -> None:
        report = _fixture_report()
        json_bytes = report.to_json().encode()
        markdown = report.to_markdown(json_digest=hashlib.sha256(json_bytes).hexdigest())
        tampered = markdown.replace("- Debt fails this report: no", "- Debt fails this report: yes")

        failures = validate_markdown_binding(report.to_dict(), json_bytes, tampered)
        self.assertTrue(any("Debt fails this report: no" in failure for failure in failures))
        self.assertIn(
            "unmapped-test debt Markdown does not exactly match JSON",
            failures,
        )

    def test_live_repository_baseline_lists_all_unmapped_identities(self) -> None:
        scan = scan_repository(ROOT, timestamp="2026-07-10T12:00:00Z")
        catalog = tomllib.loads((ROOT / "verification/test-catalog.toml").read_text())

        analysis = analyze_unmapped_test_debt(
            tests=catalog["tests"],
            facts=scan.inferred_facts,
        )

        self.assertEqual(analysis["summary"]["scanner_facts"], 3957)
        self.assertEqual(analysis["summary"]["mapped_scanner_facts"], 175)
        self.assertEqual(analysis["summary"]["unmapped_scanner_facts"], 3782)
        self.assertEqual(len(analysis["unmapped_tests"]), 3782)
        self.assertEqual(
            len({row["discovery_id"] for row in analysis["unmapped_tests"]}),
            3782,
        )


def _fact(name: str, *, line: int, ignore_state: str = "not_ignored"):
    return make_fact(
        source_kind="rust_unit_test",
        name=name,
        path=f"crates/trust-runtime/src/{name}.rs",
        line=line,
        package="trust-runtime",
        command_hint=f"cargo test -p trust-runtime {name} -- --exact",
        command_hint_authority="exact",
        discovery_confidence="exact_attribute",
        ignore_state=ignore_state,
        ignore_reason="fixture" if ignore_state != "not_ignored" else None,
    )


def _fixture_inputs():
    mapped = _fact("mapped", line=10)
    ignored = _fact("ignored_unmapped", line=30, ignore_state="ignored")
    ordinary = _fact("ordinary_unmapped", line=20)
    tests = [
        {
            "id": "TEST_MAPPED",
            "subject_kind": "generated_test",
            "discovery_id": mapped.stable_id,
        },
        {
            "id": "TEST_ARTIFACT",
            "subject_kind": "case_table_artifact",
            "discovery_id": ordinary.stable_id,
        },
    ]
    return [mapped, ignored, ordinary], tests


def _fixture_report() -> UnmappedTestDebtReport:
    facts, tests = _fixture_inputs()
    analysis = analyze_unmapped_test_debt(tests=tests, facts=facts)
    return UnmappedTestDebtReport(
        provenance=UnmappedDebtProvenance(
            command=(
                "python3",
                "scripts/report_unmapped_test_debt.py",
                "--json-out",
                "target/gate-artifacts/verification/unmapped-test-debt.json",
                "--markdown-out",
                "target/gate-artifacts/verification/unmapped-test-debt.md",
                "--timestamp",
                "2026-07-10T12:00:00Z",
            ),
            commit="0" * 40,
            timestamp="2026-07-10T12:00:00Z",
            platform="linux-test",
            input_paths=("verification/test-catalog.toml",),
            output_json="target/gate-artifacts/verification/unmapped-test-debt.json",
            output_markdown="target/gate-artifacts/verification/unmapped-test-debt.md",
        ),
        input_digest="sha256:" + "1" * 64,
        analysis=analysis,
    )


def _git_head() -> str:
    import subprocess

    return subprocess.run(
        ["git", "-C", str(ROOT), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


if __name__ == "__main__":
    unittest.main()
