"""Focused tests for the Phase 13 release-evidence audit."""

from __future__ import annotations

import copy
import json
import tomllib
import unittest
from pathlib import Path

from scripts.verification.phase13_release import (
    BOUNDARIES, LIMITATIONS, MANIFEST_PATH, MANIFEST_SCHEMA_PATH,
    PLATFORM_IDS, PROOF_ORIGINS, REPORT_SCHEMA_PATH,
    build_payload, canonical_json, render_markdown, validate_manifest,
)
from scripts.verification.phase13_release_live import _proof_origin_rows
from scripts.verification.phase13_release_validation import validate_payload, validate_schema
from scripts.verification.metadata_validator.core import Validator
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


class Phase13ReleaseTests(unittest.TestCase):
    def setUp(self) -> None:
        root = Path.cwd()
        self.manifest = tomllib.loads((root / MANIFEST_PATH).read_text(encoding="utf-8"))
        self.manifest_schema = json.loads((root / MANIFEST_SCHEMA_PATH).read_text(encoding="utf-8"))
        self.report_schema = json.loads((root / REPORT_SCHEMA_PATH).read_text(encoding="utf-8"))
        snapshot = self.manifest["latest_public_snapshot"]
        public = {
            **snapshot,
            "required_assets": list(self.manifest["required_release_assets"]),
            "missing_required_assets": ["conformance-status.json", "conformance-status.md", "release-provenance.json"],
            "matches_candidate": False,
        }
        platforms = []
        version = snapshot["tag"].removeprefix("v")
        for row in self.manifest["platforms"]:
            expected = [row["runtime_asset"], row["lsp_asset"], row["vsix_asset_template"].format(version=version)]
            platforms.append({**row, "snapshot_tag": snapshot["tag"], "expected_public_assets": expected, "public_assets_present": True})
        self.payload = build_payload(
            commit="a" * 40,
            branch="plc-verification-program",
            timestamp="2026-07-19T03:00:00+02:00",
            platform="linux-aarch64",
            input_paths=["Cargo.toml"],
            input_digest="sha256:" + "b" * 64,
            output_json="target/gate-artifacts/verification/phase13-release-evidence.json",
            output_markdown="docs/internal/testing/evidence/plc-verification-program/2026-07-19/phase13-release-evidence.md",
            command=[
                "python3", "scripts/report_phase13_release_evidence.py", "--json-out",
                "target/gate-artifacts/verification/phase13-release-evidence.json",
                "--markdown-out", "docs/internal/testing/evidence/plc-verification-program/2026-07-19/phase13-release-evidence.md",
                "--branch", "plc-verification-program", "--timestamp", "2026-07-19T03:00:00+02:00",
            ],
            candidate={
                "version": "0.24.54", "expected_tag": "v0.24.54",
                "version_sources": list(self.manifest["version_sources"]),
                "versions_synchronized": True, "changelog_mentions_version": True,
                "annotated_tag_present": False, "release_complete": False,
            },
            public_release=public,
            proof_origins=[
                {"origin": origin, "evidence_count": 1 if origin == "local" else 0,
                 "status": "recorded" if origin == "local" else ("snapshot_only" if origin == "public_github" else "missing"),
                 "limitation": f"{origin} limitation"}
                for origin in PROOF_ORIGINS
            ],
            security={
                "owned_exceptions": 7, "expired_exceptions": 0, "maximum_exception_days": 90,
                "cargo_policy_configured": True, "npm_audit_configured": True,
                "rust_commands": list(self.manifest["security_policy"]["rust_commands"]),
                "node_commands": list(self.manifest["security_policy"]["node_commands"]),
                "gate_execution_claimed": False,
            },
            platforms=platforms,
            conformance={"catalog_cases": 21, "linked_cases": 21, "missing_links": [], "public_asset_present": False, "execution_claimed": False},
            hardware_labs=[{"board_row": row, "status": "skipped_unproven", "evidence_count": 0} for row in self.manifest["hardware_lab_rows"]],
            ui_acceptance={"journeys": 30, "accepted_journeys": 0, "provisional_journeys": 1, "missing_journeys": 29, "stale_journeys": 0},
            known_gaps=[{"id": "CANDIDATE_PUBLICATION", "status": "open", "detail": "candidate is not public"}],
        )

    def test_manifest_and_report_schemas_are_closed_and_bound(self) -> None:
        self.assertEqual(validate_manifest(self.manifest), [])
        self.assertEqual(validate_json_schema_instance(self.manifest, self.manifest_schema), [])
        self.assertEqual(validate_schema(self.report_schema), [])
        self.assertEqual(validate_json_schema_instance(self.payload, self.report_schema), [])
        self.assertEqual(validate_payload(self.payload), [])

    def test_manifest_rejects_native_proof_on_artifact_only_target(self) -> None:
        tampered = copy.deepcopy(self.manifest)
        tampered["platforms"][1]["required_proof"].append("native_ci_test")
        self.assertIn("artifact-only tier claims native CI proof", "\n".join(validate_manifest(tampered)))

    def test_primary_metadata_validator_rejects_manifest_drift(self) -> None:
        validator = Validator()
        validator.load_records()
        validator.release_evidence_manifest["proof_origins"] = ["local"]
        validator.validate()
        messages = "\n".join(failure.message for failure in validator.failures)
        self.assertIn("proof origins drift", messages)

    def test_release_completion_requires_tag_latest_and_assets(self) -> None:
        tampered = copy.deepcopy(self.payload)
        tampered["candidate"]["release_complete"] = True
        self.assertIn("release_complete is not derived", "\n".join(validate_payload(tampered)))

        tampered = copy.deepcopy(self.payload)
        tampered["public_release"]["missing_required_assets"] = []
        self.assertIn("missing asset list is not derived", "\n".join(validate_payload(tampered)))

    def test_configured_gates_and_skips_cannot_be_promoted_to_proof(self) -> None:
        tampered = copy.deepcopy(self.payload)
        tampered["security"]["gate_execution_claimed"] = True
        self.assertIn("cannot claim gate execution", "\n".join(validate_payload(tampered)))

        tampered = copy.deepcopy(self.payload)
        tampered["proof_origins"][2]["status"] = "recorded"
        self.assertIn("claims recorded without evidence", "\n".join(validate_payload(tampered)))

        tampered = copy.deepcopy(self.payload)
        tampered["hardware_labs"][0]["status"] = "passed"
        self.assertIn("explicit skipped/unproven", "\n".join(validate_payload(tampered)))

    def test_proof_origin_classification_keeps_origins_distinct(self) -> None:
        evidence = [
            {"platform": "local-linux-aarch64", "kind": "committed_file"},
            {"platform": "trust-builder-linux-x86_64", "kind": "committed_file"},
            {"platform": "github-actions-linux-x64", "kind": "ci_artifact"},
            {"platform": "hardware-lab", "kind": "lab_report"},
            {"platform": "github", "kind": "release_object"},
        ]
        rows = _proof_origin_rows(evidence, self.manifest["latest_public_snapshot"])
        self.assertEqual([row["origin"] for row in rows], list(PROOF_ORIGINS))
        self.assertEqual([row["evidence_count"] for row in rows], [1, 1, 1, 1, 1])

    def test_markdown_is_stable_after_canonical_round_trip(self) -> None:
        round_tripped = json.loads(canonical_json(self.payload))
        self.assertEqual(
            render_markdown(self.payload, json_digest="c" * 64),
            render_markdown(round_tripped, json_digest="c" * 64),
        )
        self.assertEqual(self.payload["boundaries"], BOUNDARIES)
        self.assertEqual(self.payload["limitations"], list(LIMITATIONS))
        self.assertEqual([row["id"] for row in self.payload["platforms"]], list(PLATFORM_IDS))


if __name__ == "__main__":
    unittest.main()
