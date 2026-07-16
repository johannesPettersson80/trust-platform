"""Adversarial tests for hand-authored state-machine case provenance."""

from __future__ import annotations

import unittest
from datetime import date
from copy import deepcopy
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from scripts.verification.case_digests import file_digest
from scripts.verification.execution_contract import invariant_execution_contract_digest
from scripts.verification.metadata_validator.case_files import validate_case_file
from scripts.verification.metadata_validator.case_trace_contract import (
    GENERATED_DECISION_TABLE_V1,
    HAND_AUTHORED_STATE_MACHINE_V1,
    trace_definition_digest,
    validate_case_provenance,
)


PATH = Path("verification/test-catalog.toml")
GENERATOR_DIGEST = "sha256:" + "1" * 64
SOURCE_DIGEST = "sha256:" + "2" * 64


class CaseTraceContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.failures: list[str] = []
        self.spec_sources = {
            "SPEC_TIMER": {
                "id": "SPEC_TIMER",
                "source_status": "active",
                "oracle_eligible": True,
                "authority": "normative_product",
            }
        }
        self.invariant = {
            "id": "INV_TIMER",
            "contract_kind": "state_machine",
            "status": "test_written",
            "spec_gap_refs": [],
            "spec": {"status": "specified"},
        }

    def validate(self, case_data: dict, invariant: dict | None = None) -> str:
        return validate_case_provenance(
            fail=lambda _path, message: self.failures.append(message),
            path=PATH,
            test_id="TEST_TIMER_TRACE",
            case_data=case_data,
            invariant=invariant or self.invariant,
            spec_sources=self.spec_sources,
            expected_generator_digest=GENERATOR_DIGEST,
            expected_generator_v2_digest=GENERATOR_DIGEST,
            expected_source_digest=SOURCE_DIGEST,
        )

    def hand_case(self) -> dict:
        cases = [
            {
                "id": "TRACE_ONE",
                "family": "happy_path",
                "input": {"scenario": "TRACE_ONE"},
                "expect": {
                    "outcome": "accept_value",
                    "oracle_ref": "SPEC_TIMER#state-machine",
                },
                "trace": [
                    {
                        "sequence": 0,
                        "stimulus": {"input": False, "elapsed_ns": 0},
                        "expected": {"output": False, "elapsed_ns": 0},
                        "oracle_ref": "SPEC_TIMER#state-machine",
                    },
                    {
                        "sequence": 1,
                        "stimulus": {"input": True, "elapsed_ns": 10},
                        "expected": {"output": True, "elapsed_ns": 0},
                        "oracle_ref": "SPEC_TIMER#state-machine",
                    },
                ],
            }
        ]
        return {
            "schema_version": 1,
            "case_provenance_kind": HAND_AUTHORED_STATE_MACHINE_V1,
            "source_digest": SOURCE_DIGEST,
            "trace_definition_digest": trace_definition_digest(cases),
            "case": cases,
        }

    def test_legacy_generated_case_keeps_exact_generator_contract(self) -> None:
        case_data = {
            "schema_version": 1,
            "generator": "gen_cases.py v1",
            "generator_digest": GENERATOR_DIGEST,
            "source_digest": SOURCE_DIGEST,
            "case": [{"id": "GENERATED"}],
        }

        kind = self.validate(case_data)

        self.assertEqual(kind, GENERATED_DECISION_TABLE_V1)
        self.assertEqual(self.failures, [])

    def test_versioned_non_bytecode_generator_is_accepted(self) -> None:
        case_data = {
            "schema_version": 1,
            "generator": "gen_cases_v2.py v1",
            "generator_digest": GENERATOR_DIGEST,
            "source_digest": SOURCE_DIGEST,
            "case": [{"id": "GENERATED"}],
        }

        kind = self.validate(case_data)

        self.assertEqual(kind, GENERATED_DECISION_TABLE_V1)
        self.assertEqual(self.failures, [])

    def test_generated_case_rejects_hand_authored_fields_and_stale_digest(self) -> None:
        case_data = {
            "schema_version": 1,
            "case_provenance_kind": GENERATED_DECISION_TABLE_V1,
            "generator": "gen_cases.py v1",
            "generator_digest": "sha256:" + "0" * 64,
            "source_digest": SOURCE_DIGEST,
            "trace_definition_digest": "sha256:" + "3" * 64,
            "case": [{"id": "GENERATED", "trace": []}],
        }

        self.validate(case_data)

        joined = "\n".join(self.failures)
        self.assertIn("generator_digest mismatch", joined)
        self.assertIn("forbids trace_definition_digest", joined)
        self.assertIn("forbids case trace records", joined)

    def test_hand_authored_trace_accepts_closed_ordered_shape(self) -> None:
        kind = self.validate(self.hand_case())

        self.assertEqual(kind, HAND_AUTHORED_STATE_MACHINE_V1)
        self.assertEqual(self.failures, [])

    def test_unicode_trace_digest_matches_verification_cases_contract(self) -> None:
        cases = [
            {
                "id": "TRACE_UNICODE",
                "trace": [
                    {
                        "sequence": 0,
                        "stimulus": {"label": "räknare"},
                        "expected": {"status": "klar"},
                        "oracle_ref": "SPEC_TIMER#state-machine",
                    }
                ],
            }
        ]

        self.assertEqual(
            trace_definition_digest(cases),
            "sha256:e9fc05d0b2987cdaaf7e429b785bcd9e6f35aef6895e8f75c7f2a25666a414cf",
        )

    def test_hand_authored_trace_cannot_claim_generator_provenance(self) -> None:
        case_data = self.hand_case()
        case_data["generator"] = "gen_cases.py v1"
        case_data["generator_digest"] = GENERATOR_DIGEST

        self.validate(case_data)

        self.assertIn("forbids generator and generator_digest", "\n".join(self.failures))

    def test_hand_authored_trace_requires_state_machine_but_allows_other_open_gaps(self) -> None:
        invariant = deepcopy(self.invariant)
        invariant["contract_kind"] = "decision_table"
        invariant["status"] = "spec_gap"
        invariant["spec_gap_refs"] = ["SPEC_GAP_TIMER"]
        invariant["spec"] = {"status": "ambiguous"}

        self.validate(self.hand_case(), invariant)

        joined = "\n".join(self.failures)
        self.assertIn(
            "requires invariant contract_kind = state_machine or protocol_trace",
            joined,
        )
        self.assertNotIn("spec.status", joined)
        self.assertNotIn("spec_gap_refs", joined)

    def test_hand_authored_trace_accepts_protocol_trace_invariant(self) -> None:
        invariant = deepcopy(self.invariant)
        invariant["contract_kind"] = "protocol_trace"

        kind = self.validate(self.hand_case(), invariant)

        self.assertEqual(kind, HAND_AUTHORED_STATE_MACHINE_V1)
        self.assertEqual(self.failures, [])

    def test_hand_authored_trace_digest_is_recomputed(self) -> None:
        case_data = self.hand_case()
        case_data["case"][0]["trace"][1]["expected"]["elapsed_ns"] = 9

        self.validate(case_data)

        self.assertIn("trace_definition_digest mismatch", "\n".join(self.failures))

    def test_trace_steps_are_contiguous_closed_and_oracle_backed(self) -> None:
        case_data = self.hand_case()
        steps = case_data["case"][0]["trace"]
        steps[0]["extra"] = "unreviewed"
        steps[0]["stimulus"] = {}
        steps[1]["sequence"] = 3
        steps[1]["oracle_ref"] = "SPEC_UNKNOWN#state-machine"
        case_data["trace_definition_digest"] = trace_definition_digest(case_data["case"])

        self.validate(case_data)

        joined = "\n".join(self.failures)
        self.assertIn("trace step fields must be exactly", joined)
        self.assertIn("trace sequence must be contiguous from zero", joined)
        self.assertIn("trace stimulus must be a non-empty table", joined)
        self.assertIn("references unknown spec source", joined)

    def test_trace_values_reject_non_finite_and_non_json_toml_values_without_raising(self) -> None:
        case_data = self.hand_case()
        step = case_data["case"][0]["trace"][0]
        step["stimulus"]["bad_float"] = float("inf")
        step["expected"]["bad_date"] = date(2026, 7, 12)
        case_data["trace_definition_digest"] = "sha256:" + "0" * 64

        self.validate(case_data)

        joined = "\n".join(self.failures)
        self.assertIn("must not contain TOML floats", joined)
        self.assertIn("canonical JSON", joined)

    def test_trace_values_reject_finite_toml_float_before_digesting(self) -> None:
        case_data = self.hand_case()
        case_data["case"][0]["trace"][0]["stimulus"]["nested"] = [
            {"epsilon": 1e-7}
        ]
        case_data["trace_definition_digest"] = trace_definition_digest(case_data["case"])

        self.validate(case_data)

        self.assertIn("must not contain TOML floats", "\n".join(self.failures))

    def test_blocked_hand_authored_case_forbids_asserted_trace(self) -> None:
        case_data = self.hand_case()
        case = case_data["case"][0]
        case.pop("expect")
        case["state"] = "blocked"
        case["spec_gap_ref"] = "SPEC_GAP_TIMER"
        case["trace"][0].pop("expected")
        case_data["trace_definition_digest"] = trace_definition_digest(case_data["case"])

        self.validate(case_data)

        joined = "\n".join(self.failures)
        self.assertIn("forbids a trace with asserted expected states", joined)

    def test_blocked_hand_authored_case_without_trace_is_allowed_by_provenance(self) -> None:
        case_data = self.hand_case()
        case = case_data["case"][0]
        case.pop("expect")
        case.pop("trace")
        case["state"] = "blocked"
        case["spec_gap_ref"] = "SPEC_GAP_TIMER"
        case_data["trace_definition_digest"] = trace_definition_digest(case_data["case"])

        self.validate(case_data)

        self.assertEqual(self.failures, [])

    def test_unknown_provenance_kind_is_rejected(self) -> None:
        case_data = self.hand_case()
        case_data["case_provenance_kind"] = "invented_trace_v9"

        kind = self.validate(case_data)

        self.assertEqual(kind, "invented_trace_v9")
        self.assertIn("unknown case_provenance_kind", "\n".join(self.failures))

    def test_full_case_file_validator_routes_hand_authored_contract(self) -> None:
        with TemporaryDirectory() as raw_dir:
            root = Path(raw_dir)
            invariant_path = root / "verification/invariants/compiler_iec/INV_TIMER.toml"
            invariant_path.parent.mkdir(parents=True)
            invariant_path.write_text("id = \"INV_TIMER\"\n")
            invariant = deepcopy(self.invariant)
            invariant.update(
                {
                    "_path": invariant_path,
                    "area": "compiler_iec",
                    "input": {},
                    "coverage": {
                        "cells": [
                            {
                                "dimension": "happy_path",
                                "state": "gap_open",
                                "rationale": "The trace is not yet proven.",
                            }
                        ]
                    },
                    "behavior": [
                        {
                            "outcome": "accept_value",
                            "oracle_ref": "SPEC_TIMER#state-machine",
                        }
                    ],
                }
            )
            cases = self.hand_case()["case"]
            trace_digest = trace_definition_digest(cases)
            case_path = root / "verification/cases/compiler_iec/INV_TIMER.toml"
            case_path.parent.mkdir(parents=True)
            source_digest = invariant_execution_contract_digest(invariant)
            case_path.write_text(
                f'''schema_version = 1
id = "CASES_INV_TIMER"
title = "Timer trace"
area = "compiler_iec"
owner = "runtime"
status = "planned"
invariant = "INV_TIMER"
case_provenance_kind = "hand_authored_state_machine_v1"
trace_definition_digest = "{trace_digest}"
source_digest = "{source_digest}"
last_reviewed = "2026-07-12"

[[case]]
id = "TRACE_ONE"
family = "happy_path"
input = {{ scenario = "TRACE_ONE" }}
expect = {{ outcome = "accept_value", oracle_ref = "SPEC_TIMER#state-machine" }}
trace = [
  {{ sequence = 0, stimulus = {{ input = false, elapsed_ns = 0 }}, expected = {{ output = false, elapsed_ns = 0 }}, oracle_ref = "SPEC_TIMER#state-machine" }},
  {{ sequence = 1, stimulus = {{ input = true, elapsed_ns = 10 }}, expected = {{ output = true, elapsed_ns = 0 }}, oracle_ref = "SPEC_TIMER#state-machine" }},
]
'''
            )
            failures: list[str] = []
            with patch(
                "scripts.verification.metadata_validator.case_files.ROOT",
                root,
            ):
                validate_case_file(
                    fail=lambda _path, message: failures.append(message),
                    path=root / "verification/test-catalog.toml",
                    test_record={
                        "id": "TEST_TIMER_TRACE",
                        "case_file": "verification/cases/compiler_iec/INV_TIMER.toml",
                        "invariants": ["INV_TIMER"],
                    },
                    invariants={"INV_TIMER": invariant},
                    spec_sources=self.spec_sources,
                    spec_gaps={},
                )

                lifecycle_invariant = deepcopy(invariant)
                lifecycle_invariant.update(
                    {
                        "status": "validated",
                        "proof_level": "G2",
                        "tests": ["TEST_TIMER_TRACE"],
                        "gates": ["pr", "nightly"],
                        "evidence_refs": ["EVID_TIMER_RED", "EVID_TIMER_GREEN"],
                        "spec_gap_refs": [],
                        "missing": [],
                        "coverage": {"cells": [{"dimension": "happy_path", "state": "covered"}]},
                        "last_reviewed": "2026-07-13",
                    }
                )
                lifecycle_failures: list[str] = []
                validate_case_file(
                    fail=lambda _path, message: lifecycle_failures.append(message),
                    path=root / "verification/test-catalog.toml",
                    test_record={
                        "id": "TEST_TIMER_TRACE",
                        "case_file": "verification/cases/compiler_iec/INV_TIMER.toml",
                        "invariants": ["INV_TIMER"],
                    },
                    invariants={"INV_TIMER": lifecycle_invariant},
                    spec_sources=self.spec_sources,
                    spec_gaps={},
                )

                behavior_invariant = deepcopy(lifecycle_invariant)
                behavior_invariant["behavior"][0]["outcome"] = "reject"
                behavior_failures: list[str] = []
                validate_case_file(
                    fail=lambda _path, message: behavior_failures.append(message),
                    path=root / "verification/test-catalog.toml",
                    test_record={
                        "id": "TEST_TIMER_TRACE",
                        "case_file": "verification/cases/compiler_iec/INV_TIMER.toml",
                        "invariants": ["INV_TIMER"],
                    },
                    invariants={"INV_TIMER": behavior_invariant},
                    spec_sources=self.spec_sources,
                    spec_gaps={},
                )

        self.assertEqual(failures, [])
        self.assertEqual(lifecycle_failures, [])
        self.assertIn("source_digest mismatch", "\n".join(behavior_failures))

    def test_full_case_file_validator_rejects_unknown_root_and_case_fields(self) -> None:
        with TemporaryDirectory() as raw_dir:
            root = Path(raw_dir)
            invariant_path = root / "verification/invariants/compiler_iec/INV_TIMER.toml"
            invariant_path.parent.mkdir(parents=True)
            invariant_path.write_text("id = \"INV_TIMER\"\n")
            invariant = deepcopy(self.invariant)
            invariant.update(
                {
                    "_path": invariant_path,
                    "area": "compiler_iec",
                    "input": {},
                    "behavior": [],
                }
            )
            case_path = root / "verification/cases/compiler_iec/INV_TIMER.toml"
            case_path.parent.mkdir(parents=True)
            case_path.write_text(
                f'''schema_version = 1
id = "CASES_INV_TIMER"
title = "Timer trace"
area = "compiler_iec"
owner = "runtime"
status = "planned"
invariant = "INV_TIMER"
generator = "gen_cases.py v1"
generator_digest = "{GENERATOR_DIGEST}"
source_digest = "{file_digest(invariant_path)}"
last_reviewed = "2026-07-12"
unreviewed_root = true

[[case]]
id = "TRACE_ONE"
family = "happy_path"
input = {{ scenario = "TRACE_ONE" }}
state = "blocked"
spec_gap_ref = "SPEC_GAP_TIMER"
unreviewed_case = true
'''
            )
            failures: list[str] = []
            with patch(
                "scripts.verification.metadata_validator.case_files.ROOT",
                root,
            ), patch(
                "scripts.verification.metadata_validator.case_files.current_generator_digest",
                return_value=GENERATOR_DIGEST,
            ):
                validate_case_file(
                    fail=lambda _path, message: failures.append(message),
                    path=root / "verification/test-catalog.toml",
                    test_record={
                        "id": "TEST_TIMER_TRACE",
                        "case_file": "verification/cases/compiler_iec/INV_TIMER.toml",
                        "invariants": ["INV_TIMER"],
                    },
                    invariants={"INV_TIMER": invariant},
                    spec_sources=self.spec_sources,
                    spec_gaps={"SPEC_GAP_TIMER": {"resolution_status": "open"}},
                )

        joined = "\n".join(failures)
        self.assertIn("case_file root fields must be exactly", joined)
        self.assertIn("case fields must be exactly", joined)


if __name__ == "__main__":
    unittest.main()
