"""Tests for bytecode transform case generation."""

from __future__ import annotations

import tempfile
import unittest
from collections import Counter
from copy import deepcopy
from pathlib import Path

from scripts.verification.bytecode_transforms import (
    BytecodeTransformError,
    generate_bytecode_transform_case_file,
)
from scripts.verification.case_generator import generate_case_file
from scripts.verification.execution_contract import invariant_execution_contract_digest


SEED_BYTES = bytes.fromhex(
    "53 54 42 43"
    "01 00 01 00"
    "00 00 00 00"
    "18 00"
    "01 00"
    "18 00 00 00"
    "00 00 00 00"
    "06 00"
    "00 00"
    "24 00 00 00"
    "08 00 00 00"
    "00 02 00 00 00 00 12 01"
)


class BytecodeTransformTests(unittest.TestCase):
    def test_transform_generator_produces_blocked_case_families(self) -> None:
        with transform_fixture() as fixture:
            record = generate_bytecode_transform_case_file(
                fixture.invariant,
                root=fixture.root,
            )
            expected_source_digest = invariant_execution_contract_digest(fixture.invariant)

        self.assertEqual(record["invariant"], "VM_SEAM_VALID_001")
        self.assertEqual(record["status"], "planned")
        self.assertEqual(record["source_digest"], expected_source_digest)
        self.assertEqual(
            Counter(case["family"] for case in record["case"]),
            Counter(
                {
                    "missing_required": 2,
                    "extra_or_unknown": 2,
                    "wrong_type_or_shape": 3,
                }
            ),
        )
        for case in record["case"]:
            self.assertEqual(case["state"], "blocked")
            self.assertEqual(case["spec_gap_ref"], "SPEC_GAP_BYTECODE_VALIDATOR_001")
            self.assertIn("seed_artifact", case["input"])
            self.assertIn("seed_digest", case["input"])
            self.assertIn("bytes_hex", case["input"])
            self.assertIn("mutated_digest", case["input"])

    def test_specified_invariant_produces_runnable_rejection_expectations(self) -> None:
        with transform_fixture() as fixture:
            fixture.invariant["spec"] = {
                "status": "specified",
                "source_refs": ["SPEC_BYTECODE_FORMAT_001"],
            }
            fixture.invariant["transform_seed"] = {
                "path": "verification/seeds/bytecode_vm/minimal-stbc-seed.toml",
                "oracle_ref": "SPEC_BYTECODE_FORMAT_001#validator-before-apply",
            }
            fixture.invariant["behavior"] = specified_transform_behaviors()
            record = generate_bytecode_transform_case_file(
                fixture.invariant,
                root=fixture.root,
            )

        self.assertEqual(record["status"], "active")
        by_transform = {
            case["input"]["transform"]: case
            for case in record["case"]
        }
        self.assertEqual(set(by_transform), {
            "container_truncate",
            "unknown_opcode",
            "jump_target",
            "stack_underflow",
        })
        for case in record["case"]:
            self.assertNotIn("state", case)
            self.assertNotIn("spec_gap_ref", case)
            self.assertEqual(case["expect"]["outcome"], "reject")
            self.assertTrue(case["expect"]["no_partial_apply"])
            self.assertEqual(case["expect"]["delta"]["target"], "unchanged")
            self.assertNotIn("error_variant", case["expect"])
            self.assertNotIn("message_contains", case["expect"])
            self.assertEqual(
                case["expect"]["oracle_ref"],
                "SPEC_BYTECODE_FORMAT_001#validator-before-apply",
            )

    def test_specified_transform_requires_matching_behavior_partition(self) -> None:
        with transform_fixture() as fixture:
            fixture.invariant["spec"]["status"] = "specified"
            fixture.invariant["transform_seed"] = {
                "path": "verification/seeds/bytecode_vm/minimal-stbc-seed.toml",
                "oracle_ref": "SPEC_BYTECODE_FORMAT_001#validator-before-apply",
            }
            fixture.invariant["behavior"] = specified_transform_behaviors()[:-1]
            with self.assertRaisesRegex(
                BytecodeTransformError,
                "has no matching oracle-backed behavior",
            ):
                generate_bytecode_transform_case_file(
                    fixture.invariant,
                    root=fixture.root,
                )

    def test_specified_invariant_rejects_stale_gap_seed_binding(self) -> None:
        with transform_fixture() as fixture:
            fixture.invariant["spec"]["status"] = "specified"
            with self.assertRaisesRegex(
                BytecodeTransformError,
                "specified transform_seed requires a non-empty oracle_ref",
            ):
                generate_bytecode_transform_case_file(
                    fixture.invariant,
                    root=fixture.root,
                )

    def test_specified_transform_oracle_must_name_a_listed_spec_source(self) -> None:
        with transform_fixture() as fixture:
            fixture.invariant["spec"]["status"] = "specified"
            fixture.invariant["transform_seed"] = {
                "path": "verification/seeds/bytecode_vm/minimal-stbc-seed.toml",
                "oracle_ref": "SPEC_OTHER_001#validator",
            }
            with self.assertRaisesRegex(
                BytecodeTransformError,
                "oracle_ref must name a listed spec source",
            ):
                generate_bytecode_transform_case_file(
                    fixture.invariant,
                    root=fixture.root,
                )

    def test_unknown_opcode_mutates_only_the_configured_offset(self) -> None:
        with transform_fixture() as fixture:
            record = generate_bytecode_transform_case_file(
                fixture.invariant,
                root=fixture.root,
            )

        case = next(
            case
            for case in record["case"]
            if case["input"]["transform"] == "unknown_opcode"
            and case["input"]["opcode"] == 255
        )
        mutated = bytes.fromhex(case["input"]["bytes_hex"])
        self.assertEqual(mutated[36], 0xFF)
        self.assertEqual(mutated[:36], SEED_BYTES[:36])
        self.assertEqual(mutated[37:], SEED_BYTES[37:])

    def test_truncation_uses_declared_section_boundaries(self) -> None:
        with transform_fixture() as fixture:
            record = generate_bytecode_transform_case_file(
                fixture.invariant,
                root=fixture.root,
            )

        truncate_lengths = sorted(
            len(bytes.fromhex(case["input"]["bytes_hex"]))
            for case in record["case"]
            if case["input"]["transform"] == "container_truncate"
        )
        self.assertEqual(truncate_lengths, [24, 36])

    def test_transform_generation_is_deterministic(self) -> None:
        with transform_fixture() as fixture:
            first = generate_bytecode_transform_case_file(fixture.invariant, root=fixture.root)
            second = generate_bytecode_transform_case_file(fixture.invariant, root=fixture.root)

        self.assertEqual(first, second)

    def test_transform_source_digest_ignores_proof_lifecycle_only(self) -> None:
        with transform_fixture() as fixture:
            baseline = generate_bytecode_transform_case_file(
                fixture.invariant, root=fixture.root
            )
            promoted = deepcopy(fixture.invariant)
            promoted.update(
                {
                    "status": "implemented",
                    "proof_level": "G1",
                    "tests": ["TEST_BYTECODE_VALIDATOR_CASES_001"],
                    "evidence_refs": ["EVID_LOCK_COMPARE"],
                    "missing": ["broad_remote_gate"],
                }
            )
            fixture.invariant_path.write_text("proof_level = \"G1\"\n")
            promoted_record = generate_bytecode_transform_case_file(
                promoted, root=fixture.root
            )

        self.assertEqual(
            promoted_record["source_digest"], baseline["source_digest"]
        )

    def test_seed_path_must_stay_inside_workspace(self) -> None:
        with transform_fixture() as fixture:
            fixture.invariant["transform_seed"]["path"] = "../outside.toml"
            with self.assertRaisesRegex(BytecodeTransformError, "relative workspace path"):
                generate_bytecode_transform_case_file(fixture.invariant, root=fixture.root)

    def test_case_generator_routes_transform_seed_invariant(self) -> None:
        with transform_fixture() as fixture:
            validator = FakeValidator(fixture.invariant)
            record = generate_case_file("VM_SEAM_VALID_001", validator)

        self.assertEqual(record["invariant"], "VM_SEAM_VALID_001")
        self.assertTrue(all(case["state"] == "blocked" for case in record["case"]))


