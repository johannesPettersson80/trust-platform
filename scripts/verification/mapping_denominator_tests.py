"""Focused tests for scanner-driven denominator reconciliation."""

from __future__ import annotations

import unittest

from scripts.verification.mapping_denominator import (
    rebind_reviewed_nonmapping_identity,
    reconcile_denominator_document,
)
from scripts.verification.test_catalog_common import make_fact
from scripts.verification.test_catalog_denominator import (
    NONMAPPING_REASON_DEFINITIONS,
    analyze_test_catalog_denominator,
)


def _fact(
    name: str,
    *,
    line: int,
    source_kind: str = "rust_unit_test",
    ignore_state: str = "not_ignored",
):
    return make_fact(
        source_kind=source_kind,
        name=name,
        path=f"tests/{name}.rs",
        line=line,
        package="fixture",
        command_hint=f"run {name}",
        command_hint_authority="exact",
        discovery_confidence="exact_attribute",
        ignore_state=ignore_state,
        ignore_reason="fixture" if ignore_state != "not_ignored" else None,
    )


def _document(reviews: list[dict]) -> dict:
    return {
        "schema_version": 1,
        "id": "TEST_CATALOG_DENOMINATOR_REVIEW_V1",
        "title": "Fixture denominator",
        "denominator_basis": "all_live_existing_test_scanner_facts",
        "review_basis": "explicit_per_discovery_id_no_semantic_inference",
        "reason_definitions": dict(NONMAPPING_REASON_DEFINITIONS),
        "last_reviewed": "2026-07-20",
        "reviews": reviews,
    }


class MappingDenominatorTests(unittest.TestCase):
    def test_reconcile_generates_complete_canonical_structural_partition(self) -> None:
        mapped = _fact("mapped", line=1)
        ordinary = _fact("ordinary", line=2)
        ignored = _fact("ignored", line=3, ignore_state="ignored")
        fuzz = _fact("fuzz", line=4, source_kind="fuzz_target")
        gate = _fact("gate", line=5, source_kind="gate_script")
        stale = _fact("stale", line=6)
        existing = [
            {
                "discovery_id": ordinary.stable_id,
                "discovery_source_kind": ordinary.source_kind,
                "path": ordinary.path,
                "name": ordinary.name,
                "ignore_state": ordinary.ignore_state,
                "disposition": "reviewed_nonmapping",
                "reason_code": "no_reviewed_spec_or_invariant_binding",
                "last_reviewed": "2026-07-20",
            },
            {
                "discovery_id": stale.stable_id,
                "discovery_source_kind": stale.source_kind,
                "path": stale.path,
                "name": stale.name,
                "ignore_state": stale.ignore_state,
                "disposition": "reviewed_nonmapping",
                "reason_code": "no_reviewed_spec_or_invariant_binding",
                "last_reviewed": "2026-07-20",
            },
        ]
        tests = [
            {
                "id": "TEST_MAPPED",
                "subject_kind": "generated_test",
                "discovery_id": mapped.stable_id,
            }
        ]

        result = reconcile_denominator_document(
            document=_document(list(reversed(existing))),
            facts=[gate, fuzz, ignored, ordinary, mapped],
            tests=tests,
            ignored_tests=[
                {
                    "discovery_id": ignored.stable_id,
                    "ignore_state": "ignored",
                }
            ],
            reviewed_date="2026-07-23",
        )

        self.assertEqual(
            sorted(row["discovery_id"] for row in result["reviews"]),
            [row["discovery_id"] for row in result["reviews"]],
        )
        self.assertNotIn(
            stale.stable_id,
            {row["discovery_id"] for row in result["reviews"]},
        )
        by_id = {row["discovery_id"]: row for row in result["reviews"]}
        self.assertEqual("catalog_mapped", by_id[mapped.stable_id]["disposition"])
        self.assertEqual(
            "no_reviewed_spec_or_invariant_binding",
            by_id[ordinary.stable_id]["reason_code"],
        )
        self.assertEqual(
            "ignored_test_registry_owned",
            by_id[ignored.stable_id]["reason_code"],
        )
        self.assertEqual("fuzz_program_owned", by_id[fuzz.stable_id]["reason_code"])
        self.assertEqual("gate_inventory_owned", by_id[gate.stable_id]["reason_code"])
        self.assertEqual("2026-07-20", by_id[ordinary.stable_id]["last_reviewed"])
        self.assertEqual("2026-07-23", by_id[mapped.stable_id]["last_reviewed"])
        self.assertEqual("2026-07-23", result["last_reviewed"])
        analysis = analyze_test_catalog_denominator(
            facts=[gate, fuzz, ignored, ordinary, mapped],
            tests=tests,
            reviews=result["reviews"],
            ignored_tests=[
                {
                    "discovery_id": ignored.stable_id,
                    "ignore_state": "ignored",
                }
            ],
        )
        self.assertTrue(analysis["summary"]["exhaustive"])
        self.assertEqual(0, analysis["summary"]["unreviewed_facts"])

    def test_reconcile_fails_when_ignored_fact_lacks_registry_authority(self) -> None:
        ignored = _fact("ignored", line=3, ignore_state="conditional")

        with self.assertRaisesRegex(ValueError, "lacks exact ignored registry"):
            reconcile_denominator_document(
                document=_document([]),
                facts=[ignored],
                tests=[],
                ignored_tests=[],
                reviewed_date="2026-07-23",
            )

    def test_rebind_changes_only_one_unmapped_name_identity_and_date(self) -> None:
        old = _fact("old_name", line=1)
        new = _fact("new_name", line=1)
        document = _document(
            [
                {
                    "discovery_id": old.stable_id,
                    "discovery_source_kind": old.source_kind,
                    "path": old.path,
                    "name": old.name,
                    "ignore_state": old.ignore_state,
                    "disposition": "reviewed_nonmapping",
                    "reason_code": "no_reviewed_spec_or_invariant_binding",
                    "last_reviewed": "2026-07-20",
                }
            ]
        )
        new = make_fact(
            source_kind=new.source_kind,
            name=new.name,
            path=old.path,
            line=new.line,
            package=new.package,
            command_hint=new.command_hint,
            command_hint_authority=new.command_hint_authority,
            discovery_confidence=new.discovery_confidence,
        )

        rebound = rebind_reviewed_nonmapping_identity(
            document=document,
            old_discovery_id=old.stable_id,
            old_name=old.name,
            new_fact=new,
            reviewed_date="2026-07-26",
        )

        self.assertEqual("2026-07-26", rebound["last_reviewed"])
        self.assertEqual(new.stable_id, rebound["reviews"][0]["discovery_id"])
        self.assertEqual(new.name, rebound["reviews"][0]["name"])
        self.assertEqual(
            "no_reviewed_spec_or_invariant_binding",
            rebound["reviews"][0]["reason_code"],
        )


if __name__ == "__main__":
    unittest.main()
