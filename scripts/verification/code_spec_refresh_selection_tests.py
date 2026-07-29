"""Focused tests for reviewed code-spec refresh acceptance projection."""

from __future__ import annotations

import hashlib
import json
import tempfile
import tomllib
import unittest
from pathlib import Path

try:
    from scripts.verification.code_spec_refresh_projection import (
        project_refresh_selection,
    )
except ModuleNotFoundError:
    project_refresh_selection = None

try:
    from scripts.verification.code_spec_refresh_selection import (
        apply_refresh_application,
        build_refresh_application,
    )
except ModuleNotFoundError:
    apply_refresh_application = None
    build_refresh_application = None


OLD = "CODE_" + "1" * 64
SUCCESSOR = "CODE_" + "2" * 64
REMOVED = "CODE_" + "3" * 64
REBIND = "CODE_" + "4" * 64
DIRECT = "CODE_" + "5" * 64
OLD_ACK = "RISK_ACK_" + "1" * 64
SUCCESSOR_ACK = "RISK_ACK_" + "2" * 64
REBIND_ACK = "RISK_ACK_" + "4" * 64
DIRECT_ACK = "RISK_ACK_" + "5" * 64
GAP = "SPEC_GAP_P17_HISTORICAL_CODE_RUNTIME_SAFETY_001"


