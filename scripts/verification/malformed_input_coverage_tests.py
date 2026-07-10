"""Tests for reviewed malformed-input taxonomy and coverage reporting."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import tomllib
import unittest
from dataclasses import replace
from pathlib import Path

from scripts.verification.malformed_input_contract import (
    load_malformed_input_taxonomy,
    validate_catalog_malformed_bindings,
    validate_malformed_input_contract,
    validate_taxonomy_schema_contract,
)
from scripts.verification.malformed_input_coverage import (
    MalformedCoverageProvenance,
    MalformedInputCoverageReport,
    analyze_malformed_input_coverage,
)
from scripts.verification.malformed_input_coverage_validation import (
    _validate_input_binding,
    _validate_source_commit,
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
)
from scripts.verification.test_catalog_common import make_fact
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


ROOT = Path(__file__).resolve().parents[2]


class MalformedInputCoverageTests(unittest.TestCase):
    def test_committed_taxonomy_schema_and_review_doc_are_in_sync(self) -> None:
        taxonomy = load_malformed_input_taxonomy(ROOT)

        failures = validate_malformed_input_contract(ROOT, taxonomy)

        self.assertEqual(failures, [])
        self.assertEqual(taxonomy["area"], "bytecode_vm")
        self.assertEqual(taxonomy["surface_id"], "bytecode_container_instruction_stream")
        self.assertIn("bad_magic", {item["id"] for item in taxonomy["classes"]})

    def test_negative_tests_require_reviewed_classes_and_artifacts_forbid_them(self) -> None:
        taxonomy = fixture_taxonomy()
        negative = fixture_test()
        negative.pop("malformed_input_class_ids")
        artifact = {
            **fixture_test(),
            "id": "TEST_ARTIFACT",
            "subject_kind": "case_table_artifact",
            "test_class": "metadata_validation",
        }

        failures = validate_catalog_malformed_bindings(
            tests={negative["id"]: negative, artifact["id"]: artifact},
            taxonomy=taxonomy,
        )

        self.assertTrue(any("negative_malformed_input requires malformed_input_class_ids" in item for item in failures))
        self.assertTrue(any("case_table_artifact forbids malformed_input_class_ids" in item for item in failures))

    def test_unrelated_generated_classes_and_duplicate_bindings_are_rejected(self) -> None:
        taxonomy = fixture_taxonomy()
        unrelated = fixture_test()
        unrelated["test_class"] = "integration"
        duplicate = fixture_test()
        duplicate["id"] = "TEST_DUPLICATE"
        duplicate["malformed_input_class_ids"] = ["bad_magic", "bad_magic"]

        failures = validate_catalog_malformed_bindings(
            tests={unrelated["id"]: unrelated, duplicate["id"]: duplicate},
            taxonomy=taxonomy,
        )

        self.assertTrue(any("test_class 'integration' forbids malformed_input_class_ids" in item for item in failures))
        self.assertTrue(any("duplicates malformed_input_class_ids" in item for item in failures))

    def test_taxonomy_ids_disposition_fields_schema_and_review_doc_fail_closed(self) -> None:
        taxonomy = load_malformed_input_taxonomy(ROOT)
        duplicate = copy.deepcopy(taxonomy)
        duplicate["classes"].append(copy.deepcopy(duplicate["classes"][0]))
        coupling = copy.deepcopy(taxonomy)
        bad_magic = next(item for item in coupling["classes"] if item["id"] == "bad_magic")
        bad_magic["disposition"] = "spec_gap"
        review_drift = copy.deepcopy(taxonomy)
        review_drift["classes"][0]["title"] = "Drifted title"
        schema = json.loads(
            (ROOT / "verification/schemas/malformed-input-taxonomy.schema.json").read_text()
        )
        schema["additionalProperties"] = True

        self.assertTrue(any("duplicates class IDs" in item for item in validate_malformed_input_contract(ROOT, duplicate)))
        self.assertTrue(any("fields do not match spec_gap disposition" in item for item in validate_malformed_input_contract(ROOT, coupling)))
        self.assertTrue(any("review document class table drifts" in item for item in validate_malformed_input_contract(ROOT, review_drift)))
        self.assertIn(
            "malformed-input taxonomy schema root must be a closed object",
            validate_taxonomy_schema_contract(schema),
        )

    def test_blocked_deferred_and_not_applicable_authority_fields_fail_closed(self) -> None:
        taxonomy = load_malformed_input_taxonomy(ROOT)
        blocked = copy.deepcopy(taxonomy)
        blocked_class = blocked["classes"][0]
        blocked_class.pop("spec_gap_ref")
        blocked_class.update(disposition="blocked", blocker_ref="")
        deferred = copy.deepcopy(taxonomy)
        deferred_class = deferred["classes"][0]
        deferred_class["disposition"] = "deferred"
        not_applicable = copy.deepcopy(taxonomy)
        not_applicable_class = not_applicable["classes"][0]
        not_applicable_class.pop("spec_gap_ref")
        not_applicable_class.update(
            disposition="not_applicable",
            decision_ref="SPEC_IEC_DECISIONS_001",
        )

        self.assertTrue(
            any(
                "blocked disposition requires blocker_ref" in item
                for item in validate_malformed_input_contract(ROOT, blocked)
            )
        )
        self.assertTrue(
            any(
                "fields do not match deferred disposition" in item
                for item in validate_malformed_input_contract(ROOT, deferred)
            )
        )
        self.assertTrue(
            any(
                "not_applicable requires active same-area reviewed decision/deviation" in item
                for item in validate_malformed_input_contract(ROOT, not_applicable)
            )
        )

    def test_committed_catalog_binds_only_invalid_magic_to_bad_magic(self) -> None:
        catalog = tomllib.loads((ROOT / "verification/test-catalog.toml").read_text())
        mappings = {
            record["id"]: record["malformed_input_class_ids"]
            for record in catalog["tests"]
            if "malformed_input_class_ids" in record
        }

        self.assertEqual(
            mappings,
            {"TEST_BYTECODE_CONTAINER_INVALID_MAGIC": ["bad_magic"]},
        )

    def test_catalog_classes_must_exist_and_match_the_test_area(self) -> None:
        taxonomy = fixture_taxonomy()
        unknown = fixture_test()
        unknown["malformed_input_class_ids"] = ["does_not_exist"]
        wrong_area = fixture_test()
        wrong_area["id"] = "TEST_WRONG_AREA"
        wrong_area["area"] = "runtime_safety"

        failures = validate_catalog_malformed_bindings(
            tests={unknown["id"]: unknown, wrong_area["id"]: wrong_area},
            taxonomy=taxonomy,
        )

        self.assertTrue(any("unknown malformed-input class does_not_exist" in item for item in failures))
        self.assertTrue(any("malformed-input class bad_magic area bytecode_vm" in item for item in failures))

    def test_states_are_derived_without_name_or_path_inference(self) -> None:
        facts, tests = fixture_facts_and_tests()

        analysis = analyze_malformed_input_coverage(
            taxonomy=fixture_taxonomy(),
            tests=tests,
            facts=facts,
        )

        rows = {item["class_id"]: item for item in analysis["classes"]}
        self.assertEqual(rows["bad_magic"]["state"], "covered")
        self.assertEqual(rows["unsupported_version"]["state"], "gap_open")
        self.assertEqual(rows["unknown_opcode"]["state"], "spec_gap")
        self.assertEqual(rows["bad_magic"]["mapped_test_ids"], ["TEST_MAGIC"])
        self.assertEqual(analysis["summary"]["by_state"]["covered"], 1)

    def test_unmapped_name_and_path_cannot_create_bad_magic_coverage(self) -> None:
        named_fact = make_fact(
            source_kind="rust_integration_test",
            name="bad_magic",
            path="crates/trust-runtime/tests/bad_magic.rs",
            line=10,
            package="trust-runtime",
            command_hint="cargo test -p trust-runtime --test bad_magic bad_magic -- --exact",
            command_hint_authority="exact",
            discovery_confidence="exact_attribute",
        )

        analysis = analyze_malformed_input_coverage(
            taxonomy=fixture_taxonomy(),
            tests=[],
            facts=[named_fact],
        )

        bad_magic = next(item for item in analysis["classes"] if item["class_id"] == "bad_magic")
        self.assertEqual(bad_magic["state"], "gap_open")
        self.assertEqual(bad_magic["mapped_test_ids"], [])

    def test_fuzz_only_and_ignored_mappings_do_not_become_normal_coverage(self) -> None:
        facts, tests = fixture_facts_and_tests()
        fuzz = {**tests[0], "id": "TEST_FUZZ", "test_class": "fuzz"}
        tests = [fuzz]

        fuzz_analysis = analyze_malformed_input_coverage(
            taxonomy=fixture_taxonomy(),
            tests=tests,
            facts=facts,
        )
        ignored_analysis = analyze_malformed_input_coverage(
            taxonomy=fixture_taxonomy(),
            tests=[fixture_test()],
            facts=[replace(facts[0], ignore_state="ignored", ignore_reason="fixture")],
        )

        fuzz_row = next(item for item in fuzz_analysis["classes"] if item["class_id"] == "bad_magic")
        ignored_row = next(item for item in ignored_analysis["classes"] if item["class_id"] == "bad_magic")
        self.assertEqual(fuzz_row["state"], "covered_by_fuzz")
        self.assertEqual(ignored_row["state"], "gap_open")
        self.assertEqual(ignored_row["non_runnable_test_ids"], ["TEST_MAGIC"])

    def test_report_debt_is_complete_and_semantic_tampering_is_rejected(self) -> None:
        report = fixture_report()
        payload = report.to_dict()
        rendered_json = report.to_json().encode()
        digest = hashlib.sha256(rendered_json).hexdigest()
        markdown = report.to_markdown(json_digest=digest)

        self.assertEqual(payload["report_status"], "complete")
        self.assertFalse(payload["scope"]["debt_is_report_failure"])
        self.assertIn("- `gap_open`: 1", markdown)
        self.assertEqual(validate_report_payload(payload, expected_analysis=report.analysis), [])
        self.assertEqual(validate_markdown_binding(payload, rendered_json, markdown), [])
        self.assertTrue(
            validate_markdown_binding(
                payload,
                rendered_json,
                markdown + "\nContradictory appendix.\n",
            )
        )
        compact_json = json.dumps(payload, sort_keys=True).encode()
        compact_markdown = report.to_markdown(
            json_digest=hashlib.sha256(compact_json).hexdigest()
        )
        self.assertTrue(
            validate_markdown_binding(payload, compact_json, compact_markdown)
        )

        tampered = copy.deepcopy(payload)
        next(item for item in tampered["classes"] if item["class_id"] == "unsupported_version")[
            "state"
        ] = "covered"
        failures = validate_report_payload(tampered, expected_analysis=report.analysis)
        self.assertTrue(any("classes does not match current malformed-input analysis" in item for item in failures))

    def test_closed_report_schema_accepts_report_and_rejects_extra_fields(self) -> None:
        schema = json.loads(
            (ROOT / "verification/schemas/malformed-input-coverage-report.schema.json").read_text()
        )
        payload = fixture_report().to_dict()

        self.assertEqual(validate_schema_contract(schema), [])
        self.assertEqual(validate_json_schema_instance(payload, schema), [])
        payload["unexpected"] = True
        self.assertIn(
            "$: additional property unexpected is forbidden",
            validate_json_schema_instance(payload, schema),
        )

        drifted = copy.deepcopy(schema)
        drifted["$defs"]["class"]["properties"]["state"]["enum"].append("invented")
        self.assertIn(
            "malformed-input report schema class state enum drifts",
            validate_schema_contract(drifted),
        )

    def test_at_rest_provenance_input_and_markdown_tampering_is_rejected(self) -> None:
        report = fixture_report()
        payload = report.to_dict()
        payload["command"] = ["false"]
        self.assertIn(
            "command does not match canonical malformed-input generator invocation",
            validate_report_payload(payload, expected_analysis=report.analysis),
        )

        payload = report.to_dict()
        payload["timestamp"] = "not-an-iso-time"
        payload["command"][-1] = "not-an-iso-time"
        self.assertIn(
            "timestamp must be an ISO-8601 value with a timezone",
            validate_report_payload(payload, expected_analysis=report.analysis),
        )

        payload = report.to_dict()
        input_failures = _validate_input_binding(
            ROOT,
            payload,
            ["verification/test-catalog.toml"],
        )
        self.assertTrue(any("input_paths do not match" in item for item in input_failures))
        self.assertTrue(any("input_digest does not match" in item for item in input_failures))
        self.assertTrue(any("commit does not resolve" in item for item in input_failures))

        rendered_json = report.to_json().encode()
        markdown = report.to_markdown(json_digest="0" * 64)
        self.assertTrue(validate_markdown_binding(report.to_dict(), rendered_json, markdown))

    def test_dirty_source_commits_are_rejected_by_payload_schema_and_at_rest(self) -> None:
        commit = subprocess.run(
            ["git", "-C", str(ROOT), "rev-parse", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        payload = fixture_report().to_dict()
        payload["commit"] = f"dirty:{commit}"
        schema = json.loads(
            (ROOT / "verification/schemas/malformed-input-coverage-report.schema.json").read_text()
        )

        self.assertIn(
            "commit must be a clean full Git SHA",
            validate_report_payload(payload, expected_analysis=fixture_report().analysis),
        )
        self.assertTrue(
            any("does not match" in failure for failure in validate_json_schema_instance(payload, schema))
        )
        self.assertEqual(
            _validate_source_commit(ROOT, payload["commit"], ["verification/test-catalog.toml"]),
            ["commit must be a clean full Git SHA for at-rest validation"],
        )


def fixture_taxonomy() -> dict:
    return {
        "schema_version": 1,
        "id": "MALFORMED_INPUT_BYTECODE_VM_V1",
        "title": "Bytecode malformed-input taxonomy",
        "area": "bytecode_vm",
        "surface_id": "bytecode_container_instruction_stream",
        "review_doc": "verification/malformed-input-taxonomy.md",
        "last_reviewed": "2026-07-10",
        "classes": [
            {
                "id": "bad_magic",
                "title": "Bad bytecode magic",
                "disposition": "required",
                "oracle_ref": "SPEC_BYTECODE_FORMAT_001",
                "rationale": "STBC magic is written in the bytecode format contract.",
            },
            {
                "id": "unsupported_version",
                "title": "Unsupported bytecode version",
                "disposition": "required",
                "oracle_ref": "SPEC_BYTECODE_FORMAT_001",
                "rationale": "Supported major-version rejection is written in the bytecode format contract.",
            },
            {
                "id": "unknown_opcode",
                "title": "Unknown opcode",
                "disposition": "spec_gap",
                "spec_gap_ref": "SPEC_GAP_BYTECODE_VALIDATOR_001",
                "rationale": "The validator semantic contract remains open.",
            },
        ],
    }


def fixture_test() -> dict:
    return {
        "id": "TEST_MAGIC",
        "subject_kind": "generated_test",
        "test_class": "negative_malformed_input",
        "area": "bytecode_vm",
        "status": "mapped",
        "discovery_id": fixture_fact().stable_id,
        "malformed_input_class_ids": ["bad_magic"],
    }


def fixture_fact():
    return make_fact(
        source_kind="rust_integration_test",
        name="opaque_fixture_name",
        path="crates/trust-runtime/tests/opaque_fixture.rs",
        line=10,
        package="trust-runtime",
        command_hint="cargo test -p trust-runtime --test opaque_fixture opaque_fixture_name -- --exact",
        command_hint_authority="exact",
        discovery_confidence="exact_attribute",
    )


def fixture_facts_and_tests() -> tuple[list, list[dict]]:
    return [fixture_fact()], [fixture_test()]


def fixture_report() -> MalformedInputCoverageReport:
    facts, tests = fixture_facts_and_tests()
    analysis = analyze_malformed_input_coverage(
        taxonomy=fixture_taxonomy(),
        tests=tests,
        facts=facts,
    )
    return MalformedInputCoverageReport(
        provenance=MalformedCoverageProvenance(
            command=(
                "python3",
                "scripts/report_malformed_input_coverage.py",
                "--json-out",
                "target/gate-artifacts/verification/malformed-input-coverage.json",
                "--markdown-out",
                "target/gate-artifacts/verification/malformed-input-coverage.md",
                "--timestamp",
                "2026-07-10T12:00:00Z",
            ),
            commit="0" * 40,
            timestamp="2026-07-10T12:00:00Z",
            platform="linux-test",
            input_paths=(
                "verification/malformed-input-taxonomy.toml",
                "verification/test-catalog.toml",
            ),
            output_json="target/gate-artifacts/verification/malformed-input-coverage.json",
            output_markdown="target/gate-artifacts/verification/malformed-input-coverage.md",
        ),
        input_digest="sha256:" + "1" * 64,
        analysis=analysis,
    )


if __name__ == "__main__":
    unittest.main()
