"""Focused tests for reviewed reuse of existing code-spec authority."""

from __future__ import annotations

import hashlib
import json
import tempfile
import tomllib
import unittest
from pathlib import Path

try:
    from scripts.verification.code_spec_authority_reuse import (
        _candidate_invariant,
        build_authority_reuse_proposal,
    )
except ModuleNotFoundError:
    _candidate_invariant = None
    build_authority_reuse_proposal = None

from scripts.verification.code_spec_fragments import source_fragment_exists

try:
    from scripts.verification.code_spec_authority_reuse_projection import (
        project_authority_reuse_selection,
    )
except ModuleNotFoundError:
    project_authority_reuse_selection = None

try:
    from scripts.verification.code_spec_authority_reuse_selection import (
        apply_authority_reuse_application,
        build_authority_reuse_application,
    )
except ModuleNotFoundError:
    apply_authority_reuse_application = None
    build_authority_reuse_application = None


CODE_A = "CODE_" + "1" * 64
CODE_B = "CODE_" + "2" * 64
DIGEST_A = "sha256:" + "1" * 64
DIGEST_B = "sha256:" + "2" * 64
REPORT_INPUT_DIGEST = "sha256:" + "3" * 64
GAP_ID = "SPEC_GAP_P17_HISTORICAL_CODE_RUNTIME_SAFETY_001"
INVARIANT_ID = "RT_EXAMPLE_CONTRACT_001"
SECOND_INVARIANT_ID = "RT_EXAMPLE_SECONDARY_001"
TEST_ID = "TEST_EXAMPLE_CONTRACT_001"
SPEC_REF = "SPEC_RUNTIME_ENGINE_001#example-contract"
REPORT_PATH = "target/gate-artifacts/code-spec/code-spec-coverage.json"
REVIEW_REF = (
    "docs/internal/testing/evidence/plc-verification-program/"
    "authority-reuse.md#semantic-review"
)


def canonical_json(value: dict[str, object]) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def mapping_block(
    code_id: str,
    symbol: str,
    digest: str,
) -> str:
    return f"""[[mappings]]
schema_version = 1
code_id = "{code_id}"
source_kind = "rust"
path = "crates/example/src/lib.rs"
qualified_symbol = "example::{symbol}"
item_kind = "rust_function"
signature_digest = "{digest}"
reviewed_body_digest = "{digest}"
owner = "trust-runtime"
area = "runtime_safety"
visibility = "private"
risk_signals = []
disposition = "spec_gap"
spec_gap_ref = "{GAP_ID}"
last_reviewed = "2026-07-28"
"""


def code_map_bytes() -> bytes:
    return (
        "schema_version = 1\n\n"
        + mapping_block(CODE_A, "run", DIGEST_A)
        + "\n"
        + mapping_block(CODE_B, "stop", DIGEST_B)
    ).encode()


def spec_gaps_bytes() -> bytes:
    return f"""[[spec_gaps]]
schema_version = 1
id = "{GAP_ID}"
title = "Historical runtime behavior remains unspecified"
area = "runtime_safety"
risk = "wrong_result"
owner = "trust-runtime"
status = "spec_gap"
gap_class = "missing_behavior"
blocking_question = "What behavior governs these facts?"
affected_invariants = ["P17_HISTORICAL_CODE_RUNTIME_SAFETY_001"]
affected_tests = []
affected_code_ids = ["{CODE_A}", "{CODE_B}"]
candidate_spec_sources = ["SPEC_RUNTIME_ENGINE_001"]
resolution_status = "registered_backlog"
closeout_evidence = []
last_reviewed = "2026-07-28"
""".encode()


def report_bytes() -> bytes:
    facts = []
    for code_id, symbol, digest in (
        (CODE_A, "run", DIGEST_A),
        (CODE_B, "stop", DIGEST_B),
    ):
        facts.append(
            {
                "area": "runtime_safety",
                "area_candidates": ["runtime_safety"],
                "body_digest": digest,
                "code_id": code_id,
                "disposition": "spec_gap",
                "disposition_ref": GAP_ID,
                "findings": [],
                "item_kind": "rust_function",
                "owner": "trust-runtime",
                "path": "crates/example/src/lib.rs",
                "qualified_symbol": f"example::{symbol}",
                "risk_signals": [],
                "signature_digest": digest,
                "source_kind": "rust",
                "visibility": "private",
            }
        )
    return canonical_json(
        {
            "schema_version": 1,
            "generator": "code-spec-coverage",
            "generator_version": 1,
            "commit": "a" * 40,
            "input_digest": REPORT_INPUT_DIGEST,
            "report_status": "complete",
            "facts": facts,
            "findings": [],
            "summary": {
                "facts_total": 2,
                "mapped_total": 2,
                "findings_total": 0,
            },
        }
    )


