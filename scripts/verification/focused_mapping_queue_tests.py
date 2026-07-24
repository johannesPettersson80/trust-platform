"""Tests for lossless focused proposal-queue updates."""

from __future__ import annotations

import unittest

from scripts.verification.focused_mapping_queue import merge_focused_queue


class FocusedMappingQueueTests(unittest.TestCase):
    def test_focused_proposal_preserves_unselected_demand_backlog(self) -> None:
        old_discovery = "DISC_" + "1" * 20
        selected_discovery = "DISC_" + "2" * 20
        gap_code = "CODE_" + "A" * 64
        merged_candidate, proposals = merge_focused_queue(
            existing_queue={
                "demands": [
                    _demand(old_discovery, gap_code),
                    _demand(selected_discovery, gap_code),
                ],
                "proposals": [],
                "unmapped_code_demand_ranking": [],
            },
            focused_candidate_document={
                "candidates": [{"discovery_id": selected_discovery}],
                "demands": [],
                "spec_demand_observations": [],
                "demand_ranking": [],
                "structural_demand_ranking": [],
                "unmapped_code_demand_ranking": [],
            },
            focused_proposals=[
                {"discovery_id": selected_discovery, "id": "NEW"}
            ],
            selected_discovery_ids=[selected_discovery],
            code_mappings={gap_code: _gap_mapping(gap_code)},
        )

        self.assertEqual(
            [old_discovery],
            [
                row["discovery_id"]
                for row in merged_candidate["demands"]
            ],
        )
        self.assertEqual(
            [selected_discovery],
            sorted(str(row["discovery_id"]) for row in proposals),
        )
        self.assertEqual(
            [old_discovery],
            merged_candidate["demand_ranking"][0][
                "affected_discovery_ids"
            ],
        )
        self.assertEqual(
            [old_discovery],
            merged_candidate["structural_demand_ranking"][0][
                "affected_discovery_ids"
            ],
        )

    def test_focused_queue_rejects_pending_other_batch(self) -> None:
        selected_discovery = "DISC_" + "2" * 20
        pending_discovery = "DISC_" + "3" * 20
        with self.assertRaisesRegex(
            ValueError, "pending proposals from another batch"
        ):
            merge_focused_queue(
                existing_queue={
                    "demands": [],
                    "proposals": [
                        {
                            "discovery_id": pending_discovery,
                            "id": "PENDING",
                        }
                    ],
                    "unmapped_code_demand_ranking": [],
                },
                focused_candidate_document={
                    "candidates": [],
                    "demands": [],
                    "spec_demand_observations": [],
                    "demand_ranking": [],
                    "structural_demand_ranking": [],
                    "unmapped_code_demand_ranking": [],
                },
                focused_proposals=[],
                selected_discovery_ids=[selected_discovery],
                code_mappings={},
            )

    def test_focused_empty_result_removes_only_selected_records(self) -> None:
        old_discovery = "DISC_" + "1" * 20
        selected_discovery = "DISC_" + "2" * 20
        gap_code = "CODE_" + "A" * 64
        merged_candidate, proposals = merge_focused_queue(
            existing_queue={
                "demands": [_demand(old_discovery, gap_code)],
                "proposals": [
                    {"discovery_id": selected_discovery, "id": "STALE"}
                ],
                "unmapped_code_demand_ranking": [],
            },
            focused_candidate_document={
                "candidates": [],
                "demands": [],
                "spec_demand_observations": [],
                "demand_ranking": [],
                "structural_demand_ranking": [],
                "unmapped_code_demand_ranking": [],
            },
            focused_proposals=[],
            selected_discovery_ids=[selected_discovery],
            code_mappings={gap_code: _gap_mapping(gap_code)},
        )

        self.assertEqual(1, len(merged_candidate["demands"]))
        self.assertEqual([], proposals)

    def test_unmapped_demand_requires_preserved_area_authority(self) -> None:
        discovery_id = "DISC_" + "1" * 20
        unmapped_code = "CODE_" + "B" * 64
        with self.assertRaisesRegex(
            ValueError, "lacks preserved area authority"
        ):
            merge_focused_queue(
                existing_queue={
                    "demands": [
                        {
                            "discovery_id": discovery_id,
                            "candidate_tier": "C4",
                            "covered_code_ids": [unmapped_code],
                            "spec_gap_refs": [],
                            "unmapped_code_ids": [unmapped_code],
                        }
                    ],
                    "proposals": [],
                    "unmapped_code_demand_ranking": [],
                },
                focused_candidate_document={
                    "candidates": [],
                    "demands": [],
                    "spec_demand_observations": [],
                    "demand_ranking": [],
                    "structural_demand_ranking": [],
                    "unmapped_code_demand_ranking": [],
                },
                focused_proposals=[],
                selected_discovery_ids=["DISC_" + "2" * 20],
                code_mappings={},
            )


def _demand(discovery_id: str, code_id: str) -> dict[str, object]:
    return {
        "discovery_id": discovery_id,
        "candidate_tier": "C4",
        "covered_code_ids": [code_id],
        "spec_gap_refs": ["SPEC_GAP_FIXTURE"],
        "unmapped_code_ids": [],
    }


def _gap_mapping(code_id: str) -> dict[str, object]:
    return {
        "code_id": code_id,
        "disposition": "spec_gap",
        "spec_gap_ref": "SPEC_GAP_FIXTURE",
        "area": "verification",
        "owner": "verification",
        "source_kind": "rust",
        "path": "crates/example/src/lib.rs",
        "qualified_symbol": "example::fixture",
        "item_kind": "rust_function",
    }


if __name__ == "__main__":
    unittest.main()