class CodeSpecRefreshSelectionTests(unittest.TestCase):
    def test_projection_contract_is_available(self) -> None:
        self.assertIsNotNone(
            project_refresh_selection,
            "reviewed refresh acceptance projection is not implemented",
        )

    def test_application_contract_is_available(self) -> None:
        self.assertIsNotNone(
            build_refresh_application,
            "live-reconstructed refresh application is not implemented",
        )
        self.assertIsNotNone(
            apply_refresh_application,
            "compare-and-swap refresh application is not implemented",
        )

    def test_metadata_gate_registers_refresh_acceptance_tests(self) -> None:
        gate = Path("scripts/verification_metadata_gate.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "scripts.verification.code_spec_refresh_selection_tests",
            gate,
            "metadata gate does not execute refresh acceptance self-tests",
        )

    def test_complete_projection_transfers_rebinds_adds_and_removes(self) -> None:
        projection = self._project()
        for path, content in projection.replacements.items():
            self.assertTrue(content.endswith(b"\n"), path)
            self.assertFalse(content.endswith(b"\n\n"), path)
        mappings = {
            row["code_id"]: row
            for row in tomllib.loads(
                projection.replacements[
                    "verification/code-spec-map.toml"
                ].decode()
            )["mappings"]
        }
        self.assertEqual({SUCCESSOR, REBIND, DIRECT}, set(mappings))

        successor = mappings[SUCCESSOR]
        self.assertEqual("specified", successor["disposition"])
        self.assertEqual("SPEC_RUNTIME_ENGINE_001#restart", successor["spec_ref"])
        self.assertEqual(["RT_RESTART_001"], successor["invariant_ids"])
        self.assertEqual(["TEST_RESTART_001"], successor["test_ids"])
        self.assertEqual("sha256:" + "b" * 64, successor["reviewed_body_digest"])
        self.assertEqual(["state_mutation"], successor["risk_signals"])
        self.assertEqual(SUCCESSOR_ACK, successor["owner_ack"])

        rebind = mappings[REBIND]
        self.assertEqual("sha256:" + "d" * 64, rebind["reviewed_body_digest"])
        self.assertEqual(
            ["fault_mapping", "state_mutation"], rebind["risk_signals"]
        )
        self.assertEqual(REBIND_ACK, rebind["owner_ack"])

        direct = mappings[DIRECT]
        self.assertEqual("spec_gap", direct["disposition"])
        self.assertEqual(GAP, direct["spec_gap_ref"])
        self.assertEqual(DIRECT_ACK, direct["owner_ack"])

        acknowledgements = {
            row["code_id"]: row
            for row in tomllib.loads(
                projection.replacements[
                    "verification/code-spec-risk-acknowledgements.toml"
                ].decode()
            )["acknowledgements"]
        }
        self.assertEqual({SUCCESSOR, REBIND, DIRECT}, set(acknowledgements))
        self.assertEqual(
            "sha256:" + "d" * 64,
            acknowledgements[REBIND]["body_digest"],
        )
        self.assertEqual(
            ["fault_mapping", "state_mutation"],
            acknowledgements[REBIND]["risk_signals"],
        )
        self.assertEqual(
            "docs/review.md#identity-maintenance",
            acknowledgements[SUCCESSOR]["review_ref"],
        )

        gaps = tomllib.loads(
            projection.replacements["verification/spec-gaps.toml"].decode()
        )["spec_gaps"]
        self.assertEqual(
            [SUCCESSOR, REBIND, DIRECT],
            gaps[0]["affected_code_ids"],
        )

        self.assertEqual(1, projection.manifest["transferred_successors"])
        self.assertEqual(1, projection.manifest["removed_stale_identities"])
        self.assertEqual(1, projection.manifest["digest_rebinds"])
        self.assertEqual(1, projection.manifest["direct_missing_mappings"])

    def test_projection_rejects_partial_review_queue(self) -> None:
        proposal, selection, code_map, acknowledgements, gaps = self._inputs()
        selection["digest_rebind_proposal_ids"] = []
        with self.assertRaisesRegex(ValueError, "digest rebind review is partial"):
            self._call(
                proposal, selection, code_map, acknowledgements, gaps
            )

    def test_projection_rejects_nonexact_successor_transfer(self) -> None:
        proposal, selection, code_map, acknowledgements, gaps = self._inputs()
        selection["stale_identity_decisions"][0]["successor_code_id"] = DIRECT
        with self.assertRaisesRegex(ValueError, "exact successor"):
            self._call(
                proposal, selection, code_map, acknowledgements, gaps
            )

    def test_projection_transfers_successor_already_present_in_map(self) -> None:
        proposal, selection, code_map, acknowledgements, gaps = self._inputs()
        proposal["missing_mapping_proposals"] = [
            proposal["missing_mapping_proposals"][1]
        ]
        proposal["summary"]["missing_mapping_proposals"] = 1
        selection["missing_mapping_decisions"] = [
            selection["missing_mapping_decisions"][1]
        ]
        projection = self._call(
            proposal, selection, code_map, acknowledgements, gaps
        )
        mappings = {
            row["code_id"]: row
            for row in tomllib.loads(
                projection.replacements[
                    "verification/code-spec-map.toml"
                ].decode()
            )["mappings"]
        }
        self.assertEqual("specified", mappings[SUCCESSOR]["disposition"])
        self.assertEqual(
            "sha256:" + "b" * 64,
            mappings[SUCCESSOR]["reviewed_body_digest"],
        )

    def test_projection_rejects_unreviewed_area_choice(self) -> None:
        proposal, selection, code_map, acknowledgements, gaps = self._inputs()
        selection["missing_mapping_decisions"][1]["area"] = "editor_safety"
        with self.assertRaisesRegex(ValueError, "area is not a live candidate"):
            self._call(
                proposal, selection, code_map, acknowledgements, gaps
            )

    def test_application_rebuild_detects_live_reconstruction_drift(self) -> None:
        if build_refresh_application is None or apply_refresh_application is None:
            self.fail("live-reconstructed refresh application is not implemented")
        proposal, selection, code_map, acknowledgements, gaps = self._inputs()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            proposal_path, selection_path, output_path = _write_application_fixture(
                root,
                proposal,
                selection,
                code_map,
                acknowledgements,
                gaps,
            )
            calls = 0

            def reconstruct(_root: Path, _proposal: bytes) -> dict:
                nonlocal calls
                calls += 1
                return {"live_digest": f"sha256:{calls:064x}"}

            application = build_refresh_application(
                root=root,
                proposal_path=proposal_path,
                selection_path=selection_path,
                output_path=output_path,
                reconstruct_live=reconstruct,
            )
            with self.assertRaisesRegex(
                ValueError, "differs from canonical rebuilt inputs"
            ):
                apply_refresh_application(
                    root=root,
                    proposal_path=proposal_path,
                    selection_path=selection_path,
                    output_path=output_path,
                    application=application,
                    reconstruct_live=reconstruct,
                    post_validate=lambda _root: [],
                )
            self.assertEqual(code_map, (root / "verification/code-spec-map.toml").read_bytes())

    def test_application_atomically_writes_projection_and_manifest(self) -> None:
        if build_refresh_application is None or apply_refresh_application is None:
            self.fail("live-reconstructed refresh application is not implemented")
        proposal, selection, code_map, acknowledgements, gaps = self._inputs()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            proposal_path, selection_path, output_path = _write_application_fixture(
                root,
                proposal,
                selection,
                code_map,
                acknowledgements,
                gaps,
            )

            def reconstruct(_root: Path, _proposal: bytes) -> dict:
                return {"live_digest": "sha256:" + "9" * 64}

            application = build_refresh_application(
                root=root,
                proposal_path=proposal_path,
                selection_path=selection_path,
                output_path=output_path,
                reconstruct_live=reconstruct,
            )
            apply_refresh_application(
                root=root,
                proposal_path=proposal_path,
                selection_path=selection_path,
                output_path=output_path,
                application=application,
                reconstruct_live=reconstruct,
                post_validate=lambda _root: [],
            )
            self.assertEqual(
                application.projection.replacements[
                    "verification/code-spec-map.toml"
                ],
                (root / "verification/code-spec-map.toml").read_bytes(),
            )
            self.assertEqual(
                application.manifest_bytes, output_path.read_bytes()
            )

    def _project(self):
        return self._call(*self._inputs())

    def _call(
        self,
        proposal: dict,
        selection: dict,
        code_map: bytes,
        acknowledgements: bytes,
        gaps: bytes,
    ):
        if project_refresh_selection is None:
            self.fail("reviewed refresh acceptance projection is not implemented")
        proposal_bytes = _json_bytes(proposal)
        selection["source_proposal_sha256"] = _digest(proposal_bytes)
        return project_refresh_selection(
            proposal_bytes=proposal_bytes,
            selection_bytes=_json_bytes(selection),
            code_map_bytes=code_map,
            risk_acknowledgements_bytes=acknowledgements,
            spec_gaps_bytes=gaps,
        )

    def _inputs(self) -> tuple[dict, dict, bytes, bytes, bytes]:
        old = _mapping(
            OLD,
            body="a",
            risk=["state_mutation"],
            disposition="specified",
            owner_ack=OLD_ACK,
        )
        old.update(
            {
                "spec_ref": "SPEC_RUNTIME_ENGINE_001#restart",
                "invariant_ids": ["RT_RESTART_001"],
                "test_ids": ["TEST_RESTART_001"],
            }
        )
        successor = _mapping(
            SUCCESSOR,
            body="b",
            risk=["state_mutation"],
            disposition="spec_gap",
            owner_ack=SUCCESSOR_ACK,
        )
        successor["spec_gap_ref"] = GAP
        removed = _mapping(
            REMOVED,
            body="c",
            risk=[],
            disposition="spec_gap",
        )
        removed["spec_gap_ref"] = GAP
        rebind = _mapping(
            REBIND,
            body="4",
            risk=["state_mutation"],
            disposition="spec_gap",
            owner_ack=REBIND_ACK,
        )
        rebind["spec_gap_ref"] = GAP
        code_map = _toml_document(
            "mappings", [old, successor, removed, rebind]
        )

        acknowledgements = _toml_document(
            "acknowledgements",
            [
                _ack(OLD, OLD_ACK, "a", ["state_mutation"]),
                _ack(SUCCESSOR, SUCCESSOR_ACK, "b", ["state_mutation"]),
                _ack(REBIND, REBIND_ACK, "4", ["state_mutation"]),
            ],
        )
        gaps = _toml_document(
            "spec_gaps",
            [
                {
                    "schema_version": 1,
                    "id": GAP,
                    "title": "Runtime safety authority remains open",
                    "area": "runtime_safety",
                    "risk": "safety_critical",
                    "owner": "trust-runtime",
                    "status": "spec_gap",
                    "gap_class": "missing_behavior",
                    "blocking_question": "What behavior is authoritative?",
                    "affected_invariants": [],
                    "affected_tests": [],
                    "affected_code_ids": [
                        OLD,
                        SUCCESSOR,
                        REMOVED,
                        REBIND,
                    ],
                    "candidate_spec_sources": ["SPEC_RUNTIME_ENGINE_001"],
                    "resolution_status": "open",
                    "last_reviewed": "2026-07-21",
                }
            ],
        )

        successor_fact = _fact(SUCCESSOR, body="b", risk=["state_mutation"])
        rebind_fact = _fact(
            REBIND,
            body="d",
            risk=["fault_mapping", "state_mutation"],
        )
        direct_fact = _fact(
            DIRECT,
            body="e",
            risk=["persistence_retain_reload_restart"],
        )
        proposal = {
            "schema_version": 1,
            "generator": "code-spec-refresh-proposals",
            "generator_version": "1",
            "mode": "proposal_only",
            "source_observation_status": "untrusted_until_live_reconstruction",
            "source_commit": "a" * 40,
            "source_report": {
                "path": "target/gate-artifacts/code-spec-coverage/report.json",
                "sha256": "sha256:" + "1" * 64,
                "input_digest": "sha256:" + "2" * 64,
            },
            "source_code_map_sha256": _digest(code_map),
            "operation_set_digest": "sha256:" + "3" * 64,
            "boundaries": {
                "automatic_writes": False,
                "creates_product_proof": False,
                "closes_spec_gaps": False,
                "promotes_invariants": False,
                "infers_semantic_bindings": False,
                "source_facts_live_reconstructed": False,
            },
            "summary": {
                "source_facts": 3,
                "source_findings": 4,
                "partitioned_findings": 4,
                "missing_mapping_proposals": 2,
                "digest_rebind_proposals": 1,
                "stale_identity_proposals": 2,
                "semantic_review_groups": 0,
                "unclassified_findings": 0,
            },
            "missing_mapping_proposals": [
                _missing("MISSING_SUCCESSOR", successor_fact),
                _missing("MISSING_DIRECT", direct_fact),
            ],
            "digest_rebind_proposals": [
                {
                    "proposal_id": _proposal_id("REBIND"),
                    "kind": "body_digest_rebind",
                    "code_id": REBIND,
                    "fact": rebind_fact,
                    "existing_mapping_sha256": _object_digest(rebind),
                    "existing_disposition": "spec_gap",
                    "old_body_digest": "sha256:" + "4" * 64,
                    "new_body_digest": "sha256:" + "d" * 64,
                    "findings": [
                        "risk acknowledgement body digest changed",
                        "risk acknowledgement risk signals changed",
                        "stale_spec_review body digest changed",
                        "stale_spec_review risk_signals changed",
                    ],
                    "review_status": "review_required",
                    "required_reviewer_roles": [
                        "semantic_owner",
                        "risk_owner",
                    ],
                }
            ],
            "stale_identity_proposals": [
                {
                    "proposal_id": _proposal_id("STALE_TRANSFER"),
                    "kind": "identity_successor_review",
                    "stale_code_id": OLD,
                    "existing_mapping_sha256": _object_digest(old),
                    "path": "src/runtime.rs",
                    "qualified_symbol": "runtime::restart",
                    "item_kind": "python_function",
                    "candidate_match_basis": (
                        "exact_path_qualified_symbol_item_kind"
                    ),
                    "exact_successor_code_ids": [SUCCESSOR],
                    "has_owner_ack": True,
                    "findings": ["stale mapping identity"],
                    "review_status": "review_required",
                    "required_reviewer_roles": [
                        "semantic_owner",
                        "risk_owner",
                    ],
                },
                {
                    "proposal_id": _proposal_id("STALE_REMOVE"),
                    "kind": "identity_successor_review",
                    "stale_code_id": REMOVED,
                    "existing_mapping_sha256": _object_digest(removed),
                    "path": "src/runtime.rs",
                    "qualified_symbol": "runtime::removed",
                    "item_kind": "python_function",
                    "candidate_match_basis": (
                        "exact_path_qualified_symbol_item_kind"
                    ),
                    "exact_successor_code_ids": [],
                    "has_owner_ack": False,
                    "findings": ["stale mapping identity"],
                    "review_status": "review_required",
                    "required_reviewer_roles": ["semantic_owner"],
                },
            ],
            "semantic_review_groups": [],
            "unclassified_findings": [],
        }
        selection = {
            "schema_version": 1,
            "kind": "p18_code_spec_refresh_acceptance_selection",
            "reviewed_on": "2026-07-29",
            "reviewer_ids": ["semantic-owner", "risk-owner"],
            "review_ref": "docs/review.md#identity-maintenance",
            "risk_ack_expires_on": "2026-10-29",
            "source_proposal_sha256": "",
            "operation_set_digest": proposal["operation_set_digest"],
            "missing_mapping_decisions": [
                {
                    "proposal_id": _proposal_id("MISSING_SUCCESSOR"),
                    "decision": "stale_successor",
                    "stale_proposal_id": _proposal_id("STALE_TRANSFER"),
                },
                {
                    "proposal_id": _proposal_id("MISSING_DIRECT"),
                    "decision": "spec_gap",
                    "area": "runtime_safety",
                    "owner": "trust-runtime",
                    "spec_gap_ref": GAP,
                },
            ],
            "digest_rebind_proposal_ids": [_proposal_id("REBIND")],
            "stale_identity_decisions": [
                {
                    "proposal_id": _proposal_id("STALE_TRANSFER"),
                    "decision": "transfer",
                    "successor_code_id": SUCCESSOR,
                },
                {
                    "proposal_id": _proposal_id("STALE_REMOVE"),
                    "decision": "remove",
                },
            ],
        }
        return proposal, selection, code_map, acknowledgements, gaps