class FakeValidator:
    def __init__(self, invariant: dict[str, object]) -> None:
        self.invariants = {invariant["id"]: invariant}


class TransformFixture:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.seed_path = root / "verification/seeds/bytecode_vm/minimal-stbc-seed.toml"
        self.invariant_path = root / "verification/invariants/bytecode_vm/VM_SEAM_VALID_001.toml"
        self.seed_path.parent.mkdir(parents=True)
        self.invariant_path.parent.mkdir(parents=True)
        self.seed_path.write_text(seed_toml())
        self.invariant_path.write_text("id = \"VM_SEAM_VALID_001\"\n")
        self.invariant: dict[str, object] = {
            "id": "VM_SEAM_VALID_001",
            "title": "Bytecode validator rejects VM semantic contract violations",
            "area": "bytecode_vm",
            "owner": "trust-runtime",
            "last_reviewed": "2026-07-09",
            "contract_kind": "decision_table",
            "input": {"name": "bytecode_container"},
            "spec": {
                "status": "missing",
                "source_refs": ["SPEC_BYTECODE_FORMAT_001"],
            },
            "oracle": {
                "kind": "trust_contract",
                "ref": "SPEC_GAP_BYTECODE_VALIDATOR_001",
            },
            "spec_gap_refs": ["SPEC_GAP_BYTECODE_VALIDATOR_001"],
            "transform_seed": {
                "path": "verification/seeds/bytecode_vm/minimal-stbc-seed.toml",
                "spec_gap_ref": "SPEC_GAP_BYTECODE_VALIDATOR_001",
            },
            "_path": self.invariant_path,
            "_root": root,
        }