def spec_sources() -> dict[str, dict[str, object]]:
    return {
        "SPEC_RUNTIME_ENGINE_001": {
            "id": "SPEC_RUNTIME_ENGINE_001",
            "area": "runtime_safety",
            "source_status": "active",
            "oracle_eligible": True,
            "authority": "normative_product",
        }
    }


def invariant(
    invariant_id: str = INVARIANT_ID,
) -> dict[str, object]:
    return {
        "id": invariant_id,
        "title": "Example runtime contract",
        "area": "runtime_safety",
        "risk": "wrong_result",
        "status": "mapped",
        "owner": "trust-runtime",
        "claim": "Both example entrypoints preserve the reviewed result contract.",
        "source_refs": ["crates/example/src/lib.rs"],
        "tests": [TEST_ID],
        "spec": {
            "status": "specified",
            "source_refs": ["SPEC_RUNTIME_ENGINE_001"],
        },
        "oracle": {"kind": "trust_contract", "ref": SPEC_REF},
        "behavior": [],
    }


def tests() -> dict[str, dict[str, object]]:
    return {
        TEST_ID: {
            "id": TEST_ID,
            "area": "runtime_safety",
            "status": "mapped",
            "invariants": [INVARIANT_ID],
        }
    }


def proposal_bytes(
    *,
    invariant_rows: dict[str, dict[str, object]] | None = None,
) -> bytes:
    assert build_authority_reuse_proposal is not None
    selected_invariants = invariant_rows or {INVARIANT_ID: invariant()}
    test_rows = tests()
    test_rows[TEST_ID]["invariants"] = sorted(selected_invariants)
    proposal = build_authority_reuse_proposal(
        report_path=REPORT_PATH,
        report_bytes=report_bytes(),
        code_map_bytes=code_map_bytes(),
        spec_gaps_bytes=spec_gaps_bytes(),
        spec_sources=spec_sources(),
        invariants=selected_invariants,
        tests=test_rows,
        source_bindings={
            "verification/spec-sources.toml": "sha256:" + "4" * 64,
            "verification/test-catalog.toml": "sha256:" + "5" * 64,
            (
                "verification/invariants/runtime_safety/"
                f"{INVARIANT_ID}.toml"
            ): "sha256:" + "6" * 64,
        },
        gate_routes={},
    )
    return canonical_json(proposal)


def selection_bytes(proposal: bytes | None = None) -> bytes:
    proposal = proposal or proposal_bytes()
    parsed = json.loads(proposal)
    group_id = parsed["candidate_groups"][0]["candidate_group_id"]
    return canonical_json(
        {
            "schema_version": 1,
            "kind": "p18_code_spec_authority_reuse_selection",
            "reviewed_on": "2026-07-29",
            "reviewer_ids": ["semantic-owner"],
            "review_ref": REVIEW_REF,
            "source_proposal_sha256": (
                "sha256:" + hashlib.sha256(proposal).hexdigest()
            ),
            "operation_set_digest": parsed["operation_set_digest"],
            "decisions": [
                {
                    "candidate_group_id": group_id,
                    "decision": "accept",
                    "invariant_id": INVARIANT_ID,
                }
            ],
        }
    )