def _mapping(
    code_id: str,
    *,
    body: str,
    risk: list[str],
    disposition: str,
    owner_ack: str | None = None,
) -> dict:
    row = {
        "schema_version": 1,
        "code_id": code_id,
        "source_kind": "python",
        "path": "src/runtime.rs",
        "qualified_symbol": (
            "runtime::restart"
            if code_id in {OLD, SUCCESSOR}
            else f"runtime::{code_id[-4:]}"
        ),
        "item_kind": "python_function",
        "signature_digest": "sha256:" + code_id[-1].lower() * 64,
        "reviewed_body_digest": "sha256:" + body * 64,
        "owner": "trust-runtime",
        "area": "runtime_safety",
        "visibility": "private",
        "risk_signals": risk,
        "disposition": disposition,
        "last_reviewed": "2026-07-21",
    }
    if owner_ack is not None:
        row["owner_ack"] = owner_ack
        row["owner_ack_expires_on"] = "2026-10-21"
    return row


def _fact(
    code_id: str,
    *,
    body: str,
    risk: list[str],
) -> dict:
    return {
        "code_id": code_id,
        "source_kind": "python",
        "path": "src/runtime.rs",
        "qualified_symbol": (
            "runtime::restart"
            if code_id == SUCCESSOR
            else f"runtime::{code_id[-4:]}"
        ),
        "item_kind": "python_function",
        "signature_digest": "sha256:" + code_id[-1].lower() * 64,
        "body_digest": "sha256:" + body * 64,
        "owner": "trust-runtime",
        "area": "runtime_safety",
        "area_candidates": ["runtime_safety"],
        "visibility": "private",
        "risk_signals": risk,
        "line": 1,
        "observed_disposition": "missing_spec",
    }


