"""Tests for the exhaustive reviewed existing-test catalog denominator."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from scripts.verification.test_catalog_denominator import (
    NONMAPPING_REASON_CODES,
    NONMAPPING_REASON_DEFINITIONS,
    analyze_test_catalog_denominator,
    validate_test_catalog_denominator_document,
)
from scripts.verification.test_catalog_models import InferredTestFact


def _fact(
    stable_id: str,
    name: str,
    *,
    source_kind: str = "rust_unit_test",
    path: str = "crates/trust-runtime/src/example.rs",
    ignore_state: str = "not_ignored",
) -> InferredTestFact:
    return InferredTestFact(
        stable_id=stable_id,
        native_id=name,
        source_kind=source_kind,
        name=name,
        path=path,
        line=10,
        package="trust-runtime",
        command_hint="cargo test -p trust-runtime example -- --exact",
        command_hint_authority="conservative",
        discovery_confidence="exact_attribute",
        ignore_state=ignore_state,
        ignore_reason="reviewed skip" if ignore_state != "not_ignored" else None,
        reference_candidates=(),
    )


def _mapped_review(fact: InferredTestFact) -> dict[str, object]:
    return {
        "discovery_id": fact.stable_id,
        "discovery_source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
        "ignore_state": fact.ignore_state,
        "disposition": "catalog_mapped",
        "catalog_test_id": "TEST_EXAMPLE_001",
        "last_reviewed": "2026-07-18",
    }


def _nonmapping_review(
    fact: InferredTestFact,
    reason_code: str = "no_reviewed_spec_or_invariant_binding",
) -> dict[str, object]:
    return {
        "discovery_id": fact.stable_id,
        "discovery_source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
        "ignore_state": fact.ignore_state,
        "disposition": "reviewed_nonmapping",
        "reason_code": reason_code,
        "last_reviewed": "2026-07-18",
    }


class TestCatalogDenominatorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.mapped = _fact("DISC_00000000000000000001", "mapped_test")
        self.unmapped = _fact("DISC_00000000000000000002", "supporting_test")
        self.catalog = [
            {
                "id": "TEST_EXAMPLE_001",
                "subject_kind": "generated_test",
                "discovery_id": self.mapped.stable_id,
            }
        ]

    def analyze(self, reviews, *, facts=None, tests=None, ignored_tests=()):
        return analyze_test_catalog_denominator(
            facts=facts or [self.mapped, self.unmapped],
            tests=tests or self.catalog,
            reviews=reviews,
            ignored_tests=ignored_tests,
        )

    def test_complete_partition_is_disjoint_and_exhaustive(self) -> None:
        result = self.analyze(
            [_mapped_review(self.mapped), _nonmapping_review(self.unmapped)]
        )

        self.assertEqual(2, result["summary"]["scanner_facts"])
        self.assertEqual(1, result["summary"]["catalog_mapped_facts"])
        self.assertEqual(1, result["summary"]["reviewed_nonmapping_facts"])
        self.assertEqual(0, result["summary"]["unreviewed_facts"])
        self.assertTrue(result["summary"]["exhaustive"])
        self.assertEqual(
            set(NONMAPPING_REASON_CODES),
            set(result["summary"]["by_nonmapping_reason"]),
        )

    def test_new_scanner_fact_blocks_closure(self) -> None:
        new_fact = _fact("DISC_00000000000000000003", "new_test")
        with self.assertRaisesRegex(ValueError, "unreviewed live scanner facts"):
            self.analyze(
                [_mapped_review(self.mapped), _nonmapping_review(self.unmapped)],
                facts=[self.mapped, self.unmapped, new_fact],
            )

    def test_deleted_or_duplicate_review_fails_closed(self) -> None:
        stale = _fact("DISC_00000000000000000003", "deleted_test")
        with self.assertRaisesRegex(ValueError, "absent from live scanner facts"):
            self.analyze(
                [
                    _mapped_review(self.mapped),
                    _nonmapping_review(self.unmapped),
                    _nonmapping_review(stale),
                ]
            )
        with self.assertRaisesRegex(ValueError, "duplicate review"):
            self.analyze(
                [
                    _mapped_review(self.mapped),
                    _nonmapping_review(self.unmapped),
                    _nonmapping_review(self.unmapped),
                ]
            )

    def test_live_identity_binding_rejects_rename_and_ignore_drift(self) -> None:
        renamed = _nonmapping_review(self.unmapped)
        renamed["name"] = "invented_name"
        with self.assertRaisesRegex(ValueError, "name does not match"):
            self.analyze([_mapped_review(self.mapped), renamed])

        ignore_drift = _nonmapping_review(self.unmapped)
        ignore_drift["ignore_state"] = "ignored"
        with self.assertRaisesRegex(ValueError, "ignore_state does not match"):
            self.analyze([_mapped_review(self.mapped), ignore_drift])

    def test_catalog_mapping_cannot_be_downgraded_or_rebound(self) -> None:
        with self.assertRaisesRegex(ValueError, "catalog-mapped fact must use"):
            self.analyze(
                [_nonmapping_review(self.mapped), _nonmapping_review(self.unmapped)]
            )
        rebound = _mapped_review(self.mapped)
        rebound["catalog_test_id"] = "TEST_INVENTED"
        with self.assertRaisesRegex(ValueError, "catalog_test_id does not match"):
            self.analyze([rebound, _nonmapping_review(self.unmapped)])

    def test_nonmapping_cannot_claim_catalog_mapping_fields(self) -> None:
        row = _nonmapping_review(self.unmapped)
        row["catalog_test_id"] = "TEST_EXAMPLE_001"
        with self.assertRaisesRegex(ValueError, "forbids catalog_test_id"):
            self.analyze([_mapped_review(self.mapped), row])

    def test_reason_vocabulary_and_canonical_order_are_closed(self) -> None:
        row = _nonmapping_review(self.unmapped)
        row["reason_code"] = "name_did_not_match"
        with self.assertRaisesRegex(ValueError, "unsupported reason_code"):
            self.analyze([_mapped_review(self.mapped), row])
        with self.assertRaisesRegex(ValueError, "canonical discovery_id order"):
            self.analyze(
                [_nonmapping_review(self.unmapped), _mapped_review(self.mapped)]
            )

    def test_reviewed_oracle_limitations_are_explicit_owned_nonmappings(self) -> None:
        for reason in (
            "assertion_oracle_unresolved",
            "not_catalogable_product_oracle",
        ):
            with self.subTest(reason=reason):
                result = self.analyze(
                    [
                        _mapped_review(self.mapped),
                        _nonmapping_review(self.unmapped, reason),
                    ]
                )
                self.assertEqual(
                    1,
                    result["summary"]["by_nonmapping_reason"][reason],
                )

    def test_ignored_nonmapping_requires_exact_ignored_registry_join(self) -> None:
        ignored = _fact(
            "DISC_00000000000000000003",
            "ignored_test",
            ignore_state="ignored",
        )
        reviews = [
            _mapped_review(self.mapped),
            _nonmapping_review(self.unmapped),
            _nonmapping_review(ignored, "ignored_test_registry_owned"),
        ]
        with self.assertRaisesRegex(ValueError, "ignored registry"):
            self.analyze(reviews, facts=[self.mapped, self.unmapped, ignored])

        result = self.analyze(
            reviews,
            facts=[self.mapped, self.unmapped, ignored],
            ignored_tests=[
                {
                    "id": "IGNORED_EXAMPLE",
                    "discovery_id": ignored.stable_id,
                    "ignore_state": "ignored",
                }
            ],
        )
        self.assertEqual(1, result["summary"]["ignored_registry_owned_facts"])

    def test_dedicated_plane_reasons_require_the_matching_source_kind(self) -> None:
        fuzz = _fact(
            "DISC_00000000000000000003",
            "fuzz_target",
            source_kind="fuzz_target",
            path="fuzz/fuzz_targets/example.rs",
        )
        reviews = [
            _mapped_review(self.mapped),
            _nonmapping_review(self.unmapped),
            _nonmapping_review(fuzz, "fuzz_program_owned"),
        ]
        self.analyze(reviews, facts=[self.mapped, self.unmapped, fuzz])

        invalid = copy.deepcopy(reviews)
        invalid[1]["reason_code"] = "fuzz_program_owned"
        with self.assertRaisesRegex(ValueError, "requires fuzz_target"):
            self.analyze(invalid, facts=[self.mapped, self.unmapped, fuzz])

    def test_closed_document_contract_accepts_only_the_reviewed_shape(self) -> None:
        document = {
            "schema_version": 1,
            "id": "TEST_CATALOG_DENOMINATOR_REVIEW_V1",
            "title": "Reviewed existing-test denominator",
            "denominator_basis": "all_live_existing_test_scanner_facts",
            "review_basis": "explicit_per_discovery_id_no_semantic_inference",
            "reason_definitions": dict(NONMAPPING_REASON_DEFINITIONS),
            "last_reviewed": "2026-07-18",
            "reviews": [_nonmapping_review(self.unmapped)],
        }
        schema = json.loads(
            Path("verification/schemas/test-catalog-denominator.schema.json").read_text()
        )
        self.assertEqual(
            [], validate_test_catalog_denominator_document(document, schema=schema)
        )

        document["reason_definitions"] = dict(NONMAPPING_REASON_DEFINITIONS)
        document["reason_definitions"]["no_reviewed_spec_or_invariant_binding"] = (
            "Names did not match."
        )
        failures = validate_test_catalog_denominator_document(document, schema=schema)
        self.assertTrue(any("reason definitions drift" in item for item in failures))


if __name__ == "__main__":
    unittest.main()
