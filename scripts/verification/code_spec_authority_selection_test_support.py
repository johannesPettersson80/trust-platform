"""Reusable repository fixture for authority-selection focused tests."""

from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

from scripts.verification.code_spec_authority_review import (
    compile_authority_review_selection,
    write_authority_human_selection_template,
    write_authority_review_card,
)
from scripts.verification.code_spec_authority_selection import (
    build_authority_selection_application,
)


CODE_ID = "CODE_" + "A" * 64
OTHER_CODE_ID = "CODE_" + "C" * 64
SECOND_CODE_ID = "CODE_" + "D" * 64
DISCOVERY_ID = "DISC_" + "B" * 20
TEST_ID = "TEST_AUTO_" + "B" * 20
SPEC_REF = "SPEC_EXAMPLE_001#exact-behavior"
INVARIANT_ID = "INV_EXAMPLE_001"
OUTPUT_PATH = Path("target/gate-artifacts/authority-selection/plan.json")


def digest(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def mapping_block(code_id: str, *, selected: bool) -> str:
    semantic = (
        'disposition = "spec_gap"\n'
        'spec_gap_ref = "SPEC_GAP_EXAMPLE_001"\n'
        'last_reviewed = "2026-07-20"\n'
        if selected
        else (
            'disposition = "specified"\n'
            f'spec_ref = "{SPEC_REF}"\n'
            f'invariant_ids = ["{INVARIANT_ID}"]\n'
            f'test_ids = ["{TEST_ID}"]\n'
            'last_reviewed = "2026-07-20"\n'
        )
    )
    return (
        "[[mappings]]\n"
        "schema_version = 1\n"
        f'code_id = "{code_id}"\n'
        'source_kind = "rust"\n'
        'path = "crates/example/src/lib.rs"\n'
        f'qualified_symbol = "example::{code_id[-4:]}"\n'
        'item_kind = "rust_function"\n'
        f'signature_digest = "sha256:{"1" * 64}"\n'
        f'reviewed_body_digest = "sha256:{"2" * 64}"\n'
        'owner = "example"\n'
        'area = "runtime_safety"\n'
        'visibility = "private"\n'
        'risk_signals = ["state_mutation"]\n'
        f'owner_ack = "RISK_ACK_{"3" * 64}"\n'
        'owner_ack_expires_on = "2026-12-31"\n'
        + semantic
    )


def candidate_provenance() -> dict:
    return {
        "compiler": "numbered_code_spec_authority_candidates_v1",
        "candidate_card_path": (
            "target/gate-artifacts/authority-selection/candidate-card.json"
        ),
        "candidate_card_digest": "sha256:" + "1" * 64,
        "candidate_human_selection_path": (
            "target/gate-artifacts/authority-selection/candidate-human.json"
        ),
        "candidate_human_selection_digest": "sha256:" + "2" * 64,
        "trace_path": "target/gate-artifacts/authority-selection/trace.json",
        "trace_digest": "sha256:" + "3" * 64,
        "proposer_ids": ["proposer-a"],
        "risk_classes": ["compatibility"],
        "independent_authority_review_required": False,
    }


class AuthoritySelectionFixture:
    def __init__(
        self,
        root: Path,
        *,
        selection_extension: str = "json",
        include_canonical_authority: bool = False,
    ) -> None:
        self.root = root
        self.include_canonical_authority = include_canonical_authority
        self.selection_path = (
            root
            / "target/gate-artifacts/authority-selection"
            / f"selection.{selection_extension}"
        )
        self.output_path = root / OUTPUT_PATH
        self.selection = {
            "schema_version": 1,
            "reviewed_date": "2026-07-24",
            "review_provenance": {
                "compiler": "numbered_code_spec_authority_review_v1",
                "proposal_path": (
                    "target/gate-artifacts/authority-selection/proposal.json"
                ),
                "proposal_digest": "sha256:" + "4" * 64,
                "review_card_path": (
                    "target/gate-artifacts/authority-selection/review-card.json"
                ),
                "review_card_digest": "sha256:" + "5" * 64,
                "human_selection_path": (
                    "target/gate-artifacts/authority-selection/human.json"
                ),
                "human_selection_digest": "sha256:" + "6" * 64,
                "reviewer_ids": ["reviewer-a"],
                "candidate_provenance": candidate_provenance(),
            },
            "rows": [
                {
                    "code_id": CODE_ID,
                    "spec_ref": SPEC_REF,
                    "invariant_ids": [INVARIANT_ID],
                    "discovery_ids": [DISCOVERY_ID],
                }
            ],
        }
        self._write_repository()
        self.write_selection()

    def _write_repository(self) -> None:
        verification = self.root / "verification"
        invariants = verification / "invariants/runtime_safety"
        schemas = verification / "schemas"
        docs = self.root / "docs/specs"
        for path in (invariants, schemas, docs, self.selection_path.parent):
            path.mkdir(parents=True, exist_ok=True)
        for name in ("candidate-card.json", "candidate-human.json", "trace.json"):
            (self.selection_path.parent / name).write_text(
                '{"fixture":true}\n', encoding="utf-8"
            )
        self.code_map_bytes = (
            "# fixture header remains byte-exact\n"
            "schema_version = 1\n\n"
            + mapping_block(CODE_ID, selected=True)
            + "\n# unselected marker\n"
            + mapping_block(OTHER_CODE_ID, selected=False)
        ).encode()
        (verification / "code-spec-map.toml").write_bytes(self.code_map_bytes)
        (verification / "spec-sources.toml").write_text(
            (
                "[[spec_sources]]\n"
                'id = "SPEC_EXAMPLE_001"\n'
                'area = "runtime_safety"\n'
                'authority = "normative_product"\n'
                'source_status = "active"\n'
                "oracle_eligible = true\n"
                'locator_kind = "tracked_file"\n'
                'path = "docs/specs/example.md"\n'
            ),
            encoding="utf-8",
        )
        (docs / "example.md").write_text(
            "# Example\n\n## Exact behavior\n", encoding="utf-8"
        )
        invariant_tests = (
            f'tests = ["{TEST_ID}"]\n'
            if self.include_canonical_authority
            else "tests = []\n"
        )
        (invariants / f"{INVARIANT_ID}.toml").write_text(
            (
                "schema_version = 1\n"
                f'id = "{INVARIANT_ID}"\n'
                'area = "runtime_safety"\n'
                + invariant_tests
                + "[spec]\n"
                'status = "specified"\n'
                'source_refs = ["SPEC_EXAMPLE_001"]\n'
                "[oracle]\n"
                'kind = "trust_contract"\n'
                f'ref = "{SPEC_REF}"\n'
            ),
            encoding="utf-8",
        )
        catalog_rows = (
            "[[tests]]\n"
            f'id = "{TEST_ID}"\n'
            'subject_kind = "generated_test"\n'
            f'discovery_id = "{DISCOVERY_ID}"\n'
            'status = "mapped"\n'
            'area = "runtime_safety"\n'
            f'invariants = ["{INVARIANT_ID}"]\n'
            f'oracle_ref = "{SPEC_REF}"\n'
            'suite_tiers = ["pr"]\n'
            if self.include_canonical_authority
            else "tests = []\n"
        )
        (verification / "test-catalog.toml").write_text(
            "schema_version = 1\n" + catalog_rows,
            encoding="utf-8",
        )
        (verification / "test-catalog-denominator.toml").write_text(
            (
                "schema_version = 1\n"
                "[[reviews]]\n"
                f'discovery_id = "{DISCOVERY_ID}"\n'
                'discovery_source_kind = "rust_unit_test"\n'
                'path = "crates/example/src/lib.rs"\n'
                'name = "exact_behavior"\n'
                'ignore_state = "not_ignored"\n'
                'disposition = "reviewed_nonmapping"\n'
                'reason_code = "no_reviewed_spec_or_invariant_binding"\n'
                'last_reviewed = "2026-07-20"\n'
            ),
            encoding="utf-8",
        )

    def write_selection(self) -> None:
        proposal_path = self.selection_path.parent / "proposal.json"
        card_path = self.selection_path.parent / "review-card.json"
        human_path = self.selection_path.parent / "human.json"
        proposal = {
            "schema_version": self.selection["schema_version"],
            "reviewed_date": self.selection["reviewed_date"],
            "candidate_provenance": copy.deepcopy(
                self.selection["review_provenance"]["candidate_provenance"]
            ),
            "rows": copy.deepcopy(self.selection["rows"]),
        }
        proposal_path.write_text(json.dumps(proposal), encoding="utf-8")
        write_authority_review_card(
            root=self.root,
            proposal_path=proposal_path,
            output_path=card_path,
        )
        human = write_authority_human_selection_template(
            root=self.root,
            review_card_path=card_path,
            output_path=human_path,
        )
        human["reviewer_ids"] = ["reviewer-a"]
        human["accepted_row_indices"] = list(
            range(1, len(proposal["rows"]) + 1)
        )
        human_path.write_text(json.dumps(human), encoding="utf-8")
        compiled_output = (
            self.selection_path
            if self.selection_path.suffix == ".json"
            else self.selection_path.parent / "compiled.json"
        )
        self.selection = compile_authority_review_selection(
            root=self.root,
            proposal_path=proposal_path,
            review_card_path=card_path,
            human_selection_path=human_path,
            output_path=compiled_output,
        )
        if self.selection_path.suffix == ".toml":
            self._write_raw_selection()

    def _write_raw_selection(self) -> None:
        if self.selection_path.suffix == ".json":
            self.selection_path.write_text(
                json.dumps(self.selection), encoding="utf-8"
            )
            return
        provenance = self.selection["review_provenance"]
        lines = [
            f"schema_version = {self.selection['schema_version']}",
            f'reviewed_date = "{self.selection["reviewed_date"]}"',
            "[review_provenance]",
        ]
        for field in (
            "compiler",
            "proposal_path",
            "proposal_digest",
            "review_card_path",
            "review_card_digest",
            "human_selection_path",
            "human_selection_digest",
        ):
            lines.append(f"{field} = {json.dumps(provenance[field])}")
        reviewers = ", ".join(
            json.dumps(value) for value in provenance["reviewer_ids"]
        )
        lines.append(f"reviewer_ids = [{reviewers}]")
        candidate = provenance["candidate_provenance"]
        lines.append("[review_provenance.candidate_provenance]")
        for field in (
            "compiler",
            "candidate_card_path",
            "candidate_card_digest",
            "candidate_human_selection_path",
            "candidate_human_selection_digest",
            "trace_path",
            "trace_digest",
        ):
            lines.append(f"{field} = {json.dumps(candidate[field])}")
        for field in ("proposer_ids", "risk_classes"):
            values = ", ".join(
                json.dumps(value) for value in candidate[field]
            )
            lines.append(f"{field} = [{values}]")
        independence = str(
            candidate["independent_authority_review_required"]
        ).lower()
        lines.append(
            "independent_authority_review_required = " + independence
        )
        for row in self.selection["rows"]:
            lines.append("[[rows]]")
            for field in ("code_id", "spec_ref"):
                lines.append(f"{field} = {json.dumps(row[field])}")
            for field in ("invariant_ids", "discovery_ids"):
                values = ", ".join(
                    json.dumps(value) for value in row[field]
                )
                lines.append(f"{field} = [{values}]")
            for field, value in row.items():
                if field not in {
                    "code_id",
                    "spec_ref",
                    "invariant_ids",
                    "discovery_ids",
                }:
                    lines.append(f"{field} = {json.dumps(value)}")
        self.selection_path.write_text(
            "\n".join(lines) + "\n", encoding="utf-8"
        )

    def build(self):
        return build_authority_selection_application(
            root=self.root,
            selection_path=self.selection_path,
            output_path=self.output_path,
        )

    def rewrite_toml(self, relative: str, old: str, new: str) -> None:
        path = self.root / relative
        path.write_text(
            path.read_text(encoding="utf-8").replace(old, new),
            encoding="utf-8",
        )