class transform_fixture:
    def __enter__(self) -> TransformFixture:
        self.temp = tempfile.TemporaryDirectory()
        self.fixture = TransformFixture(Path(self.temp.name))
        return self.fixture

    def __exit__(self, exc_type: object, exc: object, tb: object) -> None:
        self.temp.cleanup()


def specified_transform_behaviors() -> list[dict[str, object]]:
    partitions = [
        "TRUNCATED_REQUIRED_CONTAINER_DATA",
        "UNIMPLEMENTED_RESERVED_LEGACY_OR_UNKNOWN_OPCODE",
        "JUMP_OUTSIDE_POU_OR_INSTRUCTION_BOUNDARY",
        "INVALID_OPERAND_STACK_DATAFLOW",
    ]
    return [
        {
            "partition": {"equals": partition},
            "outcome": "reject",
            "no_partial_apply": True,
            "oracle_ref": "SPEC_BYTECODE_FORMAT_001#validator-before-apply",
            "delta": {
                "target": "unchanged",
                "siblings": "unchanged",
                "retain": "unchanged",
                "process_image": "unchanged",
                "diagnostics": "expected_diagnostic",
                "status": "unchanged",
            },
        }
        for partition in partitions
    ]


def seed_toml() -> str:
    return f"""
schema_version = 1
id = "minimal-stbc-seed"
title = "Minimal STBC-like bytecode seed"
bytes_hex = "{SEED_BYTES.hex()}"

[[truncate_points]]
id = "before_section_table"
offset = 24
family = "missing_required"

[[truncate_points]]
id = "before_pou_bodies"
offset = 36
family = "missing_required"

[[opcode_sites]]
id = "pou_body_first_opcode"
offset = 36
opcodes = [255, 128]
family = "extra_or_unknown"

[[jump_sites]]
id = "pou_body_jmp_operand"
operand_offset = 38
deltas = [100, -100]
family = "wrong_type_or_shape"

[[stack_underflow_sites]]
id = "pou_body_pop_empty_stack"
offset = 36
length = 1
bytes_hex = "12"
family = "wrong_type_or_shape"
"""


if __name__ == "__main__":
    unittest.main()
