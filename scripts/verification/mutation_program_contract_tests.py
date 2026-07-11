"""Contract tests for the Phase 10 focused mutation program."""

from __future__ import annotations

import copy
import json
import unittest
from types import SimpleNamespace
from unittest import mock

from . import mutation_program_contract as contract_module
from .metadata_validator.constants import ROOT
from .mutation_program_contract import (
    MUTATION_PROGRAM_PATH,
    MUTATION_PROGRAM_SCHEMA_PATH,
    REQUIRED_SHARD_IDS,
    load_mutation_program,
    validate_mutation_program_contract,
)


class MutationProgramContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.program = load_mutation_program(ROOT)

    def test_six_required_shards_are_exact_and_ordered(self) -> None:
        self.assertEqual(
            list(REQUIRED_SHARD_IDS),
            [row["id"] for row in self.program["shards"]],
        )
        self.assertEqual(6, len(self.program["shards"]))
        self.assertEqual(
            ["measured", "planned", "planned", "planned", "planned", "planned"],
            [row["execution_status"] for row in self.program["shards"]],
        )
        self.assertEqual(
            [2, 1, 1, 1, 1, 1],
            [len(row["mutations"]) for row in self.program["shards"]],
        )

    def test_live_contract_is_valid_and_recursively_closed(self) -> None:
        self.assertEqual([], validate_mutation_program_contract(ROOT, self.program))
        schema = json.loads((ROOT / MUTATION_PROGRAM_SCHEMA_PATH).read_text())
        self.assertFalse(schema["additionalProperties"])
        for definition in schema["$defs"].values():
            if isinstance(definition, dict) and definition.get("type") == "object":
                self.assertFalse(definition["additionalProperties"])

    def test_exact_source_mutant_and_test_bindings_are_required(self) -> None:
        for mutation in (
            lambda value: value["shards"][1]["mutations"][0].__setitem__("source_digest", "sha256:" + "0" * 64),
            lambda value: value["shards"][2]["mutations"][0].__setitem__("function", "invented"),
            lambda value: value["shards"][3]["mutations"][0].__setitem__("replacement", "invented"),
            lambda value: value["shards"][4]["associated_tests"][0].__setitem__("name", "renamed"),
            lambda value: value["shards"][5]["associated_tests"][0].__setitem__("ignore_state", "ignored"),
        ):
            corrupted = copy.deepcopy(self.program)
            mutation(corrupted)
            self.assertTrue(validate_mutation_program_contract(ROOT, corrupted))

    def test_planned_shards_cannot_claim_results(self) -> None:
        corrupted = copy.deepcopy(self.program)
        corrupted["shards"][1]["result"] = "caught"
        failures = validate_mutation_program_contract(ROOT, corrupted)
        self.assertTrue(any("result" in item for item in failures), failures)

    def test_each_mutant_partitions_the_reviewed_associations_exactly(self) -> None:
        for shard in self.program["shards"]:
            expected = [row["id"] for row in shard["associated_tests"]]
            actual = [
                association_id
                for mutation in shard["mutations"]
                for association_id in mutation["association_ids"]
            ]
            self.assertEqual(expected, actual)
        corrupted = copy.deepcopy(self.program)
        corrupted["shards"][1]["mutations"][0]["association_ids"] = [
            corrupted["shards"][2]["associated_tests"][0]["id"]
        ]
        failures = validate_mutation_program_contract(ROOT, corrupted)
        self.assertTrue(any("partition associated tests" in item for item in failures), failures)

    def test_survivor_resolution_registry_cannot_invent_or_omit_outcomes(self) -> None:
        self.assertEqual([], self.program["survivor_resolutions"])
        corrupted = copy.deepcopy(self.program)
        corrupted["survivor_resolutions"] = [
            {
                "shard_id": REQUIRED_SHARD_IDS[0],
                "mutation_id": "MUTANT_VALIDATE_STACK_SHAPE_BYPASS",
                "owner": "trust-runtime",
                "action": "add_test",
                "resolution_status": "resolved",
                "rationale": "Invented resolution without a measured survivor.",
                "resolution_ref": "crates/trust-runtime/tests/bytecode_vm_core.rs",
            }
        ]
        failures = validate_mutation_program_contract(ROOT, corrupted)
        self.assertTrue(any("match measured survivors" in item for item in failures), failures)

    def test_focus_policy_rejects_broad_commands_and_large_shards(self) -> None:
        corrupted = copy.deepcopy(self.program)
        corrupted["shards"][1]["mutations"] *= 3
        corrupted["shards"][1]["mutations"][0]["test_command"] = ["just", "test-all"]
        failures = validate_mutation_program_contract(ROOT, corrupted)
        self.assertTrue(any("focused" in item or "maximum" in item for item in failures), failures)

    def test_adequacy_only_and_delivered_build_boundaries_are_pinned(self) -> None:
        corrupted = copy.deepcopy(self.program)
        corrupted["proof_posture"] = "release_proof"
        corrupted["coverage_posture"] = "safety_proof"
        corrupted["shards"][5]["delivered_build_requirement"] = "not_applicable_source_mutation"
        failures = validate_mutation_program_contract(ROOT, corrupted)
        self.assertTrue(any("proof" in item for item in failures), failures)
        self.assertTrue(any("coverage" in item for item in failures), failures)
        self.assertTrue(any("delivered" in item for item in failures), failures)

    def test_schema_weakening_and_manifest_type_corruption_fail_without_traceback(self) -> None:
        schema = json.loads((ROOT / MUTATION_PROGRAM_SCHEMA_PATH).read_text())
        schema["properties"]["shards"]["minItems"] = 0
        with mock.patch("scripts.verification.mutation_program_contract.json.loads", return_value=schema):
            failures = validate_mutation_program_contract(ROOT, self.program)
        self.assertTrue(any("schema" in item for item in failures), failures)
        corrupted = copy.deepcopy(self.program)
        corrupted["shards"] = None
        try:
            failures = validate_mutation_program_contract(ROOT, corrupted)
        except Exception as exc:  # pragma: no cover - assertion documents the boundary
            self.fail(f"hostile manifest raised {type(exc).__name__}: {exc}")
        self.assertTrue(failures)
        corrupted = copy.deepcopy(self.program)
        corrupted["shards"][1]["id"] = {"bad": 1}
        try:
            failures = validate_mutation_program_contract(ROOT, corrupted)
        except Exception as exc:  # pragma: no cover
            self.fail(f"hostile shard ID raised {type(exc).__name__}: {exc}")
        self.assertTrue(failures)

    def test_partial_scanner_duplicate_ids_fail_closed(self) -> None:
        fact = SimpleNamespace(
            to_dict=lambda: {
                "stable_id": "DISC_DUPLICATE",
                "source_kind": "rust_unit_test",
                "path": "crates/trust-syntax/src/parser/parser.rs",
                "name": "duplicate",
                "ignore_state": "not_ignored",
            }
        )
        batch = SimpleNamespace(facts=[fact, fact], diagnostics=[])
        empty = SimpleNamespace(facts=[], diagnostics=[])
        failures: list[str] = []
        with (
            mock.patch.object(contract_module, "scan_rust_file", return_value=batch),
            mock.patch.object(contract_module, "scan_conformance", return_value=empty),
        ):
            contract_module._scan_reviewed_facts(ROOT, failures)
        self.assertTrue(any("duplicate discovery ID" in item for item in failures), failures)

    def test_hostile_legacy_report_shapes_fail_without_traceback(self) -> None:
        for report in ([], {"summary": []}):
            failures: list[str] = []
            with mock.patch.object(contract_module.json, "loads", return_value=report):
                try:
                    contract_module._validate_legacy_shard(
                        ROOT, self.program["shards"], failures
                    )
                except Exception as exc:  # pragma: no cover
                    self.fail(f"hostile legacy report raised {type(exc).__name__}: {exc}")
            self.assertTrue(failures)

    def test_full_metadata_validator_owns_the_manifest_contract(self) -> None:
        from .metadata_validator.core import Validator

        validator = Validator()
        validator.load_records()
        with mock.patch.object(
            validator,
            "mutation_program",
            {**validator.mutation_program, "proof_posture": "release_proof"},
        ):
            validator.validate()
        self.assertTrue(any("proof posture" in item.message for item in validator.failures))

    def test_manifest_paths_are_stable(self) -> None:
        self.assertEqual("verification/mutation-program.toml", MUTATION_PROGRAM_PATH)
        self.assertEqual("verification/schemas/mutation-program.schema.json", MUTATION_PROGRAM_SCHEMA_PATH)


if __name__ == "__main__":
    unittest.main()
