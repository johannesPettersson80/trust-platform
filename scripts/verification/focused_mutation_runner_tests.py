"""Tests for focused source-mutation execution and durable artifacts."""

from __future__ import annotations

import copy
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from . import focused_mutation_artifact
from .focused_mutation_artifact import (
    artifact_contract_digest,
    canonical_json,
    validate_artifact_schema,
    validate_execution_artifact,
    validate_execution_artifact_source,
)
from .focused_mutation_runner import (
    CommandResult,
    artifact_output_path,
    classify_focused_mutant,
    package_from_build_command,
    select_source_shard,
)
from .mutation_execution import select_generated_mutant
from .metadata_validator.constants import ROOT
from .mutation_program_contract import load_mutation_program


class FocusedMutationClassificationTests(unittest.TestCase):
    def test_package_is_derived_from_exact_reviewed_build_command(self) -> None:
        self.assertEqual(
            "trust-runtime",
            package_from_build_command(
                ["cargo", "test", "-p", "trust-runtime", "--test", "stdlib_conv", "--no-run"]
            ),
        )
        with self.assertRaisesRegex(ValueError, "exactly one package"):
            package_from_build_command(["cargo", "test", "--workspace", "--no-run"])
        with self.assertRaisesRegex(ValueError, "exactly one package"):
            package_from_build_command(
                ["cargo", "test", "-p", "trust-runtime", "-p", "trust-hir", "--no-run"]
            )

    def test_only_reviewed_source_shards_are_selectable(self) -> None:
        program = load_mutation_program(ROOT)
        selected = select_source_shard(program, "MUTATION_SHARD_HIR_DIAGNOSTICS_001")
        self.assertEqual("trust-hir", selected["owner"])
        with self.assertRaisesRegex(ValueError, "delivered-build shard"):
            select_source_shard(program, "MUTATION_SHARD_CONNECTOR_STATUS_PROJECTION_001")
        with self.assertRaisesRegex(ValueError, "unknown mutation shard"):
            select_source_shard(program, "MUTATION_SHARD_INVENTED_001")

    def test_output_path_must_equal_the_shard_reserved_artifact(self) -> None:
        program = load_mutation_program(ROOT)
        shard = select_source_shard(
            program, "MUTATION_SHARD_RUNTIME_VALUE_CONVERSION_001"
        )
        expected = ROOT / shard["result_artifact_path"]
        self.assertEqual(expected, artifact_output_path(ROOT, shard, expected))
        with self.assertRaisesRegex(ValueError, "reserved result_artifact_path"):
            artifact_output_path(ROOT, shard, ROOT / "target/unbound.json")
        with self.assertRaisesRegex(ValueError, "reserved result_artifact_path"):
            artifact_output_path(ROOT, shard, Path("../escape.json"))

    def test_source_runner_selector_includes_the_reviewed_generated_name(self) -> None:
        program = load_mutation_program(ROOT)
        mutation = program["shards"][1]["mutations"][0]
        candidate = {
            "name": "different generated mutant",
            "function": {"function_name": mutation["function"]},
            "genre": mutation["genre"],
            "replacement": mutation["replacement"],
        }
        with self.assertRaisesRegex(RuntimeError, "found 0"):
            select_generated_mutant([candidate], mutation)

    def test_caught_requires_the_selected_test_failure_signal(self) -> None:
        build = _result(0, stdout="Finished test profile")
        caught = _result(
            101,
            stdout=(
                "running 1 test\n"
                "test conversion_functions ... FAILED\n\n"
                "failures:\n    conversion_functions\n"
                "test result: FAILED. 0 passed; 1 failed\n"
            ),
        )
        self.assertEqual(
            "caught",
            classify_focused_mutant(
                source_file="crates/trust-runtime/src/stdlib/conversions/dispatch.rs",
                selected_test_name="conversion_functions",
                build=build,
                test=caught,
            ),
        )
        unknown = _result(101, stderr="linker exited with status 1")
        self.assertEqual(
            "error",
            classify_focused_mutant(
                source_file="crates/trust-runtime/src/stdlib/conversions/dispatch.rs",
                selected_test_name="conversion_functions",
                build=build,
                test=unknown,
            ),
        )

    def test_unviable_requires_a_mutated_source_compiler_diagnostic(self) -> None:
        source = "crates/trust-hir/src/type_check/stmt_impl_part_04.rs"
        compile_failure = _result(
            101,
            stderr=f"error[E0308]: mismatched types\n --> {source}:226:9",
        )
        self.assertEqual(
            "unviable",
            classify_focused_mutant(
                source_file=source,
                selected_test_name="test_subrange_assignment_out_of_range",
                build=compile_failure,
                test=None,
            ),
        )
        self.assertEqual(
            "error",
            classify_focused_mutant(
                source_file=source,
                selected_test_name="test_subrange_assignment_out_of_range",
                build=_result(1, stderr="unknown external tool failure"),
                test=None,
            ),
        )

    def test_signals_timeouts_and_disk_failures_are_not_adequacy_results(self) -> None:
        source = "crates/trust-syntax/src/parser/parser.rs"
        for result, expected in (
            (_result(-9), "error"),
            (_result(None, timed_out=True), "timeout"),
            (_result(1, stderr="No space left on device"), "error"),
            (_result(1, stderr="mold: failed to write output"), "error"),
        ):
            with self.subTest(expected=expected, result=result):
                self.assertEqual(
                    expected,
                    classify_focused_mutant(
                        source_file=source,
                        selected_test_name="test_bounded_recovery",
                        build=result,
                        test=None,
                    ),
                )


class FocusedMutationArtifactTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.program = load_mutation_program(ROOT)
        cls.shard = select_source_shard(
            cls.program, "MUTATION_SHARD_RUNTIME_VALUE_CONVERSION_001"
        )
        cls.payload = _artifact(cls.shard)

    def test_contract_digest_ignores_measurement_state_only(self) -> None:
        planned = artifact_contract_digest(self.shard)
        measured_shard = copy.deepcopy(self.shard)
        measured_shard["execution_status"] = "measured"
        measured_shard["result_artifact_path"] = "docs/internal/testing/evidence/result.json"
        self.assertEqual(planned, artifact_contract_digest(measured_shard))
        measured_shard["mutations"][0]["replacement"] = "Ok(Value::Null)"
        self.assertNotEqual(planned, artifact_contract_digest(measured_shard))

    def test_artifact_is_canonical_and_validates_against_live_shard(self) -> None:
        text = canonical_json(self.payload)
        self.assertEqual(json.dumps(json.loads(text), indent=2, sort_keys=True) + "\n", text)
        self.assertEqual([], validate_execution_artifact(ROOT, self.payload, self.shard))

    def test_artifact_tampering_fails_closed(self) -> None:
        mutations: list[dict[str, object]] = []
        candidate = copy.deepcopy(self.payload)
        candidate["summary"]["caught"] = 0
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["mutations"][0]["test_exit_status"] = 0
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["contract_digest"] = "sha256:" + "0" * 64
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["mutations"][0]["association_ids"] = ["INVENTED"]
        mutations.append(candidate)
        for payload in mutations:
            with self.subTest(payload=payload):
                self.assertTrue(validate_execution_artifact(ROOT, payload, self.shard))

    def test_artifact_rejects_test_results_after_failed_build(self) -> None:
        candidate = copy.deepcopy(self.payload)
        source = candidate["mutations"][0]["source_file"]
        candidate["mutations"][0]["build_exit_status"] = 101
        candidate["mutations"][0]["build_stderr"] = (
            f"error[E0308]: mismatched types\n --> {source}:1:1"
        )
        failures = validate_execution_artifact(ROOT, candidate, self.shard)
        self.assertTrue(
            any("test result after an unsuccessful build" in failure for failure in failures)
        )

    def test_artifact_binds_selector_timeout_and_duration_semantics(self) -> None:
        selector = copy.deepcopy(self.payload)
        selector["mutations"][0]["generated_mutant_name"] = "different mutant"
        self.assertTrue(validate_execution_artifact(ROOT, selector, self.shard))

        timeout = copy.deepcopy(self.payload)
        outcome = timeout["mutations"][0]
        outcome["build_timed_out"] = True
        outcome["test_exit_status"] = None
        outcome["test_stdout"] = ""
        outcome["result"] = "timeout"
        timeout["summary"]["caught"] = 0
        timeout["summary"]["timeout"] = 1
        self.assertTrue(validate_execution_artifact(ROOT, timeout, self.shard))

        duration = copy.deepcopy(self.payload)
        duration["mutations"][0]["duration_seconds"] = 9.0
        self.assertTrue(validate_execution_artifact(ROOT, duration, self.shard))

    def test_source_revision_validation_binds_every_execution_input(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for path in ("input.py", "source.rs", "test.rs"):
                (root / path).write_text(path)
            _git(root, "init")
            _git(root, "config", "user.email", "verification@example.invalid")
            _git(root, "config", "user.name", "Verification Test")
            _git(root, "add", ".")
            _git(root, "commit", "-m", "fixture")
            commit = _git(root, "rev-parse", "HEAD")
            payload = {"source_commit": commit}
            shard = {
                "mutations": [{"source_file": "source.rs"}],
                "associated_tests": [{"path": "test.rs"}],
            }
            with patch.object(
                focused_mutation_artifact,
                "EXECUTION_INPUT_PATHS",
                ("input.py",),
            ):
                self.assertEqual(
                    [], validate_execution_artifact_source(root, payload, shard)
                )
                (root / "source.rs").write_text("changed")
                failures = validate_execution_artifact_source(root, payload, shard)
            self.assertTrue(
                any("differs from source_commit: source.rs" in failure for failure in failures)
            )

    def test_schema_semantics_are_drift_pinned(self) -> None:
        schema = json.loads(
            (ROOT / "verification/schemas/focused-mutation-execution.schema.json").read_text()
        )
        self.assertEqual([], validate_artifact_schema(schema))
        schema["properties"]["runner"]["const"] = "unreviewed-runner"
        self.assertTrue(validate_artifact_schema(schema))

    def test_invalid_shapes_never_raise(self) -> None:
        candidates: list[object] = [None, [], {}, {"schema_version": []}]
        for field in self.payload:
            candidate = copy.deepcopy(self.payload)
            candidate.pop(field)
            candidates.append(candidate)
        for payload in candidates:
            try:
                failures = validate_execution_artifact(ROOT, payload, self.shard)
            except Exception as exc:  # pragma: no cover
                self.fail(f"invalid artifact raised {type(exc).__name__}: {exc}")
            self.assertTrue(failures)


def _result(
    returncode: int | None,
    *,
    stdout: str = "",
    stderr: str = "",
    timed_out: bool = False,
) -> CommandResult:
    return CommandResult(
        command=("cargo", "test"),
        returncode=returncode,
        stdout=stdout,
        stderr=stderr,
        timed_out=timed_out,
        duration_seconds=0.1,
    )


def _git(root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args], cwd=root, check=True, capture_output=True, text=True
    )
    return completed.stdout.strip()


