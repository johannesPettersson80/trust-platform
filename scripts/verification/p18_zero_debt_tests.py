"""Contract tests for the Phase 18 zero-debt machine-readable substrate."""

from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from .p18_zero_debt.artifacts import canonical_json_bytes, render_markdown, write_artifacts
from .p18_zero_debt.contracts import document_closure_digest, validate_bundle
from .p18_zero_debt.cli import main as cli_main, resolve_producer_source_sha
from .p18_zero_debt.orchestrator import PROOF_VALIDATORS, evaluate_bundle
from .p18_zero_debt.assertion_reachability_producer import (
    build_open_records,
    load_plan,
)
from .test_catalog_json_schema import validate_json_schema_instance


SOURCE_SHA = "a" * 40
PROOF_CLASSES = (
    "semantic_chains",
    "test_mapping",
    "assertion_reachability",
    "coverage",
    "mutation",
    "fuzz_property",
    "concurrency_time",
    "unsafe_ffi",
    "hardware_endurance",
    "rendered_ux",
    "platform_claims",
    "release_evidence",
)


def digest_bytes(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def good_assertion_reachability_artifact() -> dict:
    return {
        "schema_version": 1,
        "proof_class": "assertion_reachability",
        "producer": "p18-assertion-reachability.py v1",
        "source_sha": SOURCE_SHA,
        "input_digest": "sha256:" + "f" * 64,
        "input_paths": ["fixture-input"],
        "status": "closed",
        "summary": {
            "denominator": 1,
            "proven": 1,
            "debt": 0,
            "executed_tests": 1,
            "caught_mutants": 1,
            "survived_mutants": 0,
            "unviable_mutants": 0,
            "timeouts": 0,
            "harness_failures": 0,
        },
        "records": [
            {
                "test_id": "TEST_FIXTURE_001",
                "discovery_id": "DISC_" + "A" * 20,
                "status": "proven",
                "code_ids": ["CODE_" + "B" * 64],
                "invariant_ids": ["INVARIANT_FIXTURE_001"],
                "blockers": [],
                "baseline": {
                    "command": ["fixture-test"],
                    "exit_status": 0,
                    "timed_out": False,
                    "stdout": "test fixture ... ok\n",
                    "stderr": "",
                    "result": "passed",
                },
                "mutations": [
                    {
                        "id": "MUTANT_FIXTURE_001",
                        "source_file": "crates/fixture/src/lib.rs",
                        "function": "fixture",
                        "genre": "FnValue",
                        "replacement": "false",
                        "generated_mutant_name": "replace fixture with false",
                        "build_command": ["fixture-build"],
                        "build_exit_status": 0,
                        "build_timed_out": False,
                        "build_stdout": "",
                        "build_stderr": "",
                        "test_command": ["fixture-test"],
                        "test_exit_status": 101,
                        "test_timed_out": False,
                        "test_stdout": "test fixture ... FAILED\n",
                        "test_stderr": "",
                        "selected_test_failed": True,
                        "result": "caught",
                    }
                ],
            }
        ],
    }


def good_bundle(root: Path) -> tuple[dict, dict, dict]:
    proof_rows = []
    budget_rows = []
    for proof_id in PROOF_CLASSES:
        raw_path = Path("raw") / f"{proof_id}.json"
        absolute = root / raw_path
        absolute.parent.mkdir(parents=True, exist_ok=True)
        raw_payload = (
            good_assertion_reachability_artifact()
            if proof_id == "assertion_reachability"
            else {"proof_class": proof_id}
        )
        payload = (json.dumps(raw_payload, sort_keys=True, separators=(",", ":")) + "\n").encode()
        absolute.write_bytes(payload)
        proof_rows.append(
            {
                "id": proof_id,
                "validator_module": (
                    "scripts.verification.p18_zero_debt.existing_test_mapping"
                    if proof_id == "test_mapping"
                    else f"scripts.verification.p18_zero_debt.{proof_id}"
                ),
                "prerequisites": [],
                "status": "closed",
                "denominator": 1,
                "numerator": 1,
                "debt": 0,
                "stale": 0,
                "unowned": 0,
                "skipped": 0,
                "filtered": 0,
                "unrun": 0,
                "timeouts": 0,
                "harness_failures": 0,
                "execution_count": 1,
                "raw_artifact": raw_path.as_posix(),
                "raw_sha256": digest_bytes(payload),
                "producer_command": f"produce-{proof_id}",
                "producer_exit": 0,
                "validator_command": f"validate-{proof_id}",
                "validator_exit": 0,
                "input_digest": (
                    raw_payload.get("input_digest")
                    if proof_id == "assertion_reachability"
                    else "sha256:" + "b" * 64
                ),
            }
        )
        budget_rows.append(
            {
                "id": f"BUDGET_{proof_id.upper()}",
                "proof_class": proof_id,
                "owner": "verification",
                "review_status": "accepted",
                "tool": "fixture-tool",
                "tool_version": "1.0.0",
                "environment": "fixture-environment",
                "topology": "none",
                "seed": "fixture-seed",
                "input_digest": "sha256:" + "c" * 64,
                "shards": 1,
                "baseline_iterations": 10,
                "current_iterations": 10,
                "baseline_duration_seconds": 60,
                "current_duration_seconds": 60,
                "baseline_obligation": 100,
                "current_obligation": 100,
                "last_reviewed": "2026-07-21",
            }
        )

    ledger = {
        "schema_version": 1,
        "id": "P18_ZERO_DEBT_LEDGER_V1",
        "source_sha": SOURCE_SHA,
        "input_closure_sha256": "sha256:" + "d" * 64,
        "state": "closed",
        "last_reviewed": "2026-07-21",
        "baseline": {
            "code_facts": 15229,
            "spec_gap": 15138,
            "test_facts": 4038,
            "reviewed_nonmapping": 3777,
        },
        "current": {
            "spec_gap": 0,
            "registered_backlog": 0,
            "missing_spec": 0,
            "missing_tests": 0,
            "unmapped": 0,
            "stale": 0,
            "reviewed_nonmapping": 0,
            "uncataloged": 0,
            "unreviewed_tests": 0,
            "ignored_skipped_filtered_unrun": 0,
            "zero_test_filters": 0,
            "unexplained_uncovered": 0,
            "viable_mutants": 0,
            "open_findings": 0,
            "unmeasured_proof_classes": 0,
        },
        "proof_classes": proof_rows,
    }
    budgets = {
        "schema_version": 1,
        "id": "P18_PROOF_BUDGETS_V1",
        "source_sha": SOURCE_SHA,
        "input_closure_sha256": "sha256:" + "e" * 64,
        "state": "frozen",
        "last_reviewed": "2026-07-21",
        "budgets": budget_rows,
    }
    claims = {
        "schema_version": 1,
        "id": "P18_CLAIM_MATRIX_V1",
        "source_sha": SOURCE_SHA,
        "input_closure_sha256": "sha256:" + "f" * 64,
        "state": "frozen",
        "last_reviewed": "2026-07-21",
        "claims": [
            {
                "id": "CLAIM_FIXTURE_001",
                "source_ref": "SPEC_VERIFICATION_PROGRAM_CONTRACT_001",
                "owner": "verification",
                "status": "closed",
                "operating_systems": ["linux"],
                "architectures": ["x86_64"],
                "features": ["default"],
                "profiles": ["debug"],
                "packaging_obligations": ["source"],
                "locale_path_encoding": ["utf8"],
                "evidence_requirements": ["fixture"],
                "evidence_refs": ["raw/platform_claims.json"],
                "retirement_decision": "",
            }
        ],
    }
    for document in (ledger, budgets, claims):
        document["input_closure_sha256"] = document_closure_digest(document)
    return ledger, budgets, claims


class P18ZeroDebtSubstrateTests(unittest.TestCase):
    def test_producer_source_identity_requires_valid_override_or_clean_git_head(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_sha, failures = resolve_producer_source_sha(root, None)
            self.assertEqual("unavailable", source_sha)
            self.assertTrue(any("git HEAD" in failure for failure in failures))
            source_sha, failures = resolve_producer_source_sha(root, "not-a-sha")
            self.assertEqual("unavailable", source_sha)
            self.assertTrue(any("40 lower-case hex" in failure for failure in failures))
            source_sha, failures = resolve_producer_source_sha(root, SOURCE_SHA)
            self.assertEqual(SOURCE_SHA, source_sha)
            self.assertEqual([], failures)

    def test_closed_schemas_reject_every_missing_required_and_unknown_field(self) -> None:
        root = Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory() as temporary:
            ledger, budgets, claims = good_bundle(Path(temporary))
            fixtures = (
                (ledger, root / "verification/schemas/p18-zero-debt-ledger.schema.json"),
                (budgets, root / "verification/schemas/p18-proof-budgets.schema.json"),
                (claims, root / "verification/schemas/p18-claim-matrix.schema.json"),
            )

            def resolve(schema_root, node):
                reference = node.get("$ref") if isinstance(node, dict) else None
                if isinstance(reference, str) and reference.startswith("#/$defs/"):
                    return schema_root["$defs"][reference.removeprefix("#/$defs/")]
                return node

            def at_path(document, path):
                value = document
                for part in path:
                    value = value[part]
                return value

            def exercise(document, schema_root, node, path=()):
                node = resolve(schema_root, node)
                value = at_path(document, path)
                if node.get("type") == "object" and isinstance(value, dict):
                    for field in node.get("required", []):
                        candidate = copy.deepcopy(document)
                        at_path(candidate, path).pop(field)
                        failures = validate_json_schema_instance(candidate, schema_root)
                        self.assertTrue(failures, f"missing {path + (field,)}")
                    if node.get("additionalProperties") is False:
                        candidate = copy.deepcopy(document)
                        at_path(candidate, path)["unexpected_p18_field"] = True
                        failures = validate_json_schema_instance(candidate, schema_root)
                        self.assertTrue(failures, f"unknown field at {path}")
                    for field, child in node.get("properties", {}).items():
                        if field in value:
                            exercise(document, schema_root, child, path + (field,))
                elif node.get("type") == "array" and isinstance(value, list):
                    for index in range(len(value)):
                        exercise(document, schema_root, node["items"], path + (index,))

            for document, schema_path in fixtures:
                schema = json.loads(schema_path.read_text())
                self.assertEqual([], validate_json_schema_instance(document, schema))
                exercise(document, schema, schema)

    def test_cli_reports_open_repository_debt_and_strict_mode_blocks(self) -> None:
        root = Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory() as temporary:
            out = Path(temporary)
            common = [
                "--root",
                str(root),
                "--expected-source-sha",
                "a74efe093d4a9fbb60e3d4cf3fd89dbaf8c2ce35",
                "--producer-source-sha",
                SOURCE_SHA,
            ]
            report_exit = cli_main([*common, "--mode", "report", "--out-dir", str(out / "report")])
            strict_exit = cli_main([*common, "--mode", "strict", "--out-dir", str(out / "strict")])
            self.assertEqual(0, report_exit)
            self.assertEqual(1, strict_exit)
            payload = json.loads((out / "strict/p18-zero-debt.json").read_text())
            self.assertEqual("open_debt", payload["status"])
            self.assertEqual(15137, payload["summary"]["spec_gap"])
            self.assertEqual(3777, payload["summary"]["reviewed_nonmapping"])
            index = json.loads((out / "report/artifact-index.json").read_text())
            indexed_paths = {row["path"] for row in index["artifacts"]}
            self.assertIn("verification/p18/burn-down-ledger.json", indexed_paths)
            self.assertEqual(SOURCE_SHA, index["source_sha"])
            self.assertEqual(
                "a74efe093d4a9fbb60e3d4cf3fd89dbaf8c2ce35",
                index["contract_source_sha"],
            )

    def test_artifact_writer_emits_digest_bound_canonical_index(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            input_path = root / "input.toml"
            input_path.write_text("schema_version = 1\n")
            out_dir = root / "out"
            report = {"schema_version": 1, "status": "report_only", "summary": {}, "findings": []}
            index = write_artifacts(
                out_dir,
                report,
                input_paths=[input_path],
                source_sha=SOURCE_SHA,
                exit_status=1,
                display_root=root,
            )
            for name in ("p18-zero-debt.json", "p18-zero-debt.md", "artifact-index.json"):
                self.assertTrue((out_dir / name).is_file(), name)
            encoded_index = canonical_json_bytes(index)
            self.assertEqual(encoded_index, (out_dir / "artifact-index.json").read_bytes())
            indexed_paths = {row["path"] for row in index["artifacts"]}
            self.assertIn("p18-zero-debt.json", indexed_paths)
            self.assertIn("p18-zero-debt.md", indexed_paths)
            self.assertIn("input.toml", indexed_paths)
            self.assertEqual(1, index["exit_status"])
            self.assertEqual("scripts/p18_zero_debt_gate.py", index["producer"])
            self.assertEqual("scripts.verification.p18_zero_debt.contracts", index["validator"])

    def test_known_good_bundle_is_canonical_and_strict_green(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            ledger, budgets, claims = good_bundle(root)
            failures = validate_bundle(
                ledger,
                budgets,
                claims,
                artifact_root=root,
                expected_source_sha=SOURCE_SHA,
            )
            self.assertEqual([], failures)
            result = evaluate_bundle(
                ledger,
                budgets,
                claims,
                artifact_root=root,
                expected_source_sha=SOURCE_SHA,
                mode="strict",
            )
            self.assertEqual(0, result.exit_code)
            encoded = canonical_json_bytes(result.report)
            self.assertTrue(encoded.endswith(b"\n"))
            self.assertEqual(encoded, canonical_json_bytes(json.loads(encoded)))
            markdown = render_markdown(result.report)
            self.assertIn("# Phase 18 Zero-Debt Report", markdown)
            self.assertTrue(markdown.endswith("\n"))

    def test_contract_negative_fixtures_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            original = good_bundle(root)
            fixtures = {}

            def add(name, expected, mutate):
                bundle = copy.deepcopy(original)
                mutate(*bundle)
                if name != "stale_input_closure":
                    for document in bundle:
                        document["input_closure_sha256"] = document_closure_digest(document)
                fixtures[name] = (expected, bundle)

            add(
                "missing_field",
                "ledger missing fields: source_sha",
                lambda ledger, _budgets, _claims: ledger.pop("source_sha"),
            )
            add(
                "unknown_field",
                "ledger unknown fields: extra",
                lambda ledger, _budgets, _claims: ledger.update(extra=True),
            )
            add(
                "stale_source",
                "ledger source_sha is stale",
                lambda ledger, _budgets, _claims: ledger.update(source_sha="b" * 40),
            )
            add(
                "stale_input_closure",
                "ledger input_closure_sha256 is stale",
                lambda ledger, _budgets, _claims: ledger.update(
                    input_closure_sha256="sha256:" + "0" * 64
                ),
            )
            add(
                "stale_artifact_digest",
                "raw artifact digest is stale",
                lambda ledger, _budgets, _claims: ledger["proof_classes"][0].update(
                    raw_sha256="sha256:" + "0" * 64
                ),
            )
            add(
                "weakened_budget",
                "weakens current_iterations",
                lambda _ledger, budgets, _claims: budgets["budgets"][0].update(
                    current_iterations=9
                ),
            )
            add(
                "failed_producer",
                "closed semantic_chains has nonzero producer_exit",
                lambda ledger, _budgets, _claims: ledger["proof_classes"][0].update(
                    producer_exit=1
                ),
            )
            add(
                "failed_validator",
                "closed semantic_chains has nonzero validator_exit",
                lambda ledger, _budgets, _claims: ledger["proof_classes"][0].update(
                    validator_exit=1
                ),
            )
            add(
                "zero_execution",
                "closed semantic_chains has zero execution",
                lambda ledger, _budgets, _claims: ledger["proof_classes"][0].update(
                    execution_count=0
                ),
            )
            add(
                "duplicate_identity",
                "duplicate proof class identity semantic_chains",
                lambda ledger, _budgets, _claims: ledger["proof_classes"].append(
                    copy.deepcopy(ledger["proof_classes"][0])
                ),
            )
            add(
                "broken_prerequisite",
                "has missing prerequisite missing-proof-class",
                lambda ledger, _budgets, _claims: ledger["proof_classes"][0].update(
                    prerequisites=["missing-proof-class"]
                ),
            )
            add(
                "arithmetic_mismatch",
                "numerator plus debt does not equal denominator",
                lambda ledger, _budgets, _claims: ledger["proof_classes"][0].update(
                    denominator=2
                ),
            )
            add(
                "wrong_validator_module",
                "validator_module is stale",
                lambda ledger, _budgets, _claims: ledger["proof_classes"][0].update(
                    validator_module="scripts.verification.p18_zero_debt.mutation"
                ),
            )
            add(
                "unclosed_prerequisite",
                "closed test_mapping has open prerequisite semantic_chains",
                lambda ledger, _budgets, _claims: (
                    ledger["proof_classes"][0].update(status="open"),
                    ledger["proof_classes"][1].update(prerequisites=["semantic_chains"]),
                ),
            )

            for name, (expected, (ledger, budgets, claims)) in fixtures.items():
                with self.subTest(name=name):
                    failures = validate_bundle(
                        ledger,
                        budgets,
                        claims,
                        artifact_root=root,
                        expected_source_sha=SOURCE_SHA,
                    )
                    self.assertTrue(any(expected in failure for failure in failures), failures)

            ledger, budgets, claims = copy.deepcopy(original)
            missing = root / ledger["proof_classes"][0]["raw_artifact"]
            missing.unlink()
            missing_failures = validate_bundle(
                    ledger,
                    budgets,
                    claims,
                    artifact_root=root,
                    expected_source_sha=SOURCE_SHA,
                )
            self.assertTrue(
                any("raw artifact is absent" in failure for failure in missing_failures),
                missing_failures,
            )

    def test_assertion_reachability_artifact_is_semantically_reconstructed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            ledger, budgets, claims = good_bundle(root)
            row = next(
                item
                for item in ledger["proof_classes"]
                if item["id"] == "assertion_reachability"
            )
            artifact_path = root / row["raw_artifact"]
            payload = json.loads(artifact_path.read_text())
            payload["records"][0]["mutations"][0]["result"] = "survived"
            artifact_bytes = (
                json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n"
            ).encode()
            artifact_path.write_bytes(artifact_bytes)
            row["raw_sha256"] = digest_bytes(artifact_bytes)
            ledger["input_closure_sha256"] = document_closure_digest(ledger)
            failures = validate_bundle(
                ledger,
                budgets,
                claims,
                artifact_root=root,
                expected_source_sha=SOURCE_SHA,
            )
            self.assertTrue(
                any("assertion_reachability" in failure for failure in failures),
                failures,
            )

    def test_assertion_reachability_plan_is_closed_and_joins_live_denominator(self) -> None:
        root = Path(__file__).resolve().parents[2]
        plan = load_plan(root)
        records = build_open_records(root)
        self.assertEqual(
            [
                "ASSERTION_REACHABILITY_INTEGER_I64_NORMALIZATION_001",
                "ASSERTION_REACHABILITY_SIGNED_INTEGER_MATERIALIZATION_001",
            ],
            [shard["id"] for shard in plan["shards"]],
        )
        self.assertEqual(267, len(records))
        by_test = {record["test_id"]: record for record in records}
        expected_bindings = {
            "TEST_RUNTIME_INTEGER_I64_NORMALIZATION_001": (
                "CODE_48DB0C5559CB11931C99FDD5BA0812B4045ACFFE01AF8DDB1E3BA4BA16F4087D",
                "RT_INTEGER_I64_NORMALIZATION_001",
            ),
            "TEST_RUNTIME_INTEGER_I64_NORMALIZATION_TRACE_001": (
                "CODE_48DB0C5559CB11931C99FDD5BA0812B4045ACFFE01AF8DDB1E3BA4BA16F4087D",
                "RT_INTEGER_I64_NORMALIZATION_001",
            ),
            "TEST_RUNTIME_SIGNED_INTEGER_MATERIALIZATION_001": (
                "CODE_F72E3E7BC892973CF9344497078A35D93380745CDC5DB4820F5CFB302AAEE14A",
                "RT_SIGNED_INTEGER_MATERIALIZATION_001",
            ),
            "TEST_RUNTIME_SIGNED_INTEGER_MATERIALIZATION_TRACE_001": (
                "CODE_F72E3E7BC892973CF9344497078A35D93380745CDC5DB4820F5CFB302AAEE14A",
                "RT_SIGNED_INTEGER_MATERIALIZATION_001",
            ),
        }
        for test_id, (code_id, invariant_id) in expected_bindings.items():
            self.assertIn(test_id, by_test)
            self.assertEqual([code_id], by_test[test_id]["code_ids"])
            self.assertEqual([invariant_id], by_test[test_id]["invariant_ids"])

    def test_removed_validator_and_open_debt_cannot_pass_strict_mode(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            ledger, budgets, claims = good_bundle(root)
            validators = dict(PROOF_VALIDATORS)
            validators.pop("mutation")
            failures = validate_bundle(
                ledger,
                budgets,
                claims,
                artifact_root=root,
                expected_source_sha=SOURCE_SHA,
                validators=validators,
            )
            self.assertTrue(any("mutation" in failure for failure in failures))

            ledger, budgets, claims = good_bundle(root)
            ledger["state"] = "open"
            ledger["current"]["spec_gap"] = 1
            ledger["input_closure_sha256"] = document_closure_digest(ledger)
            report = evaluate_bundle(
                ledger,
                budgets,
                claims,
                artifact_root=root,
                expected_source_sha=SOURCE_SHA,
                mode="report",
            )
            strict = evaluate_bundle(
                ledger,
                budgets,
                claims,
                artifact_root=root,
                expected_source_sha=SOURCE_SHA,
                mode="strict",
            )
            self.assertEqual(0, report.exit_code)
            self.assertEqual(1, strict.exit_code)
            self.assertEqual(1, strict.report["summary"]["spec_gap"])


if __name__ == "__main__":
    unittest.main()
