"""Tests for mechanical existing-test catalog discovery."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.verification.test_catalog_models import DEFAULT_JSON_PATH, DEFAULT_MARKDOWN_PATH
from scripts.verification.test_catalog_rust import scan_rust_tests
from scripts.verification.test_catalog_scanner import scan_repository, write_reports
from scripts.verification.test_catalog_st import scan_structured_text_tests
from scripts.verification.test_catalog_vscode import scan_vscode_tests
from scripts.verification.test_catalog_validation import (
    validate_payload_against_schema,
    validate_report_files,
    validate_report_payload,
    validate_schema_file,
)


class TestCatalogScannerTests(unittest.TestCase):
    def test_scans_every_board_surface_and_extracts_only_source_facts(self) -> None:
        with fixture_repo() as root:
            report = scan_repository(root)

        facts = report.inferred_facts
        self.assertEqual(len(facts), 10)
        self.assertEqual(
            {fact.source_kind for fact in facts},
            {
                "rust_integration_test",
                "rust_unit_test",
                "structured_text_test",
                "vscode_test",
                "conformance_case",
                "fuzz_target",
                "gate_script",
                "github_workflow_job",
            },
        )

        integration = fact_named(facts, "integration_waits")
        self.assertEqual(integration.package, "fixture-crate")
        self.assertEqual(integration.ignore_state, "ignored")
        self.assertEqual(integration.ignore_reason, "hardware")
        self.assertIn("VERIF-P2-001", integration.reference_candidates)
        self.assertIn("cargo test -p fixture-crate --test integration", integration.command_hint)

        unit = fact_named(facts, "unit_works")
        self.assertEqual(unit.source_kind, "rust_unit_test")
        self.assertEqual(unit.package, "fixture-crate")

        paused = fact_named(facts, "paused flow")
        self.assertEqual(paused.ignore_state, "ignored")
        self.assertEqual(paused.ignore_reason, "skip")
        self.assertEqual(paused.package, "fixture-vscode")

        conformance = fact_named(facts, "cfm_timer_001")
        self.assertIn("--bin trust-runtime", conformance.command_hint)
        self.assertIn("--filter cfm_timer_001", conformance.command_hint)

        fuzz = fact_named(facts, "syntax_parse")
        self.assertEqual(fuzz.command_hint, "cd fuzz && cargo fuzz run syntax_parse")

        gate = fact_named(facts, "sample_gate")
        self.assertEqual(gate.command_hint, "scripts/sample_gate.sh")
        self.assertIn("VERIF-P2-003", gate.reference_candidates)

        workflow = fact_named(facts, "Fixture CI / checks")
        self.assertEqual(workflow.path, ".github/workflows/fixture.yml")
        self.assertNotIn("run", {fact.name for fact in facts if fact.source_kind == "github_workflow_job"})

        st_program = fact_named(facts, "st_program_passes")
        self.assertEqual(st_program.source_kind, "structured_text_test")
        self.assertIn("--project crates/fixture/tests/st_project", st_program.command_hint)
        self.assertIn("--filter st_program_passes", st_program.command_hint)
        self.assertEqual(st_program.package, "fixture-crate")
        self.assertEqual(st_program.reference_candidates, ("VERIF-P2-001",))

        st_function_block = fact_named(facts, "st_fb_passes")
        self.assertIn("TEST_FUNCTION_BLOCK", st_function_block.native_id)
        self.assertEqual(st_function_block.reference_candidates, ())

        forbidden = {"test_class", "invariants", "oracle_ref", "expected_failure_mode", "evidence_destination"}
        for record in report.to_dict()["inferred_facts"]:
            self.assertFalse(forbidden & set(record), record)

    def test_payload_is_deterministic_and_separates_hand_owned_intent(self) -> None:
        with fixture_repo() as root:
            first = scan_repository(root)
            second = scan_repository(root)

        self.assertEqual(first.to_json(), second.to_json())
        payload = json.loads(first.to_json())
        self.assertEqual(payload["hand_owned_intent"]["included"], False)
        self.assertEqual(
            set(payload["hand_owned_intent"]["fields"]),
            {
                "subject_kind",
                "area",
                "owner",
                "status",
                "test_class",
                "invariants",
                "expected_result",
                "suite_tiers",
                "requires_hardware",
                "requires_network",
                "duration_class",
                "oracle_ref",
                "spec_gap_ref",
                "expected_failure_mode",
                "evidence_destination",
                "command",
                "last_reviewed",
            },
        )
        ordering = [
            (record["source_kind"], record["path"], record["line"], record["name"])
            for record in payload["inferred_facts"]
        ]
        self.assertEqual(ordering, sorted(ordering))

    def test_dynamic_vscode_title_is_reported_instead_of_silently_claimed(self) -> None:
        with fixture_repo() as root:
            path = root / "editors/vscode/src/test/suite/dynamic.test.ts"
            path.write_text(
                "const title = 'dynamic';\n"
                "const marker = 'this.skip()';\n"
                "const template = `\n"
                "test(\"template fake\", () => {});\n"
                "`;\n"
                "test(title, () => {});\n"
                "test(\"runtime conditional\", function () { this.skip(); });\n"
            )

            report = scan_repository(root)

        diagnostics = [item for item in report.diagnostics if item.path.endswith("dynamic.test.ts")]
        self.assertEqual(
            [item.kind for item in diagnostics],
            ["dynamic_test_name", "conditional_runtime_skip"],
        )
        self.assertFalse(
            {"dynamic", "template fake"} & {fact.name for fact in report.inferred_facts}
        )

    def test_rust_false_positives_are_ignored_and_conditional_ignore_is_visible(self) -> None:
        with fixture_repo() as root:
            path = root / "crates/fixture/src/false_positives.rs"
            write(
                path,
                r'''
// #[test]
// fn comment_fake() {}
const NORMAL: &str = "#[test] fn string_fake() {}";
const RAW: &str = r#"#[tokio::test] async fn raw_fake() {}"#;
const QUOTE: char = '"';
const BYTE_QUOTE: u8 = b'"';
/*
#[test]
fn block_fake() {}
*/
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "non-linux"
)]
#[test]
fn conditional_ignore() {}

#[test]
/* first line
continuation without a leading star
*/
fn block_comment_separated_test() {}
''',
            )

            report = scan_repository(root)

        names = {fact.name for fact in report.inferred_facts}
        self.assertFalse({"comment_fake", "string_fake", "raw_fake", "block_fake"} & names)
        conditional = fact_named(report.inferred_facts, "conditional_ignore")
        self.assertEqual(conditional.ignore_state, "conditional")
        self.assertEqual(conditional.ignore_reason, "non-linux")
        self.assertEqual(report.to_dict()["summary"]["ignored"], 2)
        self.assertEqual(report.to_dict()["summary"]["conditional_ignores"], 1)
        self.assertEqual(
            fact_named(report.inferred_facts, "block_comment_separated_test").source_kind,
            "rust_unit_test",
        )

    def test_discovery_ids_do_not_change_when_only_source_lines_move(self) -> None:
        with fixture_repo() as root:
            first = scan_repository(root)
            path = root / "crates/fixture/src/lib.rs"
            path.write_text("\n\n" + path.read_text())
            second = scan_repository(root)

        self.assertEqual(
            fact_named(first.inferred_facts, "unit_works").stable_id,
            fact_named(second.inferred_facts, "unit_works").stable_id,
        )

    def test_input_digest_changes_when_an_input_changes(self) -> None:
        with fixture_repo() as root:
            first = scan_repository(root)
            path = root / "scripts/sample_gate.sh"
            path.write_text(path.read_text() + "# source change\n")
            second = scan_repository(root)

        self.assertNotEqual(first.input_digest, second.input_digest)

    def test_git_scan_includes_untracked_nonignored_sources(self) -> None:
        with fixture_repo() as root:
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "add", "."], check=True)
            write(
                root / "crates/fixture/src/untracked.rs",
                "#[test]\nfn newly_added_test() {}\n",
            )

            report = scan_repository(root)

        self.assertEqual(fact_named(report.inferred_facts, "newly_added_test").package, "fixture-crate")

    def test_malformed_and_duplicate_conformance_manifests_are_diagnostics(self) -> None:
        with fixture_repo() as root:
            write(
                root / "conformance/cases/duplicate/manifest.toml",
                'id = "cfm_timer_001"\ncategory = "duplicate"\n',
            )
            write(root / "conformance/cases/malformed/manifest.toml", "id = [\n")

            report = scan_repository(root)

        kinds = {item.kind for item in report.diagnostics}
        self.assertIn("duplicate_conformance_id", kinds)
        self.assertIn("conformance_manifest_parse", kinds)

    def test_semantic_validator_rejects_extra_fields_and_summary_tampering(self) -> None:
        with fixture_repo() as root:
            payload = scan_repository(root).to_dict()

        self.assertEqual(validate_report_payload(payload), [])
        payload["unexpected"] = True
        payload["summary"]["records"] += 1
        failures = validate_report_payload(payload)
        self.assertTrue(any("top-level has unexpected fields" in failure for failure in failures))
        self.assertTrue(any("summary.records" in failure for failure in failures))

    def test_schema_and_source_contracts_reject_report_tampering(self) -> None:
        with fixture_repo() as root:
            schema_payload = scan_repository(root).to_dict()
            contract_payload = scan_repository(root).to_dict()
        schema_path = Path(__file__).resolve().parents[2] / "verification/schemas/generated-test-catalog.schema.json"
        schema = json.loads(schema_path.read_text())

        schema_payload["output_paths"]["json"] = "target/gate-artifacts/verification/not-json.txt"
        self.assertTrue(validate_payload_against_schema(schema_payload, schema))

        conformance = next(
            fact
            for fact in contract_payload["inferred_facts"]
            if fact["source_kind"] == "conformance_case"
        )
        conformance["command_hint"] = "echo not-a-conformance-run"
        failures = validate_report_payload(contract_payload)
        self.assertTrue(any("conformance_case command_hint" in failure for failure in failures))

    def test_committed_generated_catalog_schema_matches_semantic_contract(self) -> None:
        root = Path(__file__).resolve().parents[2]
        schema = root / "verification/schemas/generated-test-catalog.schema.json"
        self.assertEqual(validate_schema_file(schema), [])

        mutated = json.loads(schema.read_text())
        mutated["properties"]["timestamp"]["maxLength"] = 128
        with tempfile.TemporaryDirectory() as temp:
            mutated_path = Path(temp) / "generated-test-catalog.schema.json"
            mutated_path.write_text(json.dumps(mutated))
            failures = validate_schema_file(mutated_path)
        self.assertTrue(any("unsupported schema keyword maxLength" in item for item in failures))

    def test_live_repository_census_is_reviewed_evidence_tripwire(self) -> None:
        """Count drift requires an intentional evidence refresh, not a silent baseline move."""

        root = Path(__file__).resolve().parents[2]
        rust = scan_rust_tests(root)
        structured_text = scan_structured_text_tests(root)
        vscode = scan_vscode_tests(root)

        self.assertEqual(len(rust.facts), 3104)
        self.assertEqual(len(structured_text.facts), 257)
        self.assertEqual(len(vscode.facts), 456)
        runtime_core = [
            fact
            for fact in rust.facts
            if fact.source_kind == "rust_unit_test" and fact.package == "trust-runtime-core"
        ]
        self.assertEqual(len(runtime_core), 69)

    def test_report_writes_default_artifact_shape_and_concise_summary(self) -> None:
        with fixture_repo() as root, tempfile.TemporaryDirectory() as temp:
            output_root = Path(temp)
            json_path = output_root / DEFAULT_JSON_PATH
            markdown_path = output_root / DEFAULT_MARKDOWN_PATH
            report = scan_repository(root)

            write_reports(report, json_path=json_path, markdown_path=markdown_path)

            payload = json.loads(json_path.read_text())
            markdown = markdown_path.read_text()
            schema = Path(__file__).resolve().parents[2] / "verification/schemas/generated-test-catalog.schema.json"
            self.assertEqual(validate_report_files(root, json_path, markdown_path, schema), [])
            markdown_path.write_text(markdown.replace("Generated JSON SHA-256:", "Forged JSON SHA-256:"))
            self.assertTrue(
                any(
                    "Markdown is missing bound marker" in failure
                    for failure in validate_report_files(root, json_path, markdown_path, schema)
                )
            )

        self.assertEqual(payload["schema_version"], 1)
        self.assertEqual(payload["generator"], "test-catalog-scanner")
        self.assertEqual(payload["generator_version"], 2)
        self.assertEqual(payload["summary"]["records"], 10)
        self.assertIn("Generated Existing-Test Catalog", markdown)
        self.assertIn("does not map tests to claims", markdown)
        self.assertIn("rust_integration_test", markdown)
        self.assertTrue(any("xtask" in item for item in payload["limitations"]))
        self.assertTrue(any("crate-local fuzz" in item for item in payload["limitations"]))


def fact_named(facts, name: str):
    matches = [fact for fact in facts if fact.name == name]
    if len(matches) != 1:
        raise AssertionError(f"expected exactly one fact named {name!r}, found {len(matches)}")
    return matches[0]


class fixture_repo:
    def __init__(self) -> None:
        self._temp = tempfile.TemporaryDirectory()
        self.root = Path(self._temp.name)

    def __enter__(self) -> Path:
        write(
            self.root / "crates/fixture/Cargo.toml",
            '[package]\nname = "fixture-crate"\nversion = "0.1.0"\n',
        )
        write(
            self.root / "crates/fixture/tests/integration.rs",
            """
