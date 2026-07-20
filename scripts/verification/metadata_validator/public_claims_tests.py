"""Tests for registered public-claim proof-or-gap traceability."""

from __future__ import annotations

import unittest
import tomllib
from pathlib import Path

from scripts.verification.metadata_validator.public_claims import (
    validate_public_claim_records,
)


class PublicClaimTests(unittest.TestCase):
    def test_live_public_claim_closeout_has_no_gap_fallback(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with (root / "verification/spec-gaps.toml").open("rb") as handle:
            gaps = {row["id"]: row for row in tomllib.load(handle)["spec_gaps"]}

        expected = {
            "SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001": {
                "RELEASE_PLATFORM_MATRIX_001",
                "PLAT_PATH_001",
                "PLAT_VSCODE_001",
            },
            "SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001": {
                "RUNTIME_BEHAVIOR_LOCKED_001",
                "DEBUG_BEHAVIOR_LOCKED_001",
            },
            "SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001": {
                "RELEASE_SOURCE_BUILD_OPENOT_001",
            },
            "SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001": {"REL_CLAIM_001"},
        }
        self.assertEqual(
            {gap_id for gap_id in expected if gaps[gap_id]["resolution_status"] != "closed"},
            set(),
        )
        invariants = {}
        for path in (root / "verification/invariants").rglob("*.toml"):
            with path.open("rb") as handle:
                row = tomllib.load(handle)
            invariants[row["id"]] = row
        proof_anchors = {
            "RELEASE_PLATFORM_MATRIX_001",
            "RUNTIME_BEHAVIOR_LOCKED_001",
            "DEBUG_BEHAVIOR_LOCKED_001",
            "RELEASE_SOURCE_BUILD_OPENOT_001",
            "REL_CLAIM_001",
        }
        self.assertEqual(
            {
                invariant_id
                for invariant_id in proof_anchors
                if invariants[invariant_id]["status"] != "validated"
            },
            set(),
        )

    def test_behavior_and_coverage_gap_links_keep_claim_debt_visible(self) -> None:
        for field, invariant in (
            (
                "behavior",
                invariant_record(behavior=[{"spec_gap_ref": "GAP"}]),
            ),
            (
                "coverage",
                invariant_record(
                    coverage={
                        "cells": [
                            {"state": "spec_gap", "spec_gap_ref": "GAP"}
                        ]
                    }
                ),
            ),
        ):
            with self.subTest(field=field):
                failures = validate(invariant=invariant)
                self.assertEqual(failures, [])

    def test_claim_without_proof_or_any_gap_is_rejected(self) -> None:
        failures = validate(invariant=invariant_record())

        self.assertEqual(
            failures,
            ["public claim CLAIM has no proof-backed invariant or explicit spec gap"],
        )

    def test_validated_green_backlink_is_a_proof_path(self) -> None:
        invariant = invariant_record(
            status="validated",
            evidence_refs=["EVID_GREEN"],
        )
        evidence = {
            "EVID_GREEN": {
                "proof_kind": "green",
                "linked_invariants": ["INV"],
            }
        }

        self.assertEqual(validate(invariant=invariant, evidence=evidence), [])


def validate(
    *,
    invariant: dict,
    evidence: dict[str, dict] | None = None,
) -> list[str]:
    failures: list[str] = []
    validate_public_claim_records(
        fail=lambda _path, message: failures.append(message),
        spec_sources={
            "CLAIM": {
                "id": "CLAIM",
                "authority": "public_claim",
                "_path": Path("verification/spec-sources.toml"),
            }
        },
        spec_gaps={"GAP": {"resolution_status": "open"}},
        invariants={"INV": invariant},
        required_specs={},
        evidence=evidence or {},
    )
    return failures


def invariant_record(**changes: object) -> dict:
    record = {
        "status": "unproven",
        "spec": {"source_refs": ["CLAIM"]},
        "spec_gap_refs": [],
        "coverage": {"cells": []},
        "behavior": [],
        "evidence_refs": [],
    }
    record.update(changes)
    return record


if __name__ == "__main__":
    unittest.main()
