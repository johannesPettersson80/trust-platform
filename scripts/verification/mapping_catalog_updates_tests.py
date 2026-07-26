"""Focused tests for generated catalog and ledger updates."""

from __future__ import annotations

import hashlib
import tomllib
import unittest

from scripts.verification.mapping_catalog_updates import (
    append_catalog_record,
    append_catalog_records,
    append_invariant_test,
    append_invariant_tests,
    build_accepted_catalog_record,
    map_denominator_review,
    reconcile_invariant_catalog_state,
    render_denominator_document,
)


DISCOVERY_ID = "DISC_00000000000000000001"


def _digest(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _proposal() -> dict[str, object]:
    source_digest = _digest(b"exact test source")
    return {
        "id": "MAP_CAND_00000000000000000001_A1B2C3D4",
        "proposal_digest": "sha256:" + "1" * 64,
        "discovery_id": DISCOVERY_ID,
        "discovery_source_kind": "rust_unit_test",
        "path": "crates/example/src/lib.rs",
        "name": "exact_case",
        "test_source_digest": source_digest,
        "assertion_source_digests": {
            "crates/example/src/lib.rs": source_digest,
            "conformance/expected/example.json": _digest(b"exact oracle"),
        },
    }


def _decision() -> dict[str, object]:
    return {
        "id": "MAP_DEC_" + "A" * 24,
        "outcome": "accepted",
        "proposal_id": _proposal()["id"],
        "proposal_digest": _proposal()["proposal_digest"],
        "discovery_id": DISCOVERY_ID,
        "reviewed_source_digest": _proposal()["test_source_digest"],
        "assertion_reviews": [
            {
                "assertion_id": "ASSERT_" + "A" * 24,
                "disposition": "oracle_bound",
                "behavior_refs": ["INV_ONE#behavior:0"],
                "rationale": "The assertion checks the selected behavior.",
            }
        ],
        "selected_invariant_ids": ["INV_ONE"],
        "selected_behavior_refs": ["INV_ONE#behavior:0"],
        "command": "cargo nextest run -p example --lib -E 'test(=exact_case)'",
        "command_authority": "verified_exact",
        "reviewer_ids": ["reviewer-a"],
        "confirmed_catalog": {
            "test_class": "unit",
            "area": "verification",
            "owner": "verification",
            "expected_result": "The exact assertion passes.",
            "expected_failure_mode": "The assertion accepts a wrong value.",
            "suite_tiers": ["pr"],
            "requires_hardware": False,
            "requires_network": False,
            "duration_class": "fast",
        },
    }


class MappingCatalogUpdateTests(unittest.TestCase):
    def test_catalog_record_generates_id_provenance_and_oracle_fields(self) -> None:
        record = build_accepted_catalog_record(
            proposal=_proposal(),
            decision=_decision(),
            invariants={
                "INV_ONE": {
                    "oracle": {"ref": "SPEC_EXAMPLE_001#exact"},
                }
            },
            reviewed_date="2026-07-23",
        )

        self.assertEqual(
            "TEST_AUTO_00000000000000000001",
            record.get("id"),
        )
        self.assertEqual("SPEC_EXAMPLE_001#exact", record.get("oracle_ref"))
        self.assertEqual(_decision()["id"], record.get("mapping_decision_id"))
        self.assertEqual(
            _proposal()["assertion_source_digests"],
            record.get("mapping_assertion_source_digests"),
        )
        self.assertEqual(
            "target/gate-artifacts/cases/TEST_AUTO_00000000000000000001.json",
            record.get("evidence_destination"),
        )
        self.assertRegex(
            str(record.get("mapping_assertion_review_digest")),
            r"^sha256:[0-9a-f]{64}$",
        )

    def test_assertion_review_digest_is_canonical_and_content_sensitive(self) -> None:
        decision = _decision()
        decision["assertion_reviews"].append(
            {
                "assertion_id": "ASSERT_" + "B" * 24,
                "disposition": "setup_or_precondition",
                "behavior_refs": [],
                "rationale": "The assertion establishes test setup.",
            }
        )
        reversed_decision = {
            **decision,
            "assertion_reviews": list(reversed(decision["assertion_reviews"])),
        }
        changed_decision = {
            **decision,
            "assertion_reviews": [
                {
                    **decision["assertion_reviews"][0],
                    "rationale": "Changed semantic review.",
                },
                decision["assertion_reviews"][1],
            ],
        }
        invariants = {
            "INV_ONE": {"oracle": {"ref": "SPEC_EXAMPLE_001#exact"}}
        }

        original = build_accepted_catalog_record(
            proposal=_proposal(),
            decision=decision,
            invariants=invariants,
            reviewed_date="2026-07-23",
        )
        reordered = build_accepted_catalog_record(
            proposal=_proposal(),
            decision=reversed_decision,
            invariants=invariants,
            reviewed_date="2026-07-23",
        )
        changed = build_accepted_catalog_record(
            proposal=_proposal(),
            decision=changed_decision,
            invariants=invariants,
            reviewed_date="2026-07-23",
        )

        self.assertEqual(
            original["mapping_assertion_review_digest"],
            reordered["mapping_assertion_review_digest"],
        )
        self.assertNotEqual(
            original["mapping_assertion_review_digest"],
            changed["mapping_assertion_review_digest"],
        )

    def test_catalog_append_and_invariant_update_preserve_existing_records(self) -> None:
        record = build_accepted_catalog_record(
            proposal=_proposal(),
            decision=_decision(),
            invariants={
                "INV_ONE": {"oracle": {"ref": "SPEC_EXAMPLE_001#exact"}}
            },
            reviewed_date="2026-07-23",
        )
        catalog = append_catalog_record(
            "# catalog\n\ntests = []\n",
            record,
        )
        invariant = append_invariant_test(
            'schema_version = 1\nid = "INV_ONE"\ntests = ["TEST_EXISTING"]\n',
            record["id"],
        )

        self.assertEqual([record], tomllib.loads(catalog)["tests"])
        self.assertEqual(
            ["TEST_AUTO_00000000000000000001", "TEST_EXISTING"],
            tomllib.loads(invariant)["tests"],
        )

    def test_invariant_test_batch_append_is_sorted_and_fail_closed(self) -> None:
        source = 'schema_version = 1\nid = "INV_ONE"\ntests = ["TEST_EXISTING"]\n'
        updated = append_invariant_tests(
            source,
            ["TEST_TWO", "TEST_ONE"],
        )
        self.assertEqual(
            ["TEST_EXISTING", "TEST_ONE", "TEST_TWO"],
            tomllib.loads(updated)["tests"],
        )
        with self.assertRaisesRegex(ValueError, "already references TEST_EXISTING"):
            append_invariant_tests(source, ["TEST_EXISTING"])
        with self.assertRaisesRegex(ValueError, "distinct non-empty strings"):
            append_invariant_tests(source, ["TEST_ONE", "TEST_ONE"])

    def test_catalog_batch_append_validates_all_identities_before_render(self) -> None:
        first = build_accepted_catalog_record(
            proposal=_proposal(),
            decision=_decision(),
            invariants={
                "INV_ONE": {"oracle": {"ref": "SPEC_EXAMPLE_001#exact"}}
            },
            reviewed_date="2026-07-23",
        )
        second = dict(first)
        second["id"] = "TEST_AUTO_00000000000000000002"
        second["discovery_id"] = "DISC_00000000000000000002"
        rendered = append_catalog_records(
            "# catalog\n\ntests = []\n",
            [first, second],
        )
        self.assertEqual(
            [first, second],
            tomllib.loads(rendered)["tests"],
        )
        duplicate = dict(second)
        duplicate["id"] = first["id"]
        with self.assertRaisesRegex(ValueError, "already contains"):
            append_catalog_records(
                "# catalog\n\ntests = []\n",
                [first, duplicate],
            )

    def test_catalog_reconciliation_closes_only_fully_mapped_behavior_dimensions(
        self,
    ) -> None:
        invariant = """
schema_version = 1
id = "INV_ONE"
tests = ["TEST_ONE"]
missing = ["cataloged_behavior_tests", "assertion_reachability", "broad_remote_gate"]

[[behavior]]
case_family = "happy_path"

[[behavior]]
case_family = "boundary_high"

[[coverage.cells]]
dimension = "happy_path"
state = "gap_open"
rationale = "Awaiting catalog acceptance."

[[coverage.cells]]
dimension = "boundary_high"
state = "gap_open"
rationale = "Awaiting catalog acceptance."
"""
        partial_catalog = """
[[tests]]
id = "TEST_ONE"
invariants = ["INV_ONE"]
mapping_behavior_refs = ["INV_ONE#behavior:0"]
"""
        partial = reconcile_invariant_catalog_state(
            invariant,
            invariant_id="INV_ONE",
            catalog_source=partial_catalog,
        )
        partial_document = tomllib.loads(partial)
        self.assertIn("cataloged_behavior_tests", partial_document["missing"])
        self.assertEqual(
            ["covered", "gap_open"],
            [
                cell["state"]
                for cell in partial_document["coverage"]["cells"]
            ],
        )

        complete_catalog = partial_catalog + """
[[tests]]
id = "TEST_TWO"
invariants = ["INV_ONE"]
mapping_behavior_refs = ["INV_ONE#behavior:1"]
"""
        complete = reconcile_invariant_catalog_state(
            invariant,
            invariant_id="INV_ONE",
            catalog_source=complete_catalog,
        )
        complete_document = tomllib.loads(complete)
        self.assertNotIn(
            "cataloged_behavior_tests",
            complete_document["missing"],
        )
        self.assertEqual(
            ["covered", "covered"],
            [
                cell["state"]
                for cell in complete_document["coverage"]["cells"]
            ],
        )
        self.assertTrue(
            all(
                "dynamic proof and broad remote validation remain explicit debt"
                in cell["rationale"]
                for cell in complete_document["coverage"]["cells"]
            )
        )

    def test_catalog_reconciliation_requires_complete_invariant_partitions(
        self,
    ) -> None:
        catalog = """
[[tests]]
id = "TEST_ONE"
invariants = ["INV_ONE"]
mapping_behavior_refs = ["INV_ONE#behavior:0"]
"""
        sources = {
            "behavior": """
schema_version = 1
id = "INV_ONE"
missing = ["cataloged_behavior_tests"]
[[coverage.cells]]
dimension = "happy_path"
state = "gap_open"
rationale = "Awaiting catalog acceptance."
""",
            "coverage": """
schema_version = 1
id = "INV_ONE"
missing = ["cataloged_behavior_tests"]
[[behavior]]
case_family = "happy_path"
""",
            "missing": """
schema_version = 1
id = "INV_ONE"
[[behavior]]
case_family = "happy_path"
[[coverage.cells]]
dimension = "happy_path"
state = "gap_open"
rationale = "Awaiting catalog acceptance."
""",
        }
        for partition, source in sources.items():
            with self.subTest(partition=partition):
                with self.assertRaisesRegex(
                    ValueError,
                    f"requires .*{partition}",
                ):
                    reconcile_invariant_catalog_state(
                        source,
                        invariant_id="INV_ONE",
                        catalog_source=catalog,
                    )

    def test_catalog_reconciliation_requires_one_cell_per_behavior_dimension(
        self,
    ) -> None:
        invariant = """
schema_version = 1
id = "INV_ONE"
missing = ["cataloged_behavior_tests"]
[[behavior]]
case_family = "happy_path"
[[coverage.cells]]
dimension = "happy_path"
state = "gap_open"
rationale = "Awaiting catalog acceptance."
[[coverage.cells]]
dimension = "happy_path"
state = "gap_open"
rationale = "Awaiting catalog acceptance."
"""
        with self.assertRaisesRegex(
            ValueError,
            "exactly one coverage cell for behavior dimension happy_path",
        ):
            reconcile_invariant_catalog_state(
                invariant,
                invariant_id="INV_ONE",
                catalog_source="tests = []\n",
            )

    def test_catalog_reconciliation_preserves_existing_covered_states(
        self,
    ) -> None:
        invariant = """
schema_version = 1
id = "INV_ONE"
missing = ["cataloged_behavior_tests"]
[[behavior]]
case_family = "happy_path"
[[behavior]]
case_family = "boundary_high"
[[coverage.cells]]
dimension = "happy_path"
state = "covered"
rationale = "Existing behavior proof remains authoritative."
[[coverage.cells]]
dimension = "boundary_high"
state = "covered_by_fuzz"
rationale = "Existing fuzz proof remains authoritative."
"""
        catalog = """
[[tests]]
id = "TEST_ONE"
invariants = ["INV_ONE"]
mapping_behavior_refs = [
  "INV_ONE#behavior:0",
  "INV_ONE#behavior:1",
]
"""
        reconciled = tomllib.loads(
            reconcile_invariant_catalog_state(
                invariant,
                invariant_id="INV_ONE",
                catalog_source=catalog,
            )
        )
        self.assertEqual(
            ["covered", "covered_by_fuzz"],
            [cell["state"] for cell in reconciled["coverage"]["cells"]],
        )
        self.assertEqual(
            [
                "Existing behavior proof remains authoritative.",
                "Existing fuzz proof remains authoritative.",
            ],
            [cell["rationale"] for cell in reconciled["coverage"]["cells"]],
        )

    def test_catalog_reconciliation_rejects_non_catalog_coverage_debt(
        self,
    ) -> None:
        catalog = """
[[tests]]
id = "TEST_ONE"
invariants = ["INV_ONE"]
mapping_behavior_refs = ["INV_ONE#behavior:0"]
"""
        for state, rationale in (
            ("gap_open", "External target proof remains open."),
            ("spec_gap", "Normative behavior is unresolved."),
            ("blocked", "Hardware is unavailable."),
            ("deferred", "Reserved for a later release."),
            ("not_applicable", "Reviewed as outside this product."),
        ):
            invariant = f"""
schema_version = 1
id = "INV_ONE"
missing = ["cataloged_behavior_tests"]
[[behavior]]
case_family = "happy_path"
[[coverage.cells]]
dimension = "happy_path"
state = "{state}"
rationale = "{rationale}"
"""
            with self.subTest(state=state):
                with self.assertRaisesRegex(
                    ValueError,
                    "cannot close behavior dimension happy_path",
                ):
                    reconcile_invariant_catalog_state(
                        invariant,
                        invariant_id="INV_ONE",
                        catalog_source=catalog,
                    )

    def test_denominator_mapping_is_generated_and_canonically_sorted(self) -> None:
        document = {
            "schema_version": 1,
            "id": "TEST_CATALOG_DENOMINATOR_REVIEW_V1",
            "title": "Exact denominator",
            "denominator_basis": "all_live_existing_test_scanner_facts",
            "review_basis": "explicit_per_discovery_id_no_semantic_inference",
            "reason_definitions": {
                "no_reviewed_spec_or_invariant_binding": "reason",
            },
            "last_reviewed": "2026-07-22",
            "reviews": [
                {
                    "discovery_id": "DISC_00000000000000000002",
                    "discovery_source_kind": "rust_unit_test",
                    "path": "b.rs",
                    "name": "b",
                    "ignore_state": "not_ignored",
                    "disposition": "reviewed_nonmapping",
                    "reason_code": "no_reviewed_spec_or_invariant_binding",
                    "last_reviewed": "2026-07-22",
                },
                {
                    "discovery_id": DISCOVERY_ID,
                    "discovery_source_kind": "rust_unit_test",
                    "path": "crates/example/src/lib.rs",
                    "name": "exact_case",
                    "ignore_state": "not_ignored",
                    "disposition": "reviewed_nonmapping",
                    "reason_code": "no_reviewed_spec_or_invariant_binding",
                    "last_reviewed": "2026-07-22",
                },
            ],
        }

        updated = map_denominator_review(
            document=document,
            proposal=_proposal(),
            catalog_test_id="TEST_AUTO_00000000000000000001",
            reviewed_date="2026-07-23",
        )
        parsed = tomllib.loads(render_denominator_document(updated))

        self.assertEqual("2026-07-23", parsed["last_reviewed"])
        self.assertEqual(
            [DISCOVERY_ID, "DISC_00000000000000000002"],
            [row["discovery_id"] for row in parsed["reviews"]],
        )
        self.assertEqual("catalog_mapped", parsed["reviews"][0]["disposition"])
        self.assertNotIn("reason_code", parsed["reviews"][0])


if __name__ == "__main__":
    unittest.main()
