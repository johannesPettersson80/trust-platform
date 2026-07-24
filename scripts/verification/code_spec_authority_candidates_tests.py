"""Focused tests for ID-free code-spec authority candidate compilation."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification.code_spec_authority_candidates import (
    compile_authority_candidate_proposal,
    write_authority_candidate_card,
    write_authority_candidate_human_template,
)
from scripts.verification.code_spec_live import CodeSpecMetadata


COMMIT = "a" * 40
CODE_A = "CODE_" + "A" * 64
CODE_B = "CODE_" + "B" * 64
CODE_SPECIFIED = "CODE_" + "C" * 64
DISC_A = "DISC_" + "D" * 20
DISC_B = "DISC_" + "E" * 20


def _mapping(code_id: str, area: str, disposition: str) -> dict:
    return {
        "code_id": code_id,
        "area": area,
        "disposition": disposition,
        "source_kind": "rust",
        "path": "crates/example/src/lib.rs",
        "qualified_symbol": f"example::{code_id[-4:]}",
        "item_kind": "rust_function",
        "owner": "example",
        "risk_signals": ["state_mutation"],
    }


def _invariant(
    invariant_id: str, area: str, source_id: str, spec_ref: str
) -> dict:
    return {
        "id": invariant_id,
        "title": f"Title for {invariant_id}",
        "claim": f"Claim for {invariant_id}",
        "risk": "safety_critical" if invariant_id == "INV_A1" else "wrong_result",
        "area": area,
        "tests": [],
        "spec": {
            "status": "specified",
            "source_refs": [source_id],
        },
        "oracle": {"kind": "trust_contract", "ref": spec_ref},
        "behavior": [
            {
                "partition": {"equals": "EXACT"},
                "case_family": "happy_path",
                "oracle_ref": spec_ref,
                "outcome": "accept_value",
                "rationale": f"Behavior for {invariant_id}",
            }
        ],
    }


class CodeSpecAuthorityCandidatesTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.quarantine = (
            self.root / "target/gate-artifacts/authority-candidates"
        )
        self.quarantine.mkdir(parents=True)
        verification = self.root / "verification"
        invariant_root = verification / "invariants/example"
        invariant_root.mkdir(parents=True)
        (verification / "code-spec-map.toml").write_text(
            "schema_version = 1\nmappings = []\n"
        )
        (verification / "spec-sources.toml").write_text(
            "schema_version = 1\nspec_sources = []\n"
        )
        for name in ("INV_A1", "INV_A2", "INV_A_OTHER", "INV_B", "INV_C"):
            (invariant_root / f"{name}.toml").write_text(
                f'id = "{name}"\n'
            )
        self.trace_path = self.quarantine / "trace.json"
        self.card_path = self.quarantine / "card.json"
        self.human_path = self.quarantine / "human.json"
        self.proposal_path = self.quarantine / "proposal.json"
        self.trace = {
            "source_commit": COMMIT,
            "scope": {
                "mode": "focused_shard",
                "requested_discovery_ids": [DISC_A, DISC_B],
                "source_kinds": ["rust_unit_test"],
            },
            "tests": [
                {
                    "discovery_id": DISC_A,
                    "path": "crates/example/src/lib.rs",
                    "name": "case_a",
                    "trace_status": "observed",
                    "covered_code_ids": [CODE_A, CODE_B, CODE_SPECIFIED],
                },
                {
                    "discovery_id": DISC_B,
                    "path": "crates/example/src/lib.rs",
                    "name": "case_b",
                    "trace_status": "observed",
                    "covered_code_ids": [CODE_A],
                },
            ],
        }
        self.trace_path.write_text(json.dumps(self.trace))
        self.metadata = CodeSpecMetadata(
            mappings={
                CODE_A: _mapping(CODE_A, "area_a", "spec_gap"),
                CODE_B: _mapping(CODE_B, "area_b", "spec_gap"),
                CODE_SPECIFIED: _mapping(
                    CODE_SPECIFIED, "area_a", "specified"
                ),
            },
            invariants={
                "INV_A1": _invariant(
                    "INV_A1", "area_a", "SPEC_A", "SPEC_A#exact"
                ),
                "INV_A2": _invariant(
                    "INV_A2", "area_a", "SPEC_A", "SPEC_A#exact"
                ),
                "INV_A_OTHER": _invariant(
                    "INV_A_OTHER", "area_a", "SPEC_A", "SPEC_A#other"
                ),
                "INV_B": _invariant(
                    "INV_B", "area_b", "SPEC_B", "SPEC_B#exact"
                ),
                "INV_C": _invariant(
                    "INV_C", "area_c", "SPEC_A", "SPEC_A#exact"
                ),
            },
            spec_sources={
                "SPEC_A": {
                    "source_status": "active",
                    "oracle_eligible": True,
                    "authority": "normative_product",
                },
                "SPEC_B": {
                    "source_status": "active",
                    "oracle_eligible": True,
                    "authority": "reviewed_decision",
                },
            },
            tests={},
        )
        patchers = (
            patch(
                "scripts.verification.code_spec_authority_candidates."
                "load_code_spec_metadata",
                return_value=self.metadata,
            ),
            patch(
                "scripts.verification.code_spec_authority_candidates."
                "build_focused_mapping_context",
                return_value=object(),
            ),
            patch(
                "scripts.verification.code_spec_authority_candidates."
                "validate_focused_rust_trace"
            ),
            patch(
                "scripts.verification.code_spec_authority_candidates._git_head",
                return_value=COMMIT,
            ),
            patch(
                "scripts.verification.code_spec_authority_candidates."
                "_commit_date",
                return_value="2026-07-24",
            ),
        )
        self.patchers = patchers
        for patcher in patchers:
            patcher.start()

    def tearDown(self) -> None:
        for patcher in reversed(self.patchers):
            patcher.stop()
        self.temporary.cleanup()

    def _write_card_template(self) -> tuple[dict, dict]:
        card = write_authority_candidate_card(
            root=self.root,
            trace_path=self.trace_path,
            output_path=self.card_path,
        )
        template = write_authority_candidate_human_template(
            root=self.root,
            card_path=self.card_path,
            output_path=self.human_path,
        )
        return card, template

    def _write_human(self, selections: list[dict]) -> dict:
        human = json.loads(self.human_path.read_text())
        human["reviewer_ids"] = ["reviewer-a"]
        human["selections"] = selections
        self.human_path.write_text(json.dumps(human))
        return human

    def test_numbered_choices_compile_existing_proposal_contract(self) -> None:
        card, template = self._write_card_template()
        self.assertEqual([CODE_A, CODE_B], [
            row["code_id"] for row in card["code_facts"]
        ])
        self.assertEqual(
            "example::AAAA", card["code_facts"][0]["qualified_symbol"]
        )
        self.assertEqual(
            f"Claim for INV_A1", card["invariants"][0]["claim"]
        )
        self.assertEqual(
            "Behavior for INV_A1",
            card["invariants"][0]["behavior_summaries"][0]["rationale"],
        )
        self.assertNotIn(
            "INV_C", [row["invariant_id"] for row in card["invariants"]]
        )
        self.assertEqual([], template["reviewer_ids"])
        self.assertEqual([], template["selections"])
        self.assertNotIn("code_id", template)
        self.assertNotIn("trace_digest", template)
        self._write_human(
            [
                {
                    "code_indices": [1],
                    "discovery_indices": [1, 2],
                    "invariant_indices": [1, 2],
                }
            ]
        )

        proposal = compile_authority_candidate_proposal(
            root=self.root,
            card_path=self.card_path,
            human_selection_path=self.human_path,
            output_path=self.proposal_path,
        )

        self.assertEqual(1, proposal["schema_version"])
        self.assertEqual("2026-07-24", proposal["reviewed_date"])
        self.assertEqual(
            [
                {
                    "code_id": CODE_A,
                    "spec_ref": "SPEC_A#exact",
                    "invariant_ids": ["INV_A1", "INV_A2"],
                    "discovery_ids": [DISC_A, DISC_B],
                }
            ],
            proposal["rows"],
        )
        provenance = proposal["candidate_provenance"]
        self.assertEqual(["reviewer-a"], provenance["proposer_ids"])
        self.assertTrue(provenance["independent_authority_review_required"])
        self.assertEqual(proposal, json.loads(self.proposal_path.read_text()))

    def test_cross_area_and_shared_oracle_mismatches_reject(self) -> None:
        card, _ = self._write_card_template()
        indices = {
            row["invariant_id"]: row["invariant_index"]
            for row in card["invariants"]
        }
        cases = (
            (
                {
                    "code_indices": [1],
                    "discovery_indices": [1],
                    "invariant_indices": [indices["INV_B"]],
                },
                "areas differ",
            ),
            (
                {
                    "code_indices": [1],
                    "discovery_indices": [1],
                    "invariant_indices": [
                        indices["INV_A1"],
                        indices["INV_A_OTHER"],
                    ],
                },
                "cross area or oracle",
            ),
        )
        for selection, message in cases:
            with self.subTest(message=message):
                self._write_human([selection])
                with self.assertRaisesRegex(ValueError, message):
                    compile_authority_candidate_proposal(
                        root=self.root,
                        card_path=self.card_path,
                        human_selection_path=self.human_path,
                        output_path=self.proposal_path,
                    )

    def test_stale_sources_card_and_generated_binding_reject(self) -> None:
        self._write_card_template()
        invariant_path = (
            self.root / "verification/invariants/example/INV_A1.toml"
        )
        invariant_path.write_text('id = "INV_A1"\nchanged = true\n')
        with self.assertRaisesRegex(ValueError, "differs from rebuilt"):
            write_authority_candidate_human_template(
                root=self.root,
                card_path=self.card_path,
                output_path=self.human_path,
            )

        invariant_path.write_text('id = "INV_A1"\n')
        self._write_card_template()
        card = json.loads(self.card_path.read_text())
        card["code_facts"][0]["area"] = "forged"
        self.card_path.write_text(json.dumps(card))
        with self.assertRaisesRegex(ValueError, "differs from rebuilt"):
            write_authority_candidate_human_template(
                root=self.root,
                card_path=self.card_path,
                output_path=self.human_path,
            )

        self._write_card_template()
        human = self._write_human(
            [
                {
                    "code_indices": [1],
                    "discovery_indices": [1],
                    "invariant_indices": [1],
                }
            ]
        )
        human["generated_binding"]["card_digest"] = "sha256:" + "0" * 64
        self.human_path.write_text(json.dumps(human))
        with self.assertRaisesRegex(ValueError, "binding is stale"):
            compile_authority_candidate_proposal(
                root=self.root,
                card_path=self.card_path,
                human_selection_path=self.human_path,
                output_path=self.proposal_path,
            )

    def test_stale_or_tampered_trace_rejects(self) -> None:
        self._write_card_template()
        changed_trace = dict(self.trace)
        changed_trace["tests"] = [dict(row) for row in self.trace["tests"]]
        changed_trace["tests"][0]["covered_code_ids"] = [CODE_A]
        self.trace_path.write_text(json.dumps(changed_trace))
        with self.assertRaisesRegex(ValueError, "differs from rebuilt"):
            write_authority_candidate_human_template(
                root=self.root,
                card_path=self.card_path,
                output_path=self.human_path,
            )

        changed_trace["source_commit"] = "b" * 40
        self.trace_path.write_text(json.dumps(changed_trace))
        with self.assertRaisesRegex(ValueError, "source commit is stale"):
            write_authority_candidate_card(
                root=self.root,
                trace_path=self.trace_path,
                output_path=self.card_path,
            )

    def test_duplicate_and_out_of_range_numeric_indices_reject(self) -> None:
        self._write_card_template()
        for field, values in (
            ("code_indices", [1, 1]),
            ("discovery_indices", [99]),
            ("invariant_indices", [True]),
        ):
            with self.subTest(field=field):
                selection = {
                    "code_indices": [1],
                    "discovery_indices": [1],
                    "invariant_indices": [1],
                }
                selection[field] = values
                self._write_human([selection])
                with self.assertRaisesRegex(ValueError, "numeric indices"):
                    compile_authority_candidate_proposal(
                        root=self.root,
                        card_path=self.card_path,
                        human_selection_path=self.human_path,
                        output_path=self.proposal_path,
                    )

    def test_human_cannot_supply_ids_or_hashes(self) -> None:
        self._write_card_template()
        human = self._write_human(
            [
                {
                    "code_indices": [1],
                    "discovery_indices": [1],
                    "invariant_indices": [1],
                }
            ]
        )
        human["code_id"] = CODE_A
        self.human_path.write_text(json.dumps(human))
        with self.assertRaisesRegex(ValueError, "fields drift"):
            compile_authority_candidate_proposal(
                root=self.root,
                card_path=self.card_path,
                human_selection_path=self.human_path,
                output_path=self.proposal_path,
            )

    def test_trace_and_card_symlink_ancestors_reject(self) -> None:
        alias = self.root / "target/gate-artifacts/authority-alias"
        alias.symlink_to(self.quarantine, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            write_authority_candidate_card(
                root=self.root,
                trace_path=alias / "trace.json",
                output_path=self.card_path,
            )

        self._write_card_template()
        with self.assertRaisesRegex(ValueError, "symlink"):
            write_authority_candidate_human_template(
                root=self.root,
                card_path=alias / "card.json",
                output_path=self.human_path,
            )

    def test_real_wrapper_help_smoke(self) -> None:
        wrapper = (
            Path(__file__).resolve().parents[1]
            / "compile_code_spec_authority_candidates.py"
        )
        completed = subprocess.run(
            [sys.executable, str(wrapper), "--help"],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(0, completed.returncode, completed.stderr)
        self.assertIn("numbered authority candidates", completed.stdout)


if __name__ == "__main__":
    unittest.main()
