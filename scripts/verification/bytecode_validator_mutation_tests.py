"""Tests for the bytecode-validator mutation shard."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification.bytecode_validator_mutation import (
    CommandResult,
    apply_generated_mutant,
    build_report,
    clean_mutation_target,
    classify_mutant,
    render_markdown,
    select_generated_mutant,
)
from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.mutation_shards import (
    MutationContractError,
    load_mutation_contract,
    validate_mutation_report,
    validate_mutation_test_record,
)


TEST_ID = "TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001"


class MutationContractTests(unittest.TestCase):
    def test_committed_contract_accounts_for_every_case_without_execution_claims(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)

        mapped = {
            case_id
            for mutation in contract.mutations
            for case_id in mutation.related_case_ids
        }
        self.assertEqual(mapped | set(contract.out_of_scope_case_ids), set(contract.case_ids))
        self.assertFalse(mapped & set(contract.out_of_scope_case_ids))
        self.assertEqual(contract.case_semantics, "association_only_blocked_cases_not_executed")

    def test_contract_rejects_unknown_case_id(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        record = copy.deepcopy(contract.record)
        record["mutations"][0]["related_case_ids"].append("UNKNOWN_CASE")

        failures = validate_mutation_test_record(record, root=ROOT)

        self.assertTrue(any("unknown case ID UNKNOWN_CASE" in failure for failure in failures), failures)

    def test_contract_rejects_unaccounted_case(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        record = copy.deepcopy(contract.record)
        record["mutation_out_of_scope_case_ids"].pop()

        failures = validate_mutation_test_record(record, root=ROOT)

        self.assertTrue(any("does not account for committed case IDs" in failure for failure in failures), failures)

    def test_contract_rejects_non_string_case_ids_without_crashing(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        record = copy.deepcopy(contract.record)
        record["mutations"][0]["related_case_ids"] = [{"not": "a case ID"}]

        failures = validate_mutation_test_record(record, root=ROOT)

        self.assertTrue(any("related_case_ids" in failure for failure in failures), failures)

    def test_contract_rejects_validator_source_path_traversal(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        record = copy.deepcopy(contract.record)
        record["mutations"][0]["source_file"] = (
            "crates/trust-runtime/src/bytecode/validate/../../../../../Cargo.toml"
        )

        failures = validate_mutation_test_record(record, root=ROOT)

        self.assertTrue(any("validator directory" in failure for failure in failures), failures)

    def test_stack_mutant_uses_the_active_full_validator_regression(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        mutation = next(item for item in contract.mutations if item.function == "validate_stack_shape")

        self.assertIn("bytecode_vm_core", mutation.test_command)
        self.assertIn("vm_rejects_stack_underflow", mutation.test_command)
        self.assertNotIn("phase11_seam_contract", mutation.test_command)


class MutantAdapterTests(unittest.TestCase):
    def test_selector_requires_exactly_one_generated_mutant(self) -> None:
        config = {
            "function": "validate_instruction_stream",
            "genre": "FnValue",
            "replacement": "Ok(())",
        }
        candidate = generated_mutant()

        self.assertEqual(select_generated_mutant([candidate], config), candidate)
        with self.assertRaisesRegex(MutationContractError, "found 0"):
            select_generated_mutant([], config)
        with self.assertRaisesRegex(MutationContractError, "found 2"):
            select_generated_mutant([candidate, candidate], config)

    def test_apply_generated_mutant_uses_one_based_line_and_column_spans(self) -> None:
        source = "fn check() {\n    risky_call()?;\n    Ok(())\n}\n"
        candidate = generated_mutant(
            replacement="Ok(())",
            start_line=2,
            start_column=5,
            end_line=3,
            end_column=11,
        )

        mutated = apply_generated_mutant(source, candidate)

        self.assertEqual(mutated, "fn check() {\n    Ok(())\n}\n")

    def test_classification_separates_survivors_caught_and_unviable(self) -> None:
        passed = CommandResult(("cargo", "test"), 0, "", "", False, 0.1)
        failed = CommandResult(("cargo", "test"), 101, "", "test failed", False, 0.2)
        timed_out = CommandResult(("cargo", "test"), None, "", "", True, 1.0)
        disk_full = CommandResult(
            ("cargo", "test"),
            101,
            "",
            "couldn't create a temp dir: No space left on device",
            False,
            0.2,
        )
        signaled = CommandResult(("cargo", "test"), -9, "", "", False, 0.2)

        self.assertEqual(classify_mutant(passed, passed), "survived")
        self.assertEqual(classify_mutant(passed, failed), "caught")
        self.assertEqual(classify_mutant(failed, None), "unviable")
        self.assertEqual(classify_mutant(timed_out, None), "timeout")
        self.assertEqual(classify_mutant(passed, timed_out), "timeout")
        self.assertEqual(classify_mutant(disk_full, None), "error")
        self.assertEqual(classify_mutant(signaled, None), "error")

    def test_mutation_target_cleanup_removes_only_trust_runtime_outputs(self) -> None:
        result = CommandResult(("cargo", "clean"), 0, "", "", False, 0.1)
        with patch(
            "scripts.verification.bytecode_validator_mutation.run_command",
            return_value=result,
        ) as run:
            clean_mutation_target(Path("/workspace"), {"CARGO_TARGET_DIR": "/target"}, 30.0)

        run.assert_called_once_with(
            ("cargo", "clean", "-p", "trust-runtime"),
            cwd=Path("/workspace"),
            env={"CARGO_TARGET_DIR": "/target"},
            timeout=30.0,
        )


class MutationReportTests(unittest.TestCase):
    def test_survivor_report_keeps_case_ids_and_action(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        outcomes = []
        for index, mutation in enumerate(contract.mutations):
            outcomes.append(
                {
                    "id": mutation.id,
                    "source_file": mutation.source_file,
                    "function": mutation.function,
                    "genre": mutation.genre,
                    "replacement": mutation.replacement,
                    "generated_mutant_name": f"generated {mutation.id}",
                    "result": "survived" if index == 0 else "caught",
                    "related_case_ids": list(mutation.related_case_ids),
                    "survivor_action": mutation.survivor_action,
                    "build_command": list(mutation.build_command),
                    "build_exit_status": 0,
                    "build_timed_out": False,
                    "build_output_tail": "",
                    "test_command": list(mutation.test_command),
                    "test_exit_status": 0 if index == 0 else 101,
                    "test_timed_out": False,
                    "test_output_tail": "",
                    "duration_seconds": 0.5,
                }
            )

        report = build_report(
            contract=contract,
            outcomes=outcomes,
            source_commit="1d9b3ec6a54999a343f2a802307fc417d115c3e6",
            tool_version="cargo-mutants 27.0.0",
            platform="test-linux",
            started_at="2026-07-09T00:00:00Z",
            finished_at="2026-07-09T00:00:01Z",
            baseline_commands=baseline_records(contract),
        )

        self.assertEqual(report["summary"]["survived"], 1)
        self.assertEqual(report["survivors"][0]["related_case_ids"], outcomes[0]["related_case_ids"])
        self.assertTrue(report["survivors"][0]["action"])
        self.assertEqual(validate_mutation_report(report, contract), [])

    def test_report_validation_rejects_survivor_case_mapping_drift(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        outcomes = [
            {
                "id": mutation.id,
                "source_file": mutation.source_file,
                "function": mutation.function,
                "genre": mutation.genre,
                "replacement": mutation.replacement,
                "generated_mutant_name": f"generated {mutation.id}",
                "result": "survived",
                "related_case_ids": list(mutation.related_case_ids),
                "survivor_action": mutation.survivor_action,
                "build_command": list(mutation.build_command),
                "build_exit_status": 0,
                "build_timed_out": False,
                "build_output_tail": "",
                "test_command": list(mutation.test_command),
                "test_exit_status": 0,
                "test_timed_out": False,
                "test_output_tail": "",
                "duration_seconds": 0.1,
            }
            for mutation in contract.mutations
        ]
        report = build_report(
            contract=contract,
            outcomes=outcomes,
            source_commit="1d9b3ec6a54999a343f2a802307fc417d115c3e6",
            tool_version="cargo-mutants 27.0.0",
            platform="test-linux",
            started_at="2026-07-09T00:00:00Z",
            finished_at="2026-07-09T00:00:01Z",
            baseline_commands=baseline_records(contract),
        )
        report["survivors"][0]["related_case_ids"] = ["UNKNOWN_CASE"]

        failures = validate_mutation_report(report, contract)

        self.assertTrue(any("survivor mapping" in failure for failure in failures), failures)

    def test_report_validation_rejects_non_object_baseline_command(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        report = complete_caught_report(contract)
        report["baseline_commands"] = ["cargo test"]

        failures = validate_mutation_report(report, contract)

        self.assertTrue(any("baseline command" in failure for failure in failures), failures)

    def test_report_validation_binds_baseline_and_mutant_commands(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        report = complete_caught_report(contract)
        report["baseline_commands"][0]["command"] = ["cargo", "test", "--unrelated"]
        report["mutations"][0]["test_command"] = ["cargo", "test", "--unrelated"]

        failures = validate_mutation_report(report, contract)

        self.assertTrue(any("baseline command set" in failure for failure in failures), failures)
        self.assertTrue(any("test command drift" in failure for failure in failures), failures)

    def test_report_validation_binds_selector_and_runner(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        report = complete_caught_report(contract)
        report["runner"] = "manual"
        report["mutations"][0]["replacement"] = "Ok(unsafe_default())"

        failures = validate_mutation_report(report, contract)

        self.assertTrue(any("runner/tool binding mismatch" in failure for failure in failures), failures)
        self.assertTrue(any("selector binding mismatch" in failure for failure in failures), failures)

    def test_report_validation_rejects_non_object_root(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)

        failures = validate_mutation_report([], contract)

        self.assertTrue(any("JSON root" in failure for failure in failures), failures)

    def test_report_validation_rejects_unhashable_outcome_fields_without_traceback(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        for field in ("id", "result"):
            for hostile in ({}, []):
                report = complete_caught_report(contract)
                report["mutations"][0][field] = hostile
                try:
                    failures = validate_mutation_report(report, contract)
                except Exception as exc:  # pragma: no cover
                    self.fail(
                        f"hostile outcome {field} raised {type(exc).__name__}: {exc}"
                    )
                self.assertTrue(failures)

    def test_report_validation_rejects_result_exit_status_mismatch(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        report = complete_caught_report(contract)
        report["mutations"][0]["result"] = "survived"
        report["summary"]["caught"] -= 1
        report["summary"]["survived"] += 1
        report["survivors"] = [
            {
                "id": report["mutations"][0]["id"],
                "related_case_ids": report["mutations"][0]["related_case_ids"],
                "action": report["mutations"][0]["survivor_action"],
            }
        ]

        failures = validate_mutation_report(report, contract)

        self.assertTrue(any("exit status" in failure for failure in failures), failures)

    def test_report_validation_rejects_infrastructure_failure_as_unviable(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        report = complete_caught_report(contract)
        outcome = report["mutations"][0]
        outcome["result"] = "unviable"
        outcome["build_exit_status"] = 101
        outcome["build_output_tail"] = "mold: failed to write: No space left on device"
        outcome["test_exit_status"] = None
        report["summary"]["caught"] -= 1
        report["summary"]["unviable"] += 1

        failures = validate_mutation_report(report, contract)

        self.assertTrue(any("imply error, not unviable" in failure for failure in failures), failures)

    def test_complete_report_rejects_infrastructure_error_outcomes(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        report = complete_caught_report(contract)
        outcome = report["mutations"][0]
        outcome["result"] = "error"
        outcome["test_exit_status"] = -9
        report["summary"]["caught"] -= 1
        report["summary"]["error"] += 1

        failures = validate_mutation_report(report, contract)

        self.assertTrue(any("cannot contain infrastructure errors" in failure for failure in failures), failures)

    def test_report_validation_rejects_test_result_after_failed_build(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        report = complete_caught_report(contract)
        outcome = report["mutations"][0]
        outcome["result"] = "unviable"
        outcome["build_exit_status"] = 101
        outcome["test_exit_status"] = 0
        report["summary"]["caught"] -= 1
        report["summary"]["unviable"] += 1

        failures = validate_mutation_report(report, contract)

        self.assertTrue(any("test result after build did not complete" in failure for failure in failures), failures)

    def test_markdown_states_that_case_ids_are_associations_not_execution(self) -> None:
        contract = load_mutation_contract(TEST_ID, root=ROOT)
        outcomes = [
            {
                "id": mutation.id,
                "source_file": mutation.source_file,
                "function": mutation.function,
                "genre": mutation.genre,
                "replacement": mutation.replacement,
                "generated_mutant_name": f"generated {mutation.id}",
                "result": "caught",
                "related_case_ids": list(mutation.related_case_ids),
                "survivor_action": mutation.survivor_action,
                "build_command": list(mutation.build_command),
                "build_exit_status": 0,
                "build_timed_out": False,
                "build_output_tail": "",
                "test_command": list(mutation.test_command),
                "test_exit_status": 101,
                "test_timed_out": False,
                "test_output_tail": "",
                "duration_seconds": 0.1,
            }
            for mutation in contract.mutations
        ]
        report = build_report(
            contract=contract,
            outcomes=outcomes,
            source_commit="1d9b3ec6a54999a343f2a802307fc417d115c3e6",
            tool_version="cargo-mutants 27.0.0",
            platform="test-linux",
            started_at="2026-07-09T00:00:00Z",
            finished_at="2026-07-09T00:00:01Z",
            baseline_commands=baseline_records(contract),
        )

        markdown = render_markdown(report)

        self.assertIn("blocked case IDs were not executed", markdown)
        self.assertIn("Survivors: 0", markdown)


def generated_mutant(
    *,
    replacement: str = "Ok(())",
    start_line: int = 2,
    start_column: int = 5,
    end_line: int = 2,
    end_column: int = 18,
) -> dict[str, object]:
    return {
        "name": "replace validate_instruction_stream with Ok(())",
        "genre": "FnValue",
        "replacement": replacement,
        "function": {"function_name": "validate_instruction_stream"},
        "span": {
            "start": {"line": start_line, "column": start_column},
            "end": {"line": end_line, "column": end_column},
        },
    }


def complete_caught_report(contract):
    outcomes = [
        {
            "id": mutation.id,
            "source_file": mutation.source_file,
            "function": mutation.function,
            "genre": mutation.genre,
            "replacement": mutation.replacement,
            "generated_mutant_name": f"generated {mutation.id}",
            "result": "caught",
            "related_case_ids": list(mutation.related_case_ids),
            "survivor_action": mutation.survivor_action,
            "build_command": list(mutation.build_command),
            "build_exit_status": 0,
            "build_timed_out": False,
            "build_output_tail": "",
            "test_command": list(mutation.test_command),
            "test_exit_status": 101,
            "test_timed_out": False,
            "test_output_tail": "",
            "duration_seconds": 0.1,
        }
        for mutation in contract.mutations
    ]
    return build_report(
        contract=contract,
        outcomes=outcomes,
        source_commit="1d9b3ec6a54999a343f2a802307fc417d115c3e6",
        tool_version="cargo-mutants 27.0.0",
        platform="test-linux",
        started_at="2026-07-09T00:00:00Z",
        finished_at="2026-07-09T00:00:01Z",
        baseline_commands=baseline_records(contract),
    )


def baseline_records(contract):
    records = []
    for mutation in contract.mutations:
        command = list(mutation.test_command)
        if any(record["command"] == command for record in records):
            continue
        records.append(
            {
                "command": command,
                "exit_status": 0,
                "timed_out": False,
                "duration_seconds": 0.1,
            }
        )
    return records


if __name__ == "__main__":
    unittest.main()
