"""Focused tests for reviewed code-spec semantic-group acceptance."""

from __future__ import annotations

import hashlib
import json
import tempfile
import tomllib
import unittest
from pathlib import Path

try:
    from scripts.verification.code_spec_semantic_projection import (
        project_semantic_selection,
    )
except ModuleNotFoundError:
    project_semantic_selection = None

try:
    from scripts.verification.code_spec_semantic_selection import (
        apply_semantic_application,
        build_semantic_application,
    )
except ModuleNotFoundError:
    apply_semantic_application = None
    build_semantic_application = None


CODE_A = "CODE_" + "1" * 64
CODE_B = "CODE_" + "2" * 64
CROSS_GROUP = "CODEMAP_GROUP_" + "1" * 64
TEST_GROUP = "CODEMAP_GROUP_" + "2" * 64
DIGEST_A = "sha256:" + "1" * 64
DIGEST_B = "sha256:" + "2" * 64
REPORT_DIGEST = "sha256:" + "3" * 64
INPUT_DIGEST = "sha256:" + "4" * 64
OPERATION_DIGEST = "sha256:" + "5" * 64
REVIEW_REF = (
    "docs/internal/testing/evidence/plc-verification-program/"
    "semantic-review.md#semantic-review"
)


def canonical_json(value: dict[str, object]) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def code_map_bytes() -> bytes:
    return f"""schema_version = 1

[[mappings]]
schema_version = 1
code_id = "{CODE_A}"
source_kind = "rust"
path = "crates/example/src/lib.rs"
qualified_symbol = "example::run"
item_kind = "rust_function"
signature_digest = "{DIGEST_A}"
reviewed_body_digest = "{DIGEST_A}"
owner = "trust-runtime"
area = "protocols"
visibility = "private"
risk_signals = []
disposition = "specified"
spec_ref = "SPEC_RUNTIME#behavior"
invariant_ids = ["INV_SHARED"]
test_ids = ["TEST_GOOD", "TEST_WRONG"]
last_reviewed = "2026-07-28"

[[mappings]]
schema_version = 1
code_id = "{CODE_B}"
source_kind = "rust"
path = "crates/example/src/lib.rs"
qualified_symbol = "example::emit"
item_kind = "rust_function"
signature_digest = "{DIGEST_B}"
reviewed_body_digest = "{DIGEST_B}"
owner = "trust-runtime"
area = "protocols"
visibility = "private"
risk_signals = []
disposition = "specified"
spec_ref = "SPEC_RUNTIME#behavior"
invariant_ids = ["INV_SHARED"]
test_ids = ["TEST_GOOD"]
last_reviewed = "2026-07-28"
""".encode()


def proposal_bytes(mapping_bytes: bytes | None = None) -> bytes:
    mapping_bytes = mapping_bytes or code_map_bytes()
    proposal = {
        "schema_version": 1,
        "generator": "code-spec-refresh-proposals",
        "generator_version": "1",
        "mode": "proposal_only",
        "source_observation_status": "untrusted_until_live_reconstruction",
        "source_commit": "a" * 40,
        "source_report": {
            "path": "target/gate-artifacts/code-spec/code-spec-coverage.json",
            "sha256": REPORT_DIGEST,
            "input_digest": INPUT_DIGEST,
        },
        "source_code_map_sha256": (
            "sha256:" + hashlib.sha256(mapping_bytes).hexdigest()
        ),
        "boundaries": {
            "automatic_writes": False,
            "creates_product_proof": False,
            "closes_spec_gaps": False,
            "promotes_invariants": False,
            "infers_semantic_bindings": False,
            "source_facts_live_reconstructed": False,
        },
        "summary": {
            "source_facts": 2,
            "source_findings": 3,
            "partitioned_findings": 3,
            "missing_mapping_proposals": 0,
            "digest_rebind_proposals": 0,
            "stale_identity_proposals": 0,
            "semantic_review_groups": 2,
            "unclassified_findings": 0,
        },
        "missing_mapping_proposals": [],
        "digest_rebind_proposals": [],
        "stale_identity_proposals": [],
        "semantic_review_groups": [
            {
                "review_group_id": CROSS_GROUP,
                "kind": "cross_area_decision",
                "finding": (
                    "invariant INV_SHARED crosses area without decision"
                ),
                "affected_refs": [CODE_A, CODE_B],
                "affected_code_ids": [CODE_A, CODE_B],
                "owners": ["trust-runtime"],
                "review_status": "review_required",
                "required_reviewer_roles": ["semantic_owner"],
            },
            {
                "review_group_id": TEST_GROUP,
                "kind": "test_invariant_relation",
                "finding": (
                    "test TEST_WRONG does not link a mapped invariant"
                ),
                "affected_refs": [CODE_A],
                "affected_code_ids": [CODE_A],
                "owners": ["trust-runtime"],
                "review_status": "review_required",
                "required_reviewer_roles": ["semantic_owner"],
            },
        ],
        "unclassified_findings": [],
        "operation_set_digest": OPERATION_DIGEST,
    }
    return canonical_json(proposal)


