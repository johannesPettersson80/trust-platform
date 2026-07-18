"""Tests for the exhaustive Phase 8 runtime-safety denominator review."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.runtime_anomaly_denominator import (
    NONMAPPING_REASON_CODES,
    NONMAPPING_REASON_DEFINITIONS,
    analyze_runtime_anomaly_denominator,
    validate_runtime_anomaly_denominator_document,
)
from scripts.verification.test_catalog_models import InferredTestFact


def _fact(stable_id: str, name: str, path: str = "crates/trust-runtime/tests/x.rs") -> InferredTestFact:
    return InferredTestFact(
        stable_id=stable_id,
        native_id=name,
        source_kind="rust_integration_test",
        name=name,
        path=path,
        line=10,
        package="trust-runtime",
        command_hint=f"cargo test -p trust-runtime --test x {name}",
        command_hint_authority="conservative",
        discovery_confidence="exact_attribute",
        ignore_state="not_ignored",
        ignore_reason=None,
        reference_candidates=(),
    )


def _mapped_review(fact: InferredTestFact) -> dict[str, object]:
    return {
        "discovery_id": fact.stable_id,
        "discovery_source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
        "disposition": "mapped",
        "mapping_id": "ANOM_MAP_PANIC_001",
        "last_reviewed": "2026-07-18",
    }


def _nonmapping_review(fact: InferredTestFact) -> dict[str, object]:
    return {
        "discovery_id": fact.stable_id,
        "discovery_source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
        "disposition": "reviewed_nonmapping",
        "reason_code": "no_taxonomy_stimulus_or_response",
        "last_reviewed": "2026-07-18",
    }


class RuntimeAnomalyDenominatorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.mapped = _fact("DISC_00000000000000000001", "panic_is_contained")
        self.nonmapping = _fact("DISC_00000000000000000002", "ordinary_addition")
        self.mapping = {
            "id": "ANOM_MAP_PANIC_001",
            "discovery_id": self.mapped.stable_id,
        }

    def analyze(self, reviews: list[dict[str, object]], facts=None, mappings=None):
        return analyze_runtime_anomaly_denominator(
            facts=facts or [self.mapped, self.nonmapping],
            mappings=mappings or [self.mapping],
            reviews=reviews,
        )

    def test_complete_partition_reports_mapped_and_reviewed_nonmapping_counts(self) -> None:
        result = self.analyze(
            [_mapped_review(self.mapped), _nonmapping_review(self.nonmapping)]
        )
        self.assertEqual(
            result["summary"],
            {
                "scanner_denominator": 2,
                "mapped_facts": 1,
                "reviewed_nonmapping_facts": 1,
                "unreviewed_facts": 0,
                "exhaustive": True,
                "by_nonmapping_reason": {
                    code: int(code == "no_taxonomy_stimulus_or_response")
                    for code in NONMAPPING_REASON_CODES
                },
            },
        )

    def test_new_scanner_fact_blocks_exhaustiveness_until_reviewed(self) -> None:
        added = _fact("DISC_00000000000000000003", "new_test")
        with self.assertRaisesRegex(ValueError, "unreviewed live scanner facts"):
            self.analyze(
                [_mapped_review(self.mapped), _nonmapping_review(self.nonmapping)],
                facts=[self.mapped, self.nonmapping, added],
            )

    def test_stale_review_is_rejected(self) -> None:
        stale = _fact("DISC_00000000000000000003", "deleted_test")
        with self.assertRaisesRegex(ValueError, "absent from live scanner facts"):
            self.analyze(
                [
                    _mapped_review(self.mapped),
                    _nonmapping_review(self.nonmapping),
                    _nonmapping_review(stale),
                ]
            )

    def test_duplicate_review_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "duplicate review"):
            self.analyze(
                [
                    _mapped_review(self.mapped),
                    _nonmapping_review(self.nonmapping),
                    _nonmapping_review(self.nonmapping),
                ]
            )

    def test_live_fact_binding_is_exact_but_line_number_is_not_stored(self) -> None:
        review = _nonmapping_review(self.nonmapping)
        review["name"] = "renamed"
        with self.assertRaisesRegex(ValueError, "name does not match"):
            self.analyze([_mapped_review(self.mapped), review])

    def test_mapped_fact_requires_the_exact_existing_mapping(self) -> None:
        review = _mapped_review(self.mapped)
        review["mapping_id"] = "ANOM_MAP_PANIC_999"
        with self.assertRaisesRegex(ValueError, "mapping_id does not match"):
            self.analyze([review, _nonmapping_review(self.nonmapping)])

    def test_existing_mapping_cannot_be_relabeled_nonmapping(self) -> None:
        with self.assertRaisesRegex(ValueError, "mapped scanner fact must use mapped"):
            self.analyze(
                [_nonmapping_review(self.mapped), _nonmapping_review(self.nonmapping)]
            )

    def test_nonmapping_fact_cannot_claim_mapping_fields(self) -> None:
        review = _nonmapping_review(self.nonmapping)
        review["mapping_id"] = "ANOM_MAP_PANIC_001"
        with self.assertRaisesRegex(ValueError, "forbids mapping_id"):
            self.analyze([_mapped_review(self.mapped), review])

    def test_nonmapping_reason_vocabulary_is_closed(self) -> None:
        review = _nonmapping_review(self.nonmapping)
        review["reason_code"] = "name_did_not_look_relevant"
        with self.assertRaisesRegex(ValueError, "unsupported reason_code"):
            self.analyze([_mapped_review(self.mapped), review])

    def test_review_order_must_be_canonical(self) -> None:
        with self.assertRaisesRegex(ValueError, "canonical discovery_id order"):
            self.analyze(
                [_nonmapping_review(self.nonmapping), _mapped_review(self.mapped)]
            )

    def test_paths_and_names_do_not_auto_disposition_an_unreviewed_fact(self) -> None:
        obvious_name = _fact(
            "DISC_00000000000000000003",
            "panic_timeout_watchdog_corrupt_retain",
            "crates/trust-runtime/tests/runtime_safety_fail_closed.rs",
        )
        with self.assertRaisesRegex(ValueError, "unreviewed live scanner facts"):
            self.analyze(
                [_mapped_review(self.mapped), _nonmapping_review(self.nonmapping)],
                facts=[self.mapped, self.nonmapping, obvious_name],
            )

    def test_closed_document_contract_accepts_exact_review_shape(self) -> None:
        document = {
            "schema_version": 1,
            "id": "RUNTIME_ANOMALY_DENOMINATOR_REVIEW_V1",
            "title": "Reviewed denominator",
            "area": "runtime_safety",
            "denominator_basis": "all_live_production_rust_test_facts",
            "review_basis": "explicit_per_discovery_id_no_lexical_inference",
            "reason_definitions": dict(NONMAPPING_REASON_DEFINITIONS),
            "last_reviewed": "2026-07-18",
            "reviews": [_nonmapping_review(self.nonmapping)],
        }
        self.assertEqual(
            validate_runtime_anomaly_denominator_document(Path.cwd(), document), []
        )

    def test_document_contract_rejects_reason_definition_drift(self) -> None:
        definitions = dict(NONMAPPING_REASON_DEFINITIONS)
        definitions["no_taxonomy_stimulus_or_response"] = "Names did not match."
        document = {
            "schema_version": 1,
            "id": "RUNTIME_ANOMALY_DENOMINATOR_REVIEW_V1",
            "title": "Reviewed denominator",
            "area": "runtime_safety",
            "denominator_basis": "all_live_production_rust_test_facts",
            "review_basis": "explicit_per_discovery_id_no_lexical_inference",
            "reason_definitions": definitions,
            "last_reviewed": "2026-07-18",
            "reviews": [_nonmapping_review(self.nonmapping)],
        }
        failures = validate_runtime_anomaly_denominator_document(Path.cwd(), document)
        self.assertTrue(any("reason definitions drift" in item for item in failures))


if __name__ == "__main__":
    unittest.main()
