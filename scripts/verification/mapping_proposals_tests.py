"""Known-bad fixtures for the quarantined mapping-proposal plane."""

from __future__ import annotations

import copy
import hashlib
import tempfile
import unittest
from pathlib import Path

from scripts.verification.mapping_proposals import (
    build_mapping_proposals,
    validate_mapping_quarantine,
)


DISCOVERY_ID = "DISC_00000000000000000001"


def _fact() -> dict[str, object]:
    return {
        "discovery_id": DISCOVERY_ID,
        "discovery_source_kind": "rust_unit_test",
        "path": "crates/example/src/lib.rs",
        "name": "observes_contract",
        "ignore_state": "not_ignored",
    }


def _proposal() -> dict[str, object]:
    return {
        "schema_version": 1,
        "id": "MAP_CAND_00000000000000000001_A1B2C3D4",
        "proposal_digest": "sha256:" + "1" * 64,
        "discovery_id": DISCOVERY_ID,
        "discovery_source_kind": "rust_unit_test",
        "path": "crates/example/src/lib.rs",
        "name": "observes_contract",
        "ignore_state": "not_ignored",
        "command": "cargo nextest run -p example -E 'test(=observes_contract)'",
        "command_authority": "verified_synthesized",
        "duration_ms": 75,
        "duration_class": "very_fast",
        "status": "pending",
        "candidate_tier": "C2",
        "candidate_invariant_ids": ["INV_ONE"],
        "covered_code_ids": ["CODE_" + "A" * 64],
        "specified_code_ids": ["CODE_" + "A" * 64],
        "spec_gap_code_ids": [],
        "spec_gap_refs": [],
        "mutation_kill_ids": [],
        "test_source_digest": "sha256:" + "4" * 64,
        "assertion_inventory_digest": "sha256:" + "5" * 64,
        "assertions": [
            {
                "assertion_id": "ASSERT_" + "A" * 24,
                "kind": "macro:assert",
                "function": "observes_contract",
                "line": 10,
                "expression_digest": "sha256:" + "6" * 64,
                "expression": "assert ! (observed)",
            }
        ],
        "unresolved_assertion_helpers": [],
        "trace_record_digest": "sha256:" + "2" * 64,
        "candidate_signal_digest": "sha256:" + "3" * 64,
        "oracle_refs": ["SPEC_EXAMPLE_001#behavior"],
        "behavior_refs": ["INV_ONE#behavior:0"],
        "suggestions": {
            "authoritative": False,
            "test_class": "behavior_lock",
            "expected_result": "Copied candidate text.",
            "expected_failure_mode": "Copied candidate failure text.",
            "source_behavior_refs": ["INV_ONE#behavior:0"],
        },
    }


def _review(disposition: str = "reviewed_nonmapping") -> dict[str, object]:
    row = {
        **_fact(),
        "disposition": disposition,
        "last_reviewed": "2026-07-23",
    }
    if disposition == "reviewed_nonmapping":
        row["reason_code"] = "no_reviewed_spec_or_invariant_binding"
    else:
        row["catalog_test_id"] = "TEST_EXISTING"
    return row