def selection_bytes(proposal: bytes | None = None) -> bytes:
    proposal = proposal or proposal_bytes()
    return canonical_json(
        {
            "schema_version": 1,
            "kind": "p18_code_spec_semantic_acceptance_selection",
            "reviewed_on": "2026-07-29",
            "reviewer_ids": ["semantic-owner"],
            "review_ref": REVIEW_REF,
            "source_proposal_sha256": (
                "sha256:" + hashlib.sha256(proposal).hexdigest()
            ),
            "operation_set_digest": OPERATION_DIGEST,
            "semantic_decisions": [
                {
                    "review_group_id": CROSS_GROUP,
                    "decision": "accept_cross_area",
                },
                {
                    "review_group_id": TEST_GROUP,
                    "decision": "remove_mapped_test",
                    "test_id": "TEST_WRONG",
                },
            ],
        }
    )


class CodeSpecSemanticSelectionTests(unittest.TestCase):
    def test_projection_contract_is_available(self) -> None:
        self.assertIsNotNone(
            project_semantic_selection,
            "reviewed semantic-group projection is not implemented",
        )

    def test_application_contract_is_available(self) -> None:
        self.assertIsNotNone(
            build_semantic_application,
            "live-reconstructed semantic application is not implemented",
        )
        self.assertIsNotNone(
            apply_semantic_application,
            "compare-and-swap semantic application is not implemented",
        )

    def test_complete_projection_accepts_cross_area_and_removes_false_test(
        self,
    ) -> None:
        self.assertIsNotNone(project_semantic_selection)
        projection = project_semantic_selection(
            proposal_bytes=proposal_bytes(),
            selection_bytes=selection_bytes(),
            code_map_bytes=code_map_bytes(),
        )
        parsed = tomllib.loads(
            projection.replacements["verification/code-spec-map.toml"].decode()
        )
        mappings = {row["code_id"]: row for row in parsed["mappings"]}
        self.assertEqual(REVIEW_REF, mappings[CODE_A]["cross_area_decision_ref"])
        self.assertEqual(REVIEW_REF, mappings[CODE_B]["cross_area_decision_ref"])
        self.assertEqual(["TEST_GOOD"], mappings[CODE_A]["test_ids"])
        self.assertEqual(["TEST_GOOD"], mappings[CODE_B]["test_ids"])
        self.assertEqual(1, projection.manifest["accepted_cross_area_groups"])
        self.assertEqual(1, projection.manifest["removed_mapped_test_groups"])

    def test_projection_rejects_partial_review_queue(self) -> None:
        self.assertIsNotNone(project_semantic_selection)
        selection = json.loads(selection_bytes())
        selection["semantic_decisions"].pop()
        with self.assertRaisesRegex(ValueError, "semantic review is partial"):
            project_semantic_selection(
                proposal_bytes=proposal_bytes(),
                selection_bytes=canonical_json(selection),
                code_map_bytes=code_map_bytes(),
            )

    def test_projection_rejects_identity_queue(self) -> None:
        self.assertIsNotNone(project_semantic_selection)
        proposal = json.loads(proposal_bytes())
        proposal["digest_rebind_proposals"] = [{"proposal_id": "unexpected"}]
        proposal["summary"]["digest_rebind_proposals"] = 1
        encoded = canonical_json(proposal)
        with self.assertRaisesRegex(ValueError, "identity queue is not empty"):
            project_semantic_selection(
                proposal_bytes=encoded,
                selection_bytes=selection_bytes(encoded),
                code_map_bytes=code_map_bytes(),
            )

    def test_projection_rejects_wrong_test_decision(self) -> None:
        self.assertIsNotNone(project_semantic_selection)
        selection = json.loads(selection_bytes())
        selection["semantic_decisions"][1]["test_id"] = "TEST_OTHER"
        with self.assertRaisesRegex(ValueError, "finding test differs"):
            project_semantic_selection(
                proposal_bytes=proposal_bytes(),
                selection_bytes=canonical_json(selection),
                code_map_bytes=code_map_bytes(),
            )

    def test_application_rebuild_detects_selection_drift(self) -> None:
        self.assertIsNotNone(build_semantic_application)
        self.assertIsNotNone(apply_semantic_application)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "verification").mkdir()
            target = root / "target/gate-artifacts/code-spec-refresh"
            target.mkdir(parents=True)
            mapping = code_map_bytes()
            proposal = proposal_bytes(mapping)
            selection = selection_bytes(proposal)
            (root / "verification/code-spec-map.toml").write_bytes(mapping)
            proposal_path = target / "proposal.json"
            selection_path = target / "selection.json"
            output_path = target / "application.json"
            proposal_path.write_bytes(proposal)
            selection_path.write_bytes(selection)

            def reconstruct(_root: Path, _proposal: bytes) -> dict[str, object]:
                return {"source_commit": "a" * 40, "semantic_review_groups": 2}

            application = build_semantic_application(
                root=root,
                proposal_path=proposal_path,
                selection_path=selection_path,
                output_path=output_path,
                reconstruct_live=reconstruct,
            )
            changed = json.loads(selection)
            changed["reviewed_on"] = "2026-07-30"
            selection_path.write_bytes(canonical_json(changed))
            with self.assertRaisesRegex(ValueError, "differs from canonical rebuilt"):
                apply_semantic_application(
                    root=root,
                    proposal_path=proposal_path,
                    selection_path=selection_path,
                    output_path=output_path,
                    application=application,
                    reconstruct_live=reconstruct,
                    post_validate=lambda _root: [],
                )

    def test_metadata_gate_registers_semantic_acceptance_tests(self) -> None:
        gate = Path("scripts/verification_metadata_gate.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "scripts.verification.code_spec_semantic_selection_tests",
            gate,
            "metadata gate does not execute semantic acceptance self-tests",
        )


if __name__ == "__main__":
    unittest.main()
