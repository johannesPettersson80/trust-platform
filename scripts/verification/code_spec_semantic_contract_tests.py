"""Adversarial semantic fixtures for Phase 17 mapping dispositions."""

from __future__ import annotations

import unittest

from scripts.verification.code_spec_common import CodeFact, sha256_digest, stable_code_id
from scripts.verification.code_spec_contract import validate_map_document
from scripts.verification.code_spec_risk import risk_ack_id


def make_fact(
    symbol: str,
    *,
    owner: str = "trust-runtime",
    area: str = "runtime_safety",
    visibility: str = "private",
    risks: tuple[str, ...] = (),
) -> CodeFact:
    path = "crates/example/src/lib.rs"
    signature = f"fn {symbol}()"
    return CodeFact(
        code_id=stable_code_id(
            source_kind="rust",
            path=path,
            qualified_symbol=f"example::{symbol}",
            item_kind="rust_function",
            signature=signature,
        ),
        source_kind="rust",
        path=path,
        qualified_symbol=f"example::{symbol}",
        item_kind="rust_function",
        signature=signature,
        signature_digest=sha256_digest(signature.encode()),
        body_digest=sha256_digest(f"{signature} {{}}".encode()),
        owner=owner,
        area=area,
        visibility=visibility,
        risk_signals=risks,
        line=1,
    )


def base_mapping(code_fact: CodeFact, disposition: str, **fields: object) -> dict[str, object]:
    mapping: dict[str, object] = {
        "schema_version": 1,
        "code_id": code_fact.code_id,
        "source_kind": code_fact.source_kind,
        "path": code_fact.path,
        "qualified_symbol": code_fact.qualified_symbol,
        "item_kind": code_fact.item_kind,
        "signature_digest": code_fact.signature_digest,
        "reviewed_body_digest": code_fact.body_digest,
        "owner": code_fact.owner,
        "area": code_fact.area,
        "visibility": code_fact.visibility,
        "disposition": disposition,
        "risk_signals": list(code_fact.risk_signals),
        "last_reviewed": "2026-07-21",
    }
    mapping.update(fields)
    return mapping


def validate(
    facts: list[CodeFact],
    mappings: list[dict[str, object]],
    **metadata: object,
) -> list[str]:
    return validate_map_document(
        {"schema_version": 1, "mappings": mappings},
        live_facts={item.code_id: item for item in facts},
        **metadata,
    )


class SpecifiedMappingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.code = make_fact("run")
        self.specs = {
            "SPEC_RUNTIME": {
                "source_status": "active",
                "oracle_eligible": True,
                "authority": "normative_product",
                "locator_kind": "tracked_file",
                "path": "docs/specs/11-runtime-engine.md",
            }
        }
        self.invariants = {
            "INV_RUNTIME": {
                "area": "runtime_safety",
                "spec": {"source_refs": ["SPEC_RUNTIME"]},
                "oracle": {"ref": "SPEC_RUNTIME"},
                "tests": ["TEST_RUNTIME"],
            }
        }
        self.tests = {
            "TEST_RUNTIME": {
                "area": "runtime_safety",
                "status": "mapped",
                "invariants": ["INV_RUNTIME"],
                "suite_tiers": ["pr"],
            }
        }

    def mapping(self) -> dict[str, object]:
        return base_mapping(
            self.code,
            "specified",
            spec_ref="SPEC_RUNTIME#1-purpose",
            invariant_ids=["INV_RUNTIME"],
            test_ids=["TEST_RUNTIME"],
        )

    def test_exact_bidirectional_join_passes(self) -> None:
        self.assertEqual(
            [],
            validate(
                [self.code],
                [self.mapping()],
                spec_sources=self.specs,
                invariants=self.invariants,
                tests=self.tests,
            ),
        )

    def test_multi_invariant_mapping_requires_bipartite_not_cartesian_links(self) -> None:
        mapping = self.mapping()
        mapping["invariant_ids"] = ["INV_COMMAND", "INV_OUTPUT"]
        mapping["test_ids"] = ["TEST_COMMAND", "TEST_OUTPUT"]
        invariants = {
            "INV_COMMAND": {
                "area": "runtime_safety",
                "spec": {"source_refs": ["SPEC_RUNTIME"]},
                "oracle": {"ref": "SPEC_RUNTIME"},
                "tests": ["TEST_COMMAND"],
            },
            "INV_OUTPUT": {
                "area": "runtime_safety",
                "spec": {"source_refs": ["SPEC_RUNTIME"]},
                "oracle": {"ref": "SPEC_RUNTIME"},
                "tests": ["TEST_OUTPUT"],
            },
        }
        tests = {
            "TEST_COMMAND": {
                "area": "runtime_safety",
                "status": "mapped",
                "invariants": ["INV_COMMAND"],
                "suite_tiers": ["pr"],
            },
            "TEST_OUTPUT": {
                "area": "runtime_safety",
                "status": "mapped",
                "invariants": ["INV_OUTPUT"],
                "suite_tiers": ["pr"],
            },
        }
        self.assertEqual(
            [],
            validate(
                [self.code],
                [mapping],
                spec_sources=self.specs,
                invariants=invariants,
                tests=tests,
            ),
        )

    def test_inactive_or_public_claim_source_is_rejected(self) -> None:
        for source in (
            {"source_status": "stale", "oracle_eligible": True, "authority": "normative_product"},
            {"source_status": "active", "oracle_eligible": False, "authority": "normative_product"},
            {"source_status": "active", "oracle_eligible": True, "authority": "public_claim"},
        ):
            failures = validate(
                [self.code],
                [self.mapping()],
                spec_sources={"SPEC_RUNTIME": source},
                invariants=self.invariants,
                tests=self.tests,
            )
            self.assertTrue(any("not active and oracle-eligible" in item for item in failures), failures)

    def test_missing_or_untracked_spec_fragment_is_rejected(self) -> None:
        for label, source, spec_ref in (
            ("missing", self.specs["SPEC_RUNTIME"], "SPEC_RUNTIME#invented-fragment"),
            (
                "untracked",
                dict(self.specs["SPEC_RUNTIME"], locator_kind="url"),
                "SPEC_RUNTIME#1-purpose",
            ),
            (
                "escaping",
                dict(self.specs["SPEC_RUNTIME"], path="../outside.md"),
                "SPEC_RUNTIME#1-purpose",
            ),
        ):
            with self.subTest(label=label):
                mapping = self.mapping()
                mapping["spec_ref"] = spec_ref
                failures = validate(
                    [self.code],
                    [mapping],
                    spec_sources={"SPEC_RUNTIME": source},
                    invariants=self.invariants,
                    tests=self.tests,
                )
                self.assertTrue(
                    any(
                        "specified spec_ref fragment does not exist" in item
                        for item in failures
                    ),
                    failures,
                )

    def test_invariant_and_test_must_link_back_to_same_contract(self) -> None:
        invariant = dict(
            self.invariants["INV_RUNTIME"],
            spec={"source_refs": ["SPEC_OTHER"]},
            oracle={"ref": "SPEC_OTHER"},
            tests=[],
        )
        test = dict(self.tests["TEST_RUNTIME"], invariants=["INV_OTHER"])
        failures = validate(
            [self.code],
            [self.mapping()],
            spec_sources=self.specs,
            invariants={"INV_RUNTIME": invariant},
            tests={"TEST_RUNTIME": test},
        )
        self.assertTrue(any("does not reference SPEC_RUNTIME" in item for item in failures), failures)
        self.assertTrue(any("does not link a mapped invariant" in item for item in failures), failures)
        self.assertTrue(any("does not link any mapped test" in item for item in failures), failures)


class GapAndInheritanceTests(unittest.TestCase):
    def test_unresolved_owning_gap_accepts_and_closed_gap_rejects(self) -> None:
        code = make_fact("gap")
        mapping = base_mapping(code, "spec_gap", spec_gap_ref="GAP_RUNTIME")
        open_gap = {
            "area": "runtime_safety",
            "resolution_status": "open",
            "affected_code_ids": [code.code_id],
        }
        self.assertEqual([], validate([code], [mapping], spec_gaps={"GAP_RUNTIME": open_gap}))
        backlog = dict(open_gap, resolution_status="registered_backlog")
        self.assertEqual(
            [], validate([code], [mapping], spec_gaps={"GAP_RUNTIME": backlog})
        )
        closed = dict(open_gap, resolution_status="closed")
        failures = validate([code], [mapping], spec_gaps={"GAP_RUNTIME": closed})
        self.assertTrue(
            any("requires an unresolved owning gap" in item for item in failures),
            failures,
        )
        wrong_owner = dict(open_gap, affected_code_ids=[])
        failures = validate([code], [mapping], spec_gaps={"GAP_RUNTIME": wrong_owner})
        self.assertTrue(any("does not name affected code identity" in item for item in failures), failures)

    def test_private_inheritance_terminates_at_specified_parent(self) -> None:
        parent = make_fact("parent")
        child = make_fact("child")
        mappings = [
            base_mapping(
                parent,
                "specified",
                spec_ref="SPEC_RUNTIME#1-purpose",
                invariant_ids=["INV"],
                test_ids=["TEST"],
            ),
            base_mapping(child, "inherits_spec", parent_code_id=parent.code_id),
        ]
        specs = {
            "SPEC_RUNTIME": {
                "source_status": "active",
                "oracle_eligible": True,
                "authority": "normative_product",
                "locator_kind": "tracked_file",
                "path": "docs/specs/11-runtime-engine.md",
            }
        }
        invariants = {
            "INV": {
                "area": "runtime_safety",
                "spec": {"source_refs": ["SPEC_RUNTIME"]},
                "oracle": {"ref": "SPEC_RUNTIME"},
                "tests": ["TEST"],
            }
        }
        tests = {"TEST": {"status": "mapped", "invariants": ["INV"], "suite_tiers": ["pr"]}}
        self.assertEqual(
            [],
            validate(
                [parent, child],
                mappings,
                spec_sources=specs,
                invariants=invariants,
                tests=tests,
            ),
        )

    def test_public_cross_owner_missing_parent_and_cycles_fail(self) -> None:
        first = make_fact("first", visibility="public")
        second = make_fact("second", owner="trust-ide")
        mappings = [
            base_mapping(first, "inherits_spec", parent_code_id=second.code_id),
            base_mapping(second, "inherits_spec", parent_code_id=first.code_id),
        ]
        failures = validate([first, second], mappings)
        self.assertTrue(any("only private facts" in item for item in failures), failures)
        self.assertTrue(any("cross-owner or cross-area" in item for item in failures), failures)
        self.assertTrue(any("inheritance cycle" in item for item in failures), failures)
        missing = make_fact("missing")
        failures = validate(
            [missing],
            [base_mapping(missing, "inherits_spec", parent_code_id="CODE_" + "A" * 64)],
        )
        self.assertTrue(any("not a live mapped fact" in item for item in failures), failures)