class MappingProposalQuarantineTests(unittest.TestCase):
    def validate(
        self,
        proposal: dict[str, object],
        *,
        facts=None,
        tests=None,
        review=None,
        invariants=None,
    ) -> list[str]:
        return validate_mapping_quarantine(
            proposals=[proposal],
            facts=facts or {DISCOVERY_ID: _fact()},
            tests=tests or {},
            denominator_reviews={DISCOVERY_ID: review or _review()},
            invariants=invariants or {
                "INV_ONE": {
                    "status": "mapped",
                    "risk": "maintenance",
                    "oracle": {
                        "kind": "trust_contract",
                        "ref": "SPEC_EXAMPLE_001#behavior",
                    },
                    "behavior": [{}],
                }
            },
        )

    def test_valid_pending_proposal_remains_quarantined(self) -> None:
        self.assertEqual([], self.validate(_proposal()))

    def test_pending_proposal_cannot_already_be_catalog_mapped(self) -> None:
        failures = self.validate(
            _proposal(),
            tests={
                "TEST_EXISTING": {
                    "subject_kind": "generated_test",
                    "discovery_id": DISCOVERY_ID,
                }
            },
            review=_review("catalog_mapped"),
        )

        self.assertTrue(any("quarantine leakage" in item for item in failures))

    def test_pending_proposal_requires_reviewed_nonmapping_denominator(self) -> None:
        failures = self.validate(_proposal(), review=_review("catalog_mapped"))

        self.assertTrue(any("reviewed_nonmapping" in item for item in failures))

    def test_stale_live_identity_is_rejected(self) -> None:
        stale = _proposal()
        stale["name"] = "renamed_without_regeneration"

        failures = self.validate(stale)

        self.assertTrue(any("name does not match live fact" in item for item in failures))

    def test_stale_source_digest_is_rejected_before_review(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("fn changed_after_trace() {}\n")
            proposal = _proposal()
            proposal["test_source_digest"] = (
                "sha256:" + hashlib.sha256(b"fn original() {}\n").hexdigest()
            )

            failures = validate_mapping_quarantine(
                root=root,
                proposals=[proposal],
                facts={DISCOVERY_ID: _fact()},
                tests={},
                denominator_reviews={DISCOVERY_ID: _review()},
                invariants={
                    "INV_ONE": {
                        "status": "mapped",
                        "risk": "maintenance",
                        "oracle": {
                            "kind": "trust_contract",
                            "ref": "SPEC_EXAMPLE_001#behavior",
                        },
                        "behavior": [{}],
                    }
                },
            )

        self.assertTrue(any("source digest is stale" in item for item in failures))

    def test_unknown_or_unmapped_invariant_is_rejected(self) -> None:
        unknown = _proposal()
        unknown["candidate_invariant_ids"] = ["INV_UNKNOWN"]
        failures = self.validate(unknown)
        self.assertTrue(any("unknown invariant" in item for item in failures))

        failures = self.validate(
            _proposal(),
            invariants={
                "INV_ONE": {
                    "status": "spec_gap",
                    "risk": "maintenance",
                    "oracle": {
                        "kind": "trust_contract",
                        "ref": "SPEC_EXAMPLE_001#behavior",
                    },
                    "behavior": [{}],
                }
            },
        )
        self.assertTrue(any("semantically eligible" in item for item in failures))

    def test_suggestions_are_never_authoritative(self) -> None:
        proposal = copy.deepcopy(_proposal())
        proposal["suggestions"]["authoritative"] = True

        failures = self.validate(proposal)

        self.assertTrue(any("suggestions must remain non-authoritative" in item for item in failures))

    def test_behavior_and_oracle_references_must_resolve_exactly(self) -> None:
        proposal = _proposal()
        proposal["behavior_refs"] = ["INV_ONE#behavior:9"]
        proposal["oracle_refs"] = ["SPEC_WRONG_001#behavior"]

        failures = self.validate(proposal)

        self.assertTrue(any("behavior reference" in item for item in failures))
        self.assertTrue(any("oracle reference" in item for item in failures))

    def test_generator_creates_all_ids_and_digests_deterministically(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("#[test]\nfn observes_contract() {}\n")
            fact = _fact()
            invariant = {
                "status": "implemented",
                "risk": "safety_critical",
                "area": "runtime_safety",
                "owner": "trust-runtime",
                "oracle": {"ref": "SPEC_EXAMPLE_001#behavior"},
                "behavior": [
                    {
                        "case_family": "happy_path",
                        "outcome": "accept_value",
                        "rationale": "The exact value is preserved.",
                    }
                ],
            }
            candidate = {
                "discovery_id": DISCOVERY_ID,
                "candidate_tier": "C2",
                "candidate_invariant_ids": ["INV_ONE"],
                "covered_code_ids": ["CODE_" + "A" * 64],
                "specified_code_ids": ["CODE_" + "A" * 64],
                "spec_gap_code_ids": [],
                "spec_gap_refs": [],
                "mutation_kill_ids": [],
            }
            trace = {
                "discovery_id": DISCOVERY_ID,
                "trace_status": "observed",
                "command": "cargo nextest run -p example -E 'test(=observes_contract)'",
                "command_authority": "verified_synthesized",
                "duration_ms": 75,
                "covered_code_ids": ["CODE_" + "A" * 64],
            }
            assertion_inventory = {
                "inventory_digest": "sha256:" + "5" * 64,
                "assertions": [
                    {
                        "assertion_id": "ASSERT_" + "A" * 24,
                        "kind": "macro:assert",
                        "function": "observes_contract",
                        "line": 2,
                        "expression_digest": "sha256:" + "6" * 64,
                        "expression": "assert ! (true)",
                    }
                ],
                "unresolved_assertion_helpers": [],
            }

            first = build_mapping_proposals(
                root=root,
                candidates=[candidate],
                trace_records={DISCOVERY_ID: trace},
                facts={DISCOVERY_ID: fact},
                invariants={"INV_ONE": invariant},
                assertion_inventories={DISCOVERY_ID: assertion_inventory},
            )
            second = build_mapping_proposals(
                root=root,
                candidates=[candidate],
                trace_records={DISCOVERY_ID: trace},
                facts={DISCOVERY_ID: fact},
                invariants={"INV_ONE": invariant},
                assertion_inventories={DISCOVERY_ID: assertion_inventory},
            )

        self.assertEqual(first, second)
        proposal = first[0]
        self.assertRegex(proposal["id"], r"^MAP_CAND_[A-F0-9_]+$")
        for field in (
            "proposal_digest",
            "test_source_digest",
            "trace_record_digest",
            "candidate_signal_digest",
        ):
            self.assertRegex(proposal[field], r"^sha256:[0-9a-f]{64}$")
        self.assertEqual(["INV_ONE#behavior:0"], proposal["behavior_refs"])
        self.assertEqual(trace["command"], proposal["command"])
        self.assertEqual("verified_synthesized", proposal["command_authority"])
        self.assertEqual(75, proposal["duration_ms"])
        self.assertEqual("very_fast", proposal["duration_class"])
        self.assertFalse(proposal["suggestions"]["authoritative"])


if __name__ == "__main__":
    unittest.main()