// VERIF-P2-001 EVID_FIXTURE_001
#[ignore = "hardware"]
#[tokio::test]
async fn integration_waits() {}
""",
        )
        write(
            self.root / "crates/fixture/src/lib.rs",
            """
#[cfg(test)]
mod tests {
    #[test]
    fn unit_works() {}
}
""",
        )
        write(
            self.root / "crates/fixture/tests/st_project/src/tests.st",
            """
// TEST_PROGRAM line_comment_fake
(* TEST_FUNCTION_BLOCK block_comment_fake *)
VAR_GLOBAL
    FakeText : STRING := 'TEST_PROGRAM string_fake';
END_VAR

// VERIF-P2-001
TEST_PROGRAM st_program_passes
ASSERT_TRUE(TRUE);
END_TEST_PROGRAM

TEST_FUNCTION_BLOCK st_fb_passes
ASSERT_TRUE(TRUE);
END_TEST_FUNCTION_BLOCK
""",
        )
        write(
            self.root / "editors/vscode/package.json",
            '{"name":"fixture-vscode"}\n',
        )
        write(
            self.root / "editors/vscode/src/test/suite/sample.test.ts",
            """
test.skip("paused flow", () => {});
test("runs flow", () => {});
""",
        )
        write(
            self.root / "conformance/cases/timers/cfm_timer_001/manifest.toml",
            'id = "cfm_timer_001"\ncategory = "timers"\n',
        )
        write(
            self.root / "fuzz/Cargo.toml",
            """
[package]
name = "fixture-fuzz"
version = "0.0.0"

[[bin]]
name = "syntax_parse"
path = "fuzz_targets/syntax_parse.rs"
""",
        )
        write(self.root / "fuzz/fuzz_targets/syntax_parse.rs", "#![no_main]\n")
        write(
            self.root / "scripts/sample_gate.sh",
            "#!/usr/bin/env bash\n# VERIF-P2-003\nexit 0\n",
        )
        write(
            self.root / ".github/workflows/fixture.yml",
            """
name: Fixture CI
jobs:
  checks:
    name: Checks
    runs-on: ubuntu-latest
    steps:
      - name: Run
        run: scripts/sample_gate.sh
""",
        )
        return self.root

    def __exit__(self, exc_type, exc, tb) -> None:
        self._temp.cleanup()


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content.lstrip())


if __name__ == "__main__":
    unittest.main()