def _missing(label: str, fact: dict) -> dict:
    return {
        "proposal_id": _proposal_id(label),
        "kind": "missing_mapping",
        "code_id": fact["code_id"],
        "fact": fact,
        "findings": ["missing_spec"],
        "review_status": "review_required",
        "required_reviewer_roles": ["semantic_owner"],
    }


def _ack(
    code_id: str,
    ack_id: str,
    body: str,
    risk: list[str],
) -> dict:
    return {
        "schema_version": 1,
        "id": ack_id,
        "code_id": code_id,
        "owner": "trust-runtime",
        "area": "runtime_safety",
        "body_digest": "sha256:" + body * 64,
        "risk_signals": risk,
        "review_ref": "docs/old.md#review",
        "reviewed_by": "trust-runtime",
        "reviewed_on": "2026-07-21",
        "expires_on": "2026-10-21",
        "rationale": "Old review.",
    }


def _proposal_id(label: str) -> str:
    return "CODEMAP_CAND_" + hashlib.sha256(label.encode()).hexdigest().upper()


def _digest(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _object_digest(value: dict) -> str:
    return _digest(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    )


def _write_application_fixture(
    root: Path,
    proposal: dict,
    selection: dict,
    code_map: bytes,
    acknowledgements: bytes,
    gaps: bytes,
) -> tuple[Path, Path, Path]:
    proposal_bytes = _json_bytes(proposal)
    selection["source_proposal_sha256"] = _digest(proposal_bytes)
    selection_bytes = _json_bytes(selection)
    paths = {
        "verification/code-spec-map.toml": code_map,
        "verification/code-spec-risk-acknowledgements.toml": acknowledgements,
        "verification/spec-gaps.toml": gaps,
        "target/gate-artifacts/code-spec-refresh/proposal.json": proposal_bytes,
        "target/gate-artifacts/code-spec-refresh/selection.json": selection_bytes,
    }
    for relative, content in paths.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    return (
        root / "target/gate-artifacts/code-spec-refresh/proposal.json",
        root / "target/gate-artifacts/code-spec-refresh/selection.json",
        root / "target/gate-artifacts/code-spec-refresh/application.json",
    )


def _json_bytes(value: dict) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode()


def _toml_document(key: str, rows: list[dict]) -> bytes:
    lines = ["schema_version = 1", ""]
    for row in rows:
        lines.append(f"[[{key}]]")
        for field, value in row.items():
            lines.append(f"{field} = {_toml_value(value)}")
        lines.append("")
    return ("\n".join(lines) + "\n").encode()


def _toml_value(value) -> str:
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, list):
        return "[" + ", ".join(_toml_value(item) for item in value) + "]"
    raise TypeError(type(value))


if __name__ == "__main__":
    unittest.main()