def _artifact(shard: dict[str, object]) -> dict[str, object]:
    mutation = shard["mutations"][0]
    assert isinstance(mutation, dict)
    associated = shard["associated_tests"][0]
    assert isinstance(associated, dict)
    result = {
        "id": mutation["id"],
        "source_file": mutation["source_file"],
        "function": mutation["function"],
        "genre": mutation["genre"],
        "replacement": mutation["replacement"],
        "generated_mutant_name": mutation["selector_name"],
        "build_command": mutation["build_command"],
        "build_exit_status": 0,
        "build_stdout": "Finished test profile",
        "build_stderr": "",
        "build_timed_out": False,
        "build_duration_seconds": 0.1,
        "test_command": mutation["test_command"],
        "test_exit_status": 101,
        "test_stdout": (
            f"running 1 test\ntest {associated['name']} ... FAILED\n"
            f"failures:\n    {associated['name']}\n"
            "test result: FAILED. 0 passed; 1 failed\n"
        ),
        "test_stderr": "",
        "test_timed_out": False,
        "test_duration_seconds": 0.1,
        "duration_seconds": 0.2,
        "result": "caught",
        "association_ids": mutation["association_ids"],
    }
    return {
        "schema_version": 1,
        "id": "MUTATION_EXECUTION_RUNTIME_VALUE_CONVERSION_001",
        "status": "complete",
        "runner": "focused-mutation-shard.py v1",
        "tool": "cargo-mutants-single-file-adapter",
        "tool_version": "cargo-mutants 27.0.0",
        "source_commit": "0" * 40,
        "platform": "linux-x86_64",
        "started_at": "2026-07-16T12:00:00Z",
        "finished_at": "2026-07-16T12:01:00Z",
        "shard_id": shard["id"],
        "contract_digest": artifact_contract_digest(shard),
        "baseline_commands": [
            {
                "command": mutation["test_command"],
                "exit_status": 0,
                "timed_out": False,
                "duration_seconds": 0.1,
            }
        ],
        "mutations": [result],
        "summary": {
            "total": 1,
            "caught": 1,
            "survived": 0,
            "unviable": 0,
            "timeout": 0,
            "error": 0,
        },
        "boundaries": {
            "association_ids_are_execution_claims": False,
            "artifact_creates_proof": False,
            "artifact_closes_spec_gaps": False,
            "artifact_promotes_invariants": False,
        },
    }


if __name__ == "__main__":
    unittest.main()