class DispositionGuardTests(unittest.TestCase):
    def test_risk_fact_cannot_be_non_behavioral_and_requires_owner_ack(self) -> None:
        code = make_fact("risky", risks=("host_time",))
        mapping = base_mapping(
            code,
            "non_behavioral",
            non_behavioral_reason="mechanical_adapter",
            rationale="Incorrect suppression fixture.",
        )
        failures = validate([code], [mapping])
        self.assertTrue(any("requires owner_ack" in item for item in failures), failures)
        self.assertTrue(any("cannot be non_behavioral" in item for item in failures), failures)

    def test_high_risk_owner_ack_requires_a_valid_unexpired_date(self) -> None:
        code = make_fact("risky_gap", risks=("host_time",))
        gap = {
            "area": "runtime_safety",
            "resolution_status": "open",
            "affected_code_ids": [code.code_id],
        }
        mapping = base_mapping(
            code,
            "spec_gap",
            spec_gap_ref="GAP_RUNTIME",
            owner_ack="trust-runtime reviewed this exact digest",
        )

        missing_expiry = validate(
            [code], [mapping], spec_gaps={"GAP_RUNTIME": gap}
        )
        self.assertTrue(
            any("requires owner_ack_expires_on" in item for item in missing_expiry),
            missing_expiry,
        )

        mapping["owner_ack_expires_on"] = "2000-01-01"
        expired = validate([code], [mapping], spec_gaps={"GAP_RUNTIME": gap})
        self.assertTrue(any("owner acknowledgement expired" in item for item in expired), expired)

    def test_high_risk_owner_ack_rejects_free_form_unresolvable_text(self) -> None:
        code = make_fact("risky_free_form_ack", risks=("host_time",))
        gap = {
            "area": "runtime_safety",
            "resolution_status": "open",
            "affected_code_ids": [code.code_id],
        }
        mapping = base_mapping(
            code,
            "spec_gap",
            spec_gap_ref="GAP_RUNTIME",
            owner_ack="trust-runtime reviewed this exact digest",
            owner_ack_expires_on="2099-01-01",
        )

        failures = validate([code], [mapping], spec_gaps={"GAP_RUNTIME": gap})

        self.assertTrue(
            any("owner_ack must resolve to an anchored risk acknowledgement" in item for item in failures),
            failures,
        )

    def test_high_risk_owner_ack_rejects_unknown_anchored_record(self) -> None:
        code = make_fact("risky_unknown_ack", risks=("host_time",))
        gap = {
            "area": "runtime_safety",
            "resolution_status": "open",
            "affected_code_ids": [code.code_id],
        }
        mapping = base_mapping(
            code,
            "spec_gap",
            spec_gap_ref="GAP_RUNTIME",
            owner_ack=risk_ack_id(code.code_id),
            owner_ack_expires_on="2099-01-01",
        )

        failures = validate([code], [mapping], spec_gaps={"GAP_RUNTIME": gap})

        self.assertTrue(any("unknown risk acknowledgement" in item for item in failures), failures)

    def test_high_risk_owner_ack_is_bound_to_exact_fact_and_mapping(self) -> None:
        code = make_fact("risky_stale_ack", risks=("host_time", "fault_mapping"))
        acknowledgement_id = risk_ack_id(code.code_id)
        gap = {
            "area": "runtime_safety",
            "resolution_status": "open",
            "affected_code_ids": [code.code_id],
        }
        mapping = base_mapping(
            code,
            "spec_gap",
            spec_gap_ref="GAP_RUNTIME",
            owner_ack=acknowledgement_id,
            owner_ack_expires_on="2099-01-01",
        )
        stale_acknowledgement = {
            "schema_version": 1,
            "id": acknowledgement_id,
            "code_id": code.code_id,
            "owner": "wrong-owner",
            "area": code.area,
            "body_digest": "sha256:" + "0" * 64,
            "risk_signals": ["host_time"],
            "review_ref": "docs/review.md#exact-symbol",
            "reviewed_by": code.owner,
            "reviewed_on": "2026-07-21",
            "expires_on": "2099-01-01",
            "rationale": "Exact fixture acknowledgement.",
        }

        failures = validate(
            [code],
            [mapping],
            spec_gaps={"GAP_RUNTIME": gap},
            risk_acknowledgements={acknowledgement_id: stale_acknowledgement},
        )

        for expected in (
            "risk acknowledgement owner changed",
            "risk acknowledgement body digest changed",
            "risk acknowledgement risk signals changed",
        ):
            self.assertTrue(any(expected in item for item in failures), (expected, failures))

    def test_generated_requires_governed_generator_and_vendored_requires_provenance(self) -> None:
        generated = make_fact("generated")
        generated_mapping = base_mapping(
            generated,
            "generated",
            generator_input_digest=sha256_digest(b"input"),
        )
        failures = validate([generated], [generated_mapping])
        self.assertTrue(any("lacks governed generator" in item for item in failures), failures)

        vendored = make_fact("vendored")
        failures = validate([vendored], [base_mapping(vendored, "vendored_external")])
        for field in ("provenance", "version_revision", "boundary_spec_ref"):
            self.assertTrue(any(f"requires {field}" in item for item in failures), failures)

    def test_generated_input_digest_must_match_live_generator_body(self) -> None:
        generator = make_fact("generator")
        generated = make_fact("generated_asset")
        mappings = [
            base_mapping(
                generator,
                "non_behavioral",
                non_behavioral_reason="build_scaffolding",
                rationale="Exact bundle configuration input.",
            ),
            base_mapping(
                generated,
                "generated",
                generator_code_id=generator.code_id,
                generator_input_digest=sha256_digest(b"wrong input"),
            ),
        ]

        failures = validate([generator, generated], mappings)

        self.assertTrue(
            any("generator_input_digest does not match live generator" in item for item in failures),
            failures,
        )

    def test_vendored_boundary_must_resolve_to_active_oracle_spec_fragment(self) -> None:
        vendored = make_fact("vendored_boundary")
        mapping = base_mapping(
            vendored,
            "vendored_external",
            provenance="upstream package lock",
            version_revision="1.2.3",
            boundary_spec_ref="SPEC_WEB_IDE#local-assets",
        )

        failures = validate(
            [vendored],
            [mapping],
            spec_sources={
                "SPEC_WEB_IDE": {
                    "source_status": "stale",
                    "oracle_eligible": True,
                    "authority": "normative_product",
                }
            },
        )

        self.assertTrue(
            any("vendored boundary spec is not active and oracle-eligible" in item for item in failures),
            failures,
        )

        mapping["boundary_spec_ref"] = "SPEC_WEB_IDE#invented-fragment"
        failures = validate(
            [vendored],
            [mapping],
            spec_sources={
                "SPEC_WEB_IDE": {
                    "source_status": "active",
                    "oracle_eligible": True,
                    "authority": "normative_product",
                    "locator_kind": "tracked_file",
                    "path": "docs/internal/runtime/trust-runtime-web-ide-full-specification.md",
                }
            },
        )
        self.assertTrue(
            any("vendored boundary spec fragment does not exist" in item for item in failures),
            failures,
        )

    def test_duplicate_and_invented_mappings_fail(self) -> None:
        code = make_fact("duplicate")
        mapping = base_mapping(
            code,
            "non_behavioral",
            non_behavioral_reason="mechanical_adapter",
            rationale="Fixture.",
        )
        failures = validate([code], [mapping, mapping])
        self.assertTrue(any("duplicate mapping" in item for item in failures), failures)
        invented = dict(mapping, code_id="CODE_" + "F" * 64)
        failures = validate([code], [mapping, invented])
        self.assertTrue(any("invented_or_stale_code_identity" in item for item in failures), failures)


if __name__ == "__main__":
    unittest.main()
