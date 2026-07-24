"""Focused tests for deterministic specification-demand refinement."""

from __future__ import annotations

import json
import unittest

from scripts.verification.mapping_demand_refinement import (
    build_spec_demand_observations,
    build_structural_demand_ranking,
    classify_demand_affinity,
    structural_cell_id,
    structural_cell_key,
)


def _trace(discovery_id: str, *code_ids: str, **extra: object) -> dict[str, object]:
    return {
        "discovery_id": discovery_id,
        "trace_status": "observed",
        "covered_code_ids": list(code_ids),
        **extra,
    }


def _fact(
    code_id: str,
    *,
    path: str,
    qualified_symbol: str,
    gap: str = "SPEC_GAP_PARENT",
    disposition: str = "spec_gap",
) -> dict[str, object]:
    row: dict[str, object] = {
        "code_id": code_id,
        "disposition": disposition,
        "area": "runtime_safety",
        "owner": "trust-runtime",
        "source_kind": "rust",
        "path": path,
        "qualified_symbol": qualified_symbol,
        "item_kind": "rust_function",
    }
    if disposition == "spec_gap":
        row["spec_gap_ref"] = gap
    elif disposition == "specified":
        row["invariant_ids"] = ["INV_EXISTING"]
    return row


class MappingDemandRefinementTests(unittest.TestCase):
    def test_structural_cell_key_and_id_use_only_mechanical_fact_fields(self) -> None:
        fact = _fact(
            "CODE_A",
            path="crates/example/src/worker.rs",
            qualified_symbol="example::worker::Machine::run",
        )

        key = structural_cell_key(parent_spec_gap_ref="SPEC_GAP_PARENT", fact=fact)

        self.assertEqual(
            (
                "SPEC_GAP_PARENT",
                "runtime_safety",
                "trust-runtime",
                "rust",
                "crates/example/src/worker.rs",
                "example::worker::Machine",
                "rust_function",
            ),
            key,
        )
        self.assertEqual(structural_cell_id(key), structural_cell_id(tuple(key)))
        self.assertRegex(structural_cell_id(key), r"^DEMAND_CELL_[0-9A-F]{64}$")

    def test_mixed_specified_and_spec_gap_trace_still_emits_observation(self) -> None:
        mappings = {
            "CODE_SPECIFIED": _fact(
                "CODE_SPECIFIED",
                path="crates/example/src/lib.rs",
                qualified_symbol="example::already_specified",
                disposition="specified",
            ),
            "CODE_GAP": _fact(
                "CODE_GAP",
                path="crates/example/src/gap.rs",
                qualified_symbol="example::gap::missing_behavior",
            ),
        }

        observations = build_spec_demand_observations(
            trace_records=[
                _trace("DISC_MIXED", "CODE_SPECIFIED", "CODE_GAP"),
            ],
            code_mappings=mappings,
        )

        self.assertEqual(1, len(observations))
        self.assertEqual("DISC_MIXED", observations[0]["discovery_id"])
        self.assertEqual(["CODE_GAP"], observations[0]["spec_gap_code_ids"])
        self.assertEqual("R1", observations[0]["affinity_tier"])

    def test_two_files_under_one_parent_gap_remain_separate_cells(self) -> None:
        mappings = {
            "CODE_A": _fact(
                "CODE_A",
                path="crates/example/src/a.rs",
                qualified_symbol="example::a::run",
            ),
            "CODE_B": _fact(
                "CODE_B",
                path="crates/example/src/b.rs",
                qualified_symbol="example::b::run",
            ),
        }

        observation = build_spec_demand_observations(
            trace_records=[_trace("DISC_TWO_FILES", "CODE_A", "CODE_B")],
            code_mappings=mappings,
        )[0]

        self.assertEqual(2, len(observation["cells"]))
        self.assertEqual("R3", observation["affinity_tier"])

    def test_unmapped_live_fact_emits_ambiguous_observation(self) -> None:
        observations = build_spec_demand_observations(
            trace_records=[_trace("DISC_UNMAPPED", "CODE_UNMAPPED")],
            code_mappings={},
            live_code_facts={
                "CODE_UNMAPPED": {
                    "code_id": "CODE_UNMAPPED",
                    "area": "runtime_safety",
                }
            },
        )

        self.assertEqual(1, len(observations))
        self.assertEqual([], observations[0]["spec_gap_code_ids"])
        self.assertEqual(["CODE_UNMAPPED"], observations[0]["unmapped_code_ids"])
        self.assertEqual([], observations[0]["cells"])
        self.assertEqual("R3", observations[0]["affinity_tier"])

    def test_mechanical_source_move_changes_cell_identity_only(self) -> None:
        before = _fact(
            "CODE_A",
            path="crates/example/src/before.rs",
            qualified_symbol="example::worker::run",
        )
        after = {
            **before,
            "path": "crates/example/src/after.rs",
        }

        before_cell = build_spec_demand_observations(
            trace_records=[_trace("DISC_A", "CODE_A")],
            code_mappings={"CODE_A": before},
        )[0]["cells"][0]
        after_cell = build_spec_demand_observations(
            trace_records=[_trace("DISC_A", "CODE_A")],
            code_mappings={"CODE_A": after},
        )[0]["cells"][0]

        self.assertNotEqual(before_cell["cell_id"], after_cell["cell_id"])
        self.assertEqual(["CODE_A"], before_cell["covered_code_ids"])
        self.assertEqual(["CODE_A"], after_cell["covered_code_ids"])
        self.assertNotIn("invariant_id", before_cell)
        self.assertNotIn("invariant_id", after_cell)

    def test_affinity_is_r1_for_one_cell_and_r2_for_unique_eighty_percent(self) -> None:
        one_cell = [{"cell_id": "CELL_A", "covered_code_ids": ["A", "B"]}]
        dominant = [
            {
                "cell_id": "CELL_A",
                "covered_code_ids": ["A1", "A2", "A3", "A4"],
            },
            {"cell_id": "CELL_B", "covered_code_ids": ["B1"]},
        ]

        self.assertEqual(
            {
                "affinity_tier": "R1",
                "dominant_cell_id": "CELL_A",
                "dominant_concentration_ppm": 1_000_000,
            },
            classify_demand_affinity(cells=one_cell, unmapped_code_ids=[]),
        )
        self.assertEqual(
            {
                "affinity_tier": "R2",
                "dominant_cell_id": "CELL_A",
                "dominant_concentration_ppm": 800_000,
            },
            classify_demand_affinity(cells=dominant, unmapped_code_ids=[]),
        )

    def test_tie_or_unmapped_fact_is_r3_ambiguous(self) -> None:
        tied = [
            {"cell_id": "CELL_A", "covered_code_ids": ["A"]},
            {"cell_id": "CELL_B", "covered_code_ids": ["B"]},
        ]

        self.assertEqual(
            {
                "affinity_tier": "R3",
                "dominant_cell_id": "",
                "dominant_concentration_ppm": 500_000,
            },
            classify_demand_affinity(cells=tied, unmapped_code_ids=[]),
        )
        self.assertEqual(
            "R3",
            classify_demand_affinity(
                cells=[{"cell_id": "CELL_A", "covered_code_ids": ["A"]}],
                unmapped_code_ids=["CODE_UNMAPPED"],
            )["affinity_tier"],
        )

    def test_ranking_reports_exact_unions_and_honest_test_counts(self) -> None:
        mappings = {
            "CODE_A1": _fact(
                "CODE_A1",
                path="crates/example/src/a.rs",
                qualified_symbol="example::a::run",
            ),
            "CODE_A2": _fact(
                "CODE_A2",
                path="crates/example/src/a.rs",
                qualified_symbol="example::a::stop",
            ),
            "CODE_B": _fact(
                "CODE_B",
                path="crates/example/src/b.rs",
                qualified_symbol="example::b::run",
            ),
        }
        observations = build_spec_demand_observations(
            trace_records=[
                _trace("DISC_EXCLUSIVE", "CODE_A1"),
                _trace("DISC_DOMINANT", "CODE_A1", "CODE_A2", "CODE_B"),
                _trace("DISC_AMBIGUOUS", "CODE_A2", "CODE_B"),
            ],
            code_mappings=mappings,
            dominant_concentration_ppm=600_000,
        )

        ranking = build_structural_demand_ranking(observations)
        cell_a = next(row for row in ranking if row["path"].endswith("/a.rs"))
        cell_b = next(row for row in ranking if row["path"].endswith("/b.rs"))

        self.assertEqual(
            ["DISC_AMBIGUOUS", "DISC_DOMINANT", "DISC_EXCLUSIVE"],
            cell_a["affected_discovery_ids"],
        )
        self.assertEqual(["CODE_A1", "CODE_A2"], cell_a["covered_code_ids"])
        self.assertEqual(3, cell_a["tests_touching"])
        self.assertEqual(1, cell_a["tests_exclusive"])
        self.assertEqual(1, cell_a["tests_dominant"])
        self.assertEqual(2, cell_b["tests_touching"])
        self.assertEqual(0, cell_b["tests_exclusive"])
        self.assertEqual(0, cell_b["tests_dominant"])
        self.assertNotIn("tests_unblocked", cell_a)

    def test_input_order_is_byte_deterministic(self) -> None:
        mappings = {
            "CODE_A": _fact(
                "CODE_A",
                path="crates/example/src/a.rs",
                qualified_symbol="example::a::run",
            ),
            "CODE_B": _fact(
                "CODE_B",
                path="crates/example/src/b.rs",
                qualified_symbol="example::b::run",
            ),
        }
        forward = build_spec_demand_observations(
            trace_records=[
                _trace("DISC_B", "CODE_B", "CODE_A"),
                _trace("DISC_A", "CODE_A"),
            ],
            code_mappings=mappings,
        )
        reverse = build_spec_demand_observations(
            trace_records=[
                _trace("DISC_A", "CODE_A"),
                _trace("DISC_B", "CODE_A", "CODE_B"),
            ],
            code_mappings=dict(reversed(list(mappings.items()))),
        )

        self.assertEqual(
            json.dumps(forward, sort_keys=True, separators=(",", ":")),
            json.dumps(reverse, sort_keys=True, separators=(",", ":")),
        )
        self.assertEqual(
            build_structural_demand_ranking(forward),
            build_structural_demand_ranking(reverse),
        )

    def test_test_names_paths_and_reference_candidates_do_not_affect_output(self) -> None:
        mappings = {
            "CODE_A": _fact(
                "CODE_A",
                path="crates/example/src/a.rs",
                qualified_symbol="example::a::run",
            )
        }
        baseline = build_spec_demand_observations(
            trace_records=[_trace("DISC_A", "CODE_A")],
            code_mappings=mappings,
        )
        lexical_noise = build_spec_demand_observations(
            trace_records=[
                _trace(
                    "DISC_A",
                    "CODE_A",
                    name="SPEC_GAP_INVENTED",
                    path="tests/misleading_name.rs",
                    reference_candidates=["INV_INVENTED"],
                )
            ],
            code_mappings=mappings,
        )

        self.assertEqual(baseline, lexical_noise)


if __name__ == "__main__":
    unittest.main()
