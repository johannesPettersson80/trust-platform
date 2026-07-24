"""Focused tests for ID-free code-spec authority review compilation."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.verification.code_spec_authority_review import (
    compile_authority_review_selection,
    write_authority_human_selection_template,
    write_authority_review_card,
)


CODE_ID = "CODE_" + "A" * 64
DISCOVERY_ID = "DISC_" + "B" * 20


def _candidate_provenance(*, high_risk: bool = False) -> dict:
    return {
        "compiler": "numbered_code_spec_authority_candidates_v1",
        "candidate_card_path": (
            "target/gate-artifacts/authority-review/candidate-card.json"
        ),
        "candidate_card_digest": "sha256:" + "1" * 64,
        "candidate_human_selection_path": (
            "target/gate-artifacts/authority-review/candidate-human.json"
        ),
        "candidate_human_selection_digest": "sha256:" + "2" * 64,
        "trace_path": (
            "target/gate-artifacts/authority-review/trace.json"
        ),
        "trace_digest": "sha256:" + "3" * 64,
        "proposer_ids": ["proposer-a"],
        "risk_classes": ["safety_critical" if high_risk else "compatibility"],
        "independent_authority_review_required": high_risk,
    }


class CodeSpecAuthorityReviewTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.quarantine = self.root / "target/gate-artifacts/authority-review"
        self.quarantine.mkdir(parents=True)
        self.proposal_path = self.quarantine / "proposal.json"
        self.card_path = self.quarantine / "review-card.json"
        self.human_path = self.quarantine / "human-selection.json"
        self.output_path = self.quarantine / "compiled-selection.json"
        self.proposal = {
            "schema_version": 1,
            "reviewed_date": "2026-07-24",
            "candidate_provenance": _candidate_provenance(),
            "rows": [
                {
                    "code_id": CODE_ID,
                    "spec_ref": "SPEC_EXAMPLE_001#exact",
                    "invariant_ids": ["INV_EXAMPLE_001"],
                    "discovery_ids": [DISCOVERY_ID],
                }
            ],
        }
        self.proposal_path.write_text(json.dumps(self.proposal))

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_card_and_human_selection(self) -> dict:
        card = write_authority_review_card(
            root=self.root,
            proposal_path=self.proposal_path,
            output_path=self.card_path,
        )
        template = write_authority_human_selection_template(
            root=self.root,
            review_card_path=self.card_path,
            output_path=self.human_path,
        )
        template["reviewer_ids"] = ["reviewer-a", "reviewer-b"]
        template["accepted_row_indices"] = [1]
        self.human_path.write_text(json.dumps(template))
        return card

    def test_generated_template_requires_only_manual_review_fields(self) -> None:
        write_authority_review_card(
            root=self.root,
            proposal_path=self.proposal_path,
            output_path=self.card_path,
        )

        template = write_authority_human_selection_template(
            root=self.root,
            review_card_path=self.card_path,
            output_path=self.human_path,
        )

        self.assertEqual(
            "generated_numbered_authority_selection_template",
            template["kind"],
        )
        self.assertEqual([], template["reviewer_ids"])
        self.assertEqual([], template["accepted_row_indices"])
        self.assertNotIn("proposal_path", template)
        self.assertNotIn("proposal_digest", template)
        self.assertNotIn("review_card_digest", template)

    def test_numbered_review_compiles_internal_ids_with_provenance(self) -> None:
        card = self._write_card_and_human_selection()

        compiled = compile_authority_review_selection(
            root=self.root,
            proposal_path=self.proposal_path,
            review_card_path=self.card_path,
            human_selection_path=self.human_path,
            output_path=self.output_path,
        )

        self.assertEqual(1, card["rows"][0]["row_index"])
        self.assertEqual([CODE_ID], [row["code_id"] for row in compiled["rows"]])
        self.assertEqual(
            ["reviewer-a", "reviewer-b"],
            compiled["review_provenance"]["reviewer_ids"],
        )
        human = json.loads(self.human_path.read_text())
        self.assertNotIn("code_id", human)
        self.assertNotIn("invariant_ids", human)
        self.assertNotIn("discovery_ids", human)
        self.assertEqual(compiled, json.loads(self.output_path.read_text()))

    def test_stale_proposal_or_card_digest_is_rejected(self) -> None:
        self._write_card_and_human_selection()
        changed = dict(self.proposal)
        changed["reviewed_date"] = "2026-07-25"
        self.proposal_path.write_text(json.dumps(changed))

        with self.assertRaisesRegex(ValueError, "stale|digest"):
            compile_authority_review_selection(
                root=self.root,
                proposal_path=self.proposal_path,
                review_card_path=self.card_path,
                human_selection_path=self.human_path,
                output_path=self.output_path,
            )

    def test_human_selection_cannot_supply_ids_and_requires_reviewers(self) -> None:
        self._write_card_and_human_selection()
        selection = json.loads(self.human_path.read_text())
        selection["code_id"] = CODE_ID
        self.human_path.write_text(json.dumps(selection))
        with self.assertRaisesRegex(ValueError, "fields drift"):
            compile_authority_review_selection(
                root=self.root,
                proposal_path=self.proposal_path,
                review_card_path=self.card_path,
                human_selection_path=self.human_path,
                output_path=self.output_path,
            )

        selection.pop("code_id")
        selection["reviewer_ids"] = []
        self.human_path.write_text(json.dumps(selection))
        with self.assertRaisesRegex(ValueError, "reviewer"):
            compile_authority_review_selection(
                root=self.root,
                proposal_path=self.proposal_path,
                review_card_path=self.card_path,
                human_selection_path=self.human_path,
                output_path=self.output_path,
            )

    def test_generated_binding_cannot_be_forged_by_human(self) -> None:
        self._write_card_and_human_selection()
        selection = json.loads(self.human_path.read_text())
        selection["generated_binding"]["proposal_digest"] = (
            "sha256:" + "0" * 64
        )
        self.human_path.write_text(json.dumps(selection))

        with self.assertRaisesRegex(ValueError, "generated binding is stale"):
            compile_authority_review_selection(
                root=self.root,
                proposal_path=self.proposal_path,
                review_card_path=self.card_path,
                human_selection_path=self.human_path,
                output_path=self.output_path,
            )

    def test_high_risk_review_is_independent_and_has_two_reviewers(self) -> None:
        self.proposal["candidate_provenance"] = _candidate_provenance(
            high_risk=True
        )
        self.proposal_path.write_text(json.dumps(self.proposal))
        self._write_card_and_human_selection()
        human = json.loads(self.human_path.read_text())
        human["reviewer_ids"] = ["proposer-a", "reviewer-b"]
        self.human_path.write_text(json.dumps(human))
        with self.assertRaisesRegex(ValueError, "independent"):
            compile_authority_review_selection(
                root=self.root,
                proposal_path=self.proposal_path,
                review_card_path=self.card_path,
                human_selection_path=self.human_path,
                output_path=self.output_path,
            )

        human["reviewer_ids"] = ["reviewer-a"]
        self.human_path.write_text(json.dumps(human))
        with self.assertRaisesRegex(ValueError, "two distinct"):
            compile_authority_review_selection(
                root=self.root,
                proposal_path=self.proposal_path,
                review_card_path=self.card_path,
                human_selection_path=self.human_path,
                output_path=self.output_path,
            )

        human["reviewer_ids"] = ["reviewer-a", "reviewer-b"]
        self.human_path.write_text(json.dumps(human))
        compiled = compile_authority_review_selection(
            root=self.root,
            proposal_path=self.proposal_path,
            review_card_path=self.card_path,
            human_selection_path=self.human_path,
            output_path=self.output_path,
        )
        self.assertEqual(
            ["proposer-a"],
            compiled["review_provenance"]["candidate_provenance"][
                "proposer_ids"
            ],
        )

    def test_numeric_indices_must_be_unique_and_in_range(self) -> None:
        self._write_card_and_human_selection()
        selection = json.loads(self.human_path.read_text())
        for indices in ([1, 1], [2]):
            with self.subTest(indices=indices):
                selection["accepted_row_indices"] = indices
                self.human_path.write_text(json.dumps(selection))
                with self.assertRaisesRegex(ValueError, "indices"):
                    compile_authority_review_selection(
                        root=self.root,
                        proposal_path=self.proposal_path,
                        review_card_path=self.card_path,
                        human_selection_path=self.human_path,
                        output_path=self.output_path,
                    )

    def test_quarantine_inputs_and_outputs_reject_symlink_ancestors(self) -> None:
        alias = self.root / "target/gate-artifacts/authority-review-alias"
        alias.symlink_to(self.quarantine, target_is_directory=True)

        with self.assertRaisesRegex(ValueError, "symlink"):
            write_authority_review_card(
                root=self.root,
                proposal_path=alias / self.proposal_path.name,
                output_path=self.card_path,
            )
        with self.assertRaisesRegex(ValueError, "symlink"):
            write_authority_review_card(
                root=self.root,
                proposal_path=self.proposal_path,
                output_path=alias / "card.json",
            )


if __name__ == "__main__":
    unittest.main()