class CodeSpecAuthorityReuseTests(unittest.TestCase):
    def test_proposal_contract_is_available(self) -> None:
        self.assertIsNotNone(
            build_authority_reuse_proposal,
            "authority-reuse proposal contract is not implemented",
        )

    def test_projection_contract_is_available(self) -> None:
        self.assertIsNotNone(
            project_authority_reuse_selection,
            "authority-reuse projection contract is not implemented",
        )

    def test_application_contract_is_available(self) -> None:
        self.assertIsNotNone(
            build_authority_reuse_application,
            "authority-reuse application is not implemented",
        )
        self.assertIsNotNone(
            apply_authority_reuse_application,
            "authority-reuse compare-and-swap is not implemented",
        )

    def test_singular_exact_path_authority_is_review_only(self) -> None:
        proposal = json.loads(proposal_bytes())
        self.assertEqual(2, proposal["summary"]["singular_candidate_facts"])
        self.assertEqual(0, proposal["summary"]["ambiguous_candidate_facts"])
        self.assertEqual(0, proposal["summary"]["no_candidate_facts"])
        self.assertEqual(1, len(proposal["candidate_groups"]))
        group = proposal["candidate_groups"][0]
        self.assertEqual("singular", group["classification"])
        self.assertEqual([INVARIANT_ID], group["candidate_invariant_ids"])
        self.assertFalse(proposal["boundaries"]["automatic_writes"])
        self.assertFalse(proposal["boundaries"]["creates_product_proof"])

    def test_metadata_integrity_authority_uses_an_exact_live_fragment(self) -> None:
        root = Path.cwd()
        invariant_id = "VERIFICATION_METADATA_INTEGRITY_001"
        invariant_path = (
            root
            / "verification/invariants/verification"
            / f"{invariant_id}.toml"
        )
        metadata_invariant = tomllib.loads(
            invariant_path.read_text(encoding="utf-8")
        )
        source_rows = tomllib.loads(
            (root / "verification/spec-sources.toml").read_text(
                encoding="utf-8"
            )
        )["spec_sources"]
        sources = {row["id"]: row for row in source_rows}
        test_rows = tomllib.loads(
            (root / "verification/test-catalog.toml").read_text(
                encoding="utf-8"
            )
        )["tests"]
        catalog = {row["id"]: row for row in test_rows}

        oracle_ref = metadata_invariant["oracle"]["ref"]
        self.assertEqual(
            (
                "SPEC_VERIFICATION_PROGRAM_CONTRACT_001"
                "#verification-metadata-model"
            ),
            oracle_ref,
            "metadata integrity authority is not bound to its exact contract",
        )
        source_id, fragment = oracle_ref.split("#", 1)
        self.assertTrue(
            source_fragment_exists(sources[source_id], fragment, root=root),
            "metadata integrity authority fragment does not resolve",
        )
        self.assertIsNotNone(_candidate_invariant)
        candidate = _candidate_invariant(
            invariant_id=invariant_id,
            invariant=metadata_invariant,
            spec_sources=sources,
            tests=catalog,
        )
        self.assertIsNotNone(candidate)
        metadata_test = catalog["TEST_VERIFICATION_METADATA_GATE_001"]
        self.assertEqual(
            oracle_ref,
            metadata_test["oracle_ref"],
            "metadata gate catalog authority is not fragment-bound",
        )
        self.assertEqual(
            [
                "scripts/validate_verification_metadata.py",
                "scripts/verification/metadata_validator/oracle_refs.py",
                "scripts/verification/metadata_validator/schema_contracts.py",
                "scripts/verification/metadata_validator/spec_sources.py",
                "scripts/verification_metadata_gate.sh",
            ],
            sorted(
                path
                for path in candidate["source_refs"]
                if path.startswith("scripts/")
            ),
        )

    def test_metadata_gate_registers_focused_contract_tests(self) -> None:
        gate = Path("scripts/verification_metadata_gate.sh").read_text(
            encoding="utf-8"
        )

        for module in (
            "scripts.verification.metadata_validator.oracle_refs_tests",
            "scripts.verification.metadata_validator.schema_contracts_tests",
            "scripts.verification.metadata_validator.spec_sources_tests",
        ):
            with self.subTest(module=module):
                self.assertIn(module, gate)

    def test_runtime_apply_authority_uses_exact_implementation_paths(self) -> None:
        invariant = tomllib.loads(
            Path(
                "verification/invariants/runtime_safety/"
                "RT_COMM_APPLY_MUTATION_001.toml"
            ).read_text(encoding="utf-8")
        )

        self.assertEqual(
            [
                "crates/trust-runtime/src/control/comm_handlers/apply.rs",
                "crates/trust-runtime/src/control/comm_handlers/apply/io_file.rs",
                "crates/trust-runtime/src/control/comm_handlers/apply/runtime_file.rs",
                (
                    "crates/trust-runtime/src/control/comm_handlers/apply/"
                    "runtime_file/path_policy.rs"
                ),
                "crates/trust-runtime/src/control/comm_handlers/apply/validation.rs",
            ],
            sorted(
                source_ref
                for source_ref in invariant["source_refs"]
                if source_ref.startswith("crates/")
            ),
        )

    def test_implemented_authority_remains_reusable_after_identity_move(self) -> None:
        implemented = invariant()
        implemented["status"] = "implemented"

        proposal = json.loads(
            proposal_bytes(invariant_rows={INVARIANT_ID: implemented})
        )

        self.assertEqual(2, proposal["summary"]["singular_candidate_facts"])
        self.assertEqual(0, proposal["summary"]["no_candidate_facts"])
        self.assertEqual(
            [INVARIANT_ID],
            proposal["candidate_groups"][0]["candidate_invariant_ids"],
        )

    def test_multiple_exact_path_authorities_remain_ambiguous(self) -> None:
        secondary = invariant(SECOND_INVARIANT_ID)
        secondary["oracle"] = {
            "kind": "trust_contract",
            "ref": "SPEC_RUNTIME_ENGINE_001#secondary-contract",
        }
        proposal = json.loads(
            proposal_bytes(
                invariant_rows={
                    INVARIANT_ID: invariant(),
                    SECOND_INVARIANT_ID: secondary,
                }
            )
        )
        self.assertEqual(0, proposal["summary"]["singular_candidate_facts"])
        self.assertEqual(2, proposal["summary"]["ambiguous_candidate_facts"])
        self.assertEqual(
            [INVARIANT_ID, SECOND_INVARIANT_ID],
            proposal["candidate_groups"][0]["candidate_invariant_ids"],
        )

    def test_complete_review_projects_map_and_gap_ledger(self) -> None:
        assert project_authority_reuse_selection is not None
        proposal = proposal_bytes()
        projection = project_authority_reuse_selection(
            proposal_bytes=proposal,
            selection_bytes=selection_bytes(proposal),
            code_map_bytes=code_map_bytes(),
            spec_gaps_bytes=spec_gaps_bytes(),
        )
        mappings = tomllib.loads(
            projection.replacements["verification/code-spec-map.toml"].decode()
        )["mappings"]
        for row in mappings:
            self.assertEqual("specified", row["disposition"])
            self.assertEqual(SPEC_REF, row["spec_ref"])
            self.assertEqual([INVARIANT_ID], row["invariant_ids"])
            self.assertEqual([TEST_ID], row["test_ids"])
            self.assertNotIn("spec_gap_ref", row)
        gap = tomllib.loads(
            projection.replacements["verification/spec-gaps.toml"].decode()
        )["spec_gaps"][0]
        self.assertEqual([], gap["affected_code_ids"])
        self.assertEqual(2, projection.manifest["accepted_code_mappings"])

    def test_projection_rejects_partial_or_ambiguous_acceptance(self) -> None:
        assert project_authority_reuse_selection is not None
        proposal = proposal_bytes()
        selection = json.loads(selection_bytes(proposal))
        selection["decisions"] = []
        with self.assertRaisesRegex(ValueError, "review is partial"):
            project_authority_reuse_selection(
                proposal_bytes=proposal,
                selection_bytes=canonical_json(selection),
                code_map_bytes=code_map_bytes(),
                spec_gaps_bytes=spec_gaps_bytes(),
            )

        ambiguous = proposal_bytes(
            invariant_rows={
                INVARIANT_ID: invariant(),
                SECOND_INVARIANT_ID: invariant(SECOND_INVARIANT_ID),
            }
        )
        selection = json.loads(selection_bytes(ambiguous))
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            project_authority_reuse_selection(
                proposal_bytes=ambiguous,
                selection_bytes=canonical_json(selection),
                code_map_bytes=code_map_bytes(),
                spec_gaps_bytes=spec_gaps_bytes(),
            )

    def test_application_rebuild_detects_selection_drift(self) -> None:
        assert build_authority_reuse_application is not None
        assert apply_authority_reuse_application is not None
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "verification").mkdir()
            target = root / "target/gate-artifacts/authority-reuse"
            target.mkdir(parents=True)
            proposal = proposal_bytes()
            selection = selection_bytes(proposal)
            (root / "verification/code-spec-map.toml").write_bytes(
                code_map_bytes()
            )
            (root / "verification/spec-gaps.toml").write_bytes(
                spec_gaps_bytes()
            )
            proposal_path = target / "proposal.json"
            selection_path = target / "selection.json"
            output_path = target / "application.json"
            proposal_path.write_bytes(proposal)
            selection_path.write_bytes(selection)

            def reconstruct(
                _root: Path, _proposal: bytes
            ) -> dict[str, object]:
                return {"source_commit": "a" * 40, "candidate_groups": 1}

            application = build_authority_reuse_application(
                root=root,
                proposal_path=proposal_path,
                selection_path=selection_path,
                output_path=output_path,
                reconstruct_live=reconstruct,
            )
            changed = json.loads(selection)
            changed["reviewed_on"] = "2026-07-30"
            selection_path.write_bytes(canonical_json(changed))
            with self.assertRaisesRegex(
                ValueError, "differs from canonical rebuilt"
            ):
                apply_authority_reuse_application(
                    root=root,
                    proposal_path=proposal_path,
                    selection_path=selection_path,
                    output_path=output_path,
                    application=application,
                    reconstruct_live=reconstruct,
                    post_validate=lambda _root: [],
                )

    def test_metadata_gate_registers_authority_reuse_tests(self) -> None:
        gate = Path("scripts/verification_metadata_gate.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "scripts.verification.code_spec_authority_reuse_tests",
            gate,
            "metadata gate does not execute authority-reuse self-tests",
        )


if __name__ == "__main__":
    unittest.main()
