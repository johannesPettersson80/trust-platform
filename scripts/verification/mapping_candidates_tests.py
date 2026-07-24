"""Focused tests for execution-guided mapping candidate ranking."""

from __future__ import annotations

import unittest

from scripts.verification.mapping_candidates import analyze_mapping_candidates


def _trace(discovery_id: str, *code_ids: str) -> dict[str, object]:
    return {
        "discovery_id": discovery_id,
        "trace_status": "observed",
        "covered_code_ids": list(code_ids),
    }


def _specified(*invariant_ids: str) -> dict[str, object]:
    return {
        "disposition": "specified",
        "invariant_ids": list(invariant_ids),
    }


def _live(*code_ids: str) -> dict[str, dict[str, str]]:
    return {
        code_id: {"code_id": code_id, "area": "runtime_safety"}
        for code_id in code_ids
    }


class MappingCandidateTests(unittest.TestCase):
    def test_failed_execution_cannot_generate_candidate_or_demand(self) -> None:
        result = analyze_mapping_candidates(
            trace_records=[
                {
                    "discovery_id": "DISC_00000000000000000001",
                    "trace_status": "failed",
                    "covered_code_ids": [],
                }
            ],
            code_mappings={"CODE_A": _specified("INV_ONE")},
            invariants={"INV_ONE": {"risk": "maintenance"}},
        )

        self.assertEqual([], result["candidates"])
        self.assertEqual([], result["demands"])
        self.assertEqual([], result["unmapped_code_demand_ranking"])

    def test_unique_execution_join_ranks_without_lexical_inference(self) -> None:
        result = analyze_mapping_candidates(
            trace_records=[
                {
                    **_trace("DISC_00000000000000000001", "CODE_A", "CODE_B"),
                    "reference_candidates": ["UNRELATED_NAME_MATCH"],
                }
            ],
            code_mappings={
                "CODE_A": _specified("INV_EXECUTED"),
                "CODE_B": _specified("INV_EXECUTED"),
            },
            invariants={
                "INV_EXECUTED": {"risk": "maintenance"},
                "UNRELATED_NAME_MATCH": {"risk": "maintenance"},
            },
        )

        self.assertEqual(1, len(result["candidates"]))
        candidate = result["candidates"][0]
        self.assertEqual("C1", candidate["candidate_tier"])
        self.assertEqual(["INV_EXECUTED"], candidate["candidate_invariant_ids"])
        self.assertNotIn("UNRELATED_NAME_MATCH", candidate["candidate_invariant_ids"])

    def test_single_covered_fact_is_coverage_only_c2(self) -> None:
        result = analyze_mapping_candidates(
            trace_records=[
                _trace("DISC_00000000000000000001", "CODE_A"),
            ],
            code_mappings={"CODE_A": _specified("INV_ONE")},
            invariants={"INV_ONE": {"risk": "maintenance"}},
        )

        self.assertEqual("C2", result["candidates"][0]["candidate_tier"])

    def test_live_fact_missing_from_code_map_is_ranked_without_becoming_a_mapping(self) -> None:
        result = analyze_mapping_candidates(
            trace_records=[
                _trace(
                    "DISC_00000000000000000001",
                    "CODE_SPECIFIED",
                    "CODE_LIVE_UNMAPPED",
                ),
            ],
            code_mappings={"CODE_SPECIFIED": _specified("INV_ONE")},
            invariants={"INV_ONE": {"risk": "maintenance"}},
            live_code_facts=_live("CODE_SPECIFIED", "CODE_LIVE_UNMAPPED"),
        )

        self.assertEqual(
            ["CODE_LIVE_UNMAPPED"],
            result["candidates"][0]["unmapped_code_ids"],
        )
        self.assertEqual(
            [
                {
                    "area": "runtime_safety",
                    "affected_discovery_ids": ["DISC_00000000000000000001"],
                    "covered_code_ids": ["CODE_LIVE_UNMAPPED"],
                    "tests_affected": 1,
                    "code_facts_missing": 1,
                }
            ],
            result["unmapped_code_demand_ranking"],
        )

    def test_invented_trace_code_id_still_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "absent from live code inventory"):
            analyze_mapping_candidates(
                trace_records=[
                    _trace("DISC_00000000000000000001", "CODE_INVENTED"),
                ],
                code_mappings={},
                invariants={},
                live_code_facts={},
            )

    def test_multiple_invariants_remain_ambiguous_c3(self) -> None:
        result = analyze_mapping_candidates(
            trace_records=[
                _trace("DISC_00000000000000000001", "CODE_A", "CODE_B"),
            ],
            code_mappings={
                "CODE_A": _specified("INV_A"),
                "CODE_B": _specified("INV_B"),
            },
            invariants={
                "INV_A": {"risk": "maintenance"},
                "INV_B": {"risk": "maintenance"},
            },
        )

        candidate = result["candidates"][0]
        self.assertEqual("C3", candidate["candidate_tier"])
        self.assertEqual(["INV_A", "INV_B"], candidate["candidate_invariant_ids"])

    def test_spec_gap_only_execution_emits_demand_not_mapping(self) -> None:
        result = analyze_mapping_candidates(
            trace_records=[
                _trace("DISC_00000000000000000001", "CODE_GAP"),
            ],
            code_mappings={
                "CODE_GAP": {
                    "disposition": "spec_gap",
                    "spec_gap_ref": "SPEC_GAP_RUNTIME_001",
                }
            },
            invariants={},
        )

        self.assertEqual([], result["candidates"])
        self.assertEqual(
            [
                {
                    "discovery_id": "DISC_00000000000000000001",
                    "candidate_tier": "C4",
                    "covered_code_ids": ["CODE_GAP"],
                    "spec_gap_refs": ["SPEC_GAP_RUNTIME_001"],
                    "unmapped_code_ids": [],
                }
            ],
            result["demands"],
        )

    def test_specification_demand_ranking_aggregates_tests_and_exact_code_facts(self) -> None:
        result = analyze_mapping_candidates(
            trace_records=[
                _trace(
                    "DISC_00000000000000000001",
                    "CODE_GAP_A",
                    "CODE_GAP_SHARED",
                ),
                _trace(
                    "DISC_00000000000000000002",
                    "CODE_GAP_B",
                    "CODE_GAP_SHARED",
                ),
                _trace("DISC_00000000000000000003", "CODE_OTHER"),
            ],
            code_mappings={
                "CODE_GAP_A": {
                    "disposition": "spec_gap",
                    "spec_gap_ref": "SPEC_GAP_HIGH_YIELD",
                },
                "CODE_GAP_B": {
                    "disposition": "spec_gap",
                    "spec_gap_ref": "SPEC_GAP_HIGH_YIELD",
                },
                "CODE_GAP_SHARED": {
                    "disposition": "spec_gap",
                    "spec_gap_ref": "SPEC_GAP_HIGH_YIELD",
                },
                "CODE_OTHER": {
                    "disposition": "spec_gap",
                    "spec_gap_ref": "SPEC_GAP_LOW_YIELD",
                },
            },
            invariants={},
        )

        self.assertEqual(
            [
                {
                    "spec_gap_ref": "SPEC_GAP_HIGH_YIELD",
                    "affected_discovery_ids": [
                        "DISC_00000000000000000001",
                        "DISC_00000000000000000002",
                    ],
                    "covered_code_ids": [
                        "CODE_GAP_A",
                        "CODE_GAP_B",
                        "CODE_GAP_SHARED",
                    ],
                    "tests_unblocked": 2,
                    "code_facts_unblocked": 3,
                },
                {
                    "spec_gap_ref": "SPEC_GAP_LOW_YIELD",
                    "affected_discovery_ids": [
                        "DISC_00000000000000000003",
                    ],
                    "covered_code_ids": ["CODE_OTHER"],
                    "tests_unblocked": 1,
                    "code_facts_unblocked": 1,
                },
            ],
            result.get("demand_ranking"),
        )

    def test_mutation_kill_upgrades_single_fact_candidate_to_c1(self) -> None:
        result = analyze_mapping_candidates(
            trace_records=[
                _trace("DISC_00000000000000000001", "CODE_A"),
            ],
            code_mappings={"CODE_A": _specified("INV_ONE")},
            invariants={"INV_ONE": {"risk": "wrong_result"}},
            mutation_kills=[
                {
                    "discovery_id": "DISC_00000000000000000001",
                    "invariant_id": "INV_ONE",
                    "mutation_id": "MUT_001",
                    "outcome": "killed",
                }
            ],
        )

        candidate = result["candidates"][0]
        self.assertEqual("C1", candidate["candidate_tier"])
        self.assertEqual(["MUT_001"], candidate["mutation_kill_ids"])

    def test_output_order_is_deterministic(self) -> None:
        traces = [
            _trace("DISC_00000000000000000002", "CODE_B"),
            _trace("DISC_00000000000000000001", "CODE_A"),
        ]
        result = analyze_mapping_candidates(
            trace_records=traces,
            code_mappings={
                "CODE_A": _specified("INV_A"),
                "CODE_B": _specified("INV_B"),
            },
            invariants={
                "INV_A": {"risk": "maintenance"},
                "INV_B": {"risk": "maintenance"},
            },
        )

        self.assertEqual(
            [
                "DISC_00000000000000000001",
                "DISC_00000000000000000002",
            ],
            [row["discovery_id"] for row in result["candidates"]],
        )


if __name__ == "__main__":
    unittest.main()
