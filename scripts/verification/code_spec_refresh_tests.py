"""Focused fail-closed tests for quarantined code-spec refresh proposals."""

from __future__ import annotations

import copy
import hashlib
import io
import json
import shutil
import subprocess
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path

from scripts.verification.code_spec_common import (
    CodeFact,
    sha256_digest,
    stable_code_id,
)
from scripts.verification.code_spec_refresh import (
    CodeSpecRefreshError,
    build_refresh_proposals,
    canonical_json,
    validate_refresh_proposals,
)
from scripts.verification.code_spec_refresh_cli import (
    _repo_path,
    _require_quarantine_output,
    main,
)
from scripts.verification.code_spec_provenance import input_closure_digest
from scripts.verification.code_spec_report import (
    CodeSpecDenominator,
    CodeSpecProvenance,
    build_report_payload,
    canonical_json as canonical_report_json,
    render_markdown,
)
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


SCHEMA_PATH = Path("verification/schemas/code-spec-refresh-proposals.schema.json")
CODE_A = stable_code_id(
    source_kind="python",
    path="scripts/verification/new_tool.py",
    qualified_symbol="scripts::verification::new_tool::build",
    item_kind="python_function",
    signature="def build()",
)
CODE_B = stable_code_id(
    source_kind="python",
    path="scripts/verification/existing.py",
    qualified_symbol="scripts::verification::existing::run",
    item_kind="python_function",
    signature="def run()",
)
CODE_C = stable_code_id(
    source_kind="python",
    path="scripts/verification/new_tool.py",
    qualified_symbol="scripts::verification::new_tool::build",
    item_kind="python_function",
    signature="def build(value)",
)
DIGEST_1 = "sha256:" + "1" * 64
DIGEST_2 = "sha256:" + "2" * 64
DIGEST_3 = "sha256:" + "3" * 64


def _digest(value: dict) -> str:
    encoded = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _rebind_generated_identity(payload: dict, section: str, index: int = 0) -> None:
    row = payload[section][index]
    id_field = (
        "review_group_id"
        if section == "semantic_review_groups"
        else "proposal_id"
    )
    prefix = "CODEMAP_GROUP" if id_field == "review_group_id" else "CODEMAP_CAND"
    body = {key: value for key, value in row.items() if key != id_field}
    row[id_field] = f"{prefix}_{_digest(body).removeprefix('sha256:').upper()}"
    operations = {
        key: payload[key]
        for key in (
            "missing_mapping_proposals",
            "digest_rebind_proposals",
            "stale_identity_proposals",
            "semantic_review_groups",
        )
    }
    payload["operation_set_digest"] = _digest(operations)


def _fact(
    code_id: str,
    *,
    path: str,
    symbol: str,
    disposition: str,
    findings: list[str],
) -> dict:
    signature = f"def {symbol.rsplit('::', 1)[-1]}()"
    return {
        "code_id": code_id,
        "source_kind": "python",
        "path": path,
        "qualified_symbol": symbol,
        "item_kind": "python_function",
        "signature": signature,
        "signature_digest": sha256_digest(signature.encode()),
        "body_digest": DIGEST_2,
        "owner": "verification",
        "area": "verification",
        "area_candidates": ["verification"],
        "visibility": "private",
        "risk_signals": [],
        "risk_acknowledgement": None,
        "risk_acknowledgement_status": "none",
        "cargo_bindings": [],
        "inventory_bindings": [],
        "line": 10,
        "disposition": disposition,
        "disposition_ref": None,
        "debt_status": "none",
        "findings": findings,
    }


def _report() -> dict:
    findings = [
        f"{CODE_A}: missing_spec",
        f"{CODE_A}: missing_spec new fact absent from map and frozen baseline",
        f"{CODE_B}: stale_spec_review body digest changed",
        f"{CODE_B}: invariant VERIFY_ONE crosses area without decision",
        f"{CODE_C}: invented_or_stale_code_identity",
        f"{CODE_C}: area binding names invented code identity",
    ]
    return {
        "schema_version": 1,
        "generator": "code-spec-coverage",
        "generator_version": 1,
        "report_status": "complete",
        "commit": "a" * 40,
        "input_digest": DIGEST_3,
        "input_paths": [
            "scripts/verification/existing.py",
            "scripts/verification/new_tool.py",
            "verification/code-spec-map.toml",
        ],
        "summary": {
            "facts_total": 2,
            "findings_total": len(findings),
        },
        "digest_rebinds": [
            {
                "code_id": CODE_B,
                "old_digest": DIGEST_1,
                "new_digest": DIGEST_2,
                "owner": "verification",
                "area": "verification",
                "owner_ack": None,
            }
        ],
        "findings": findings,
        "facts": [
            _fact(
                CODE_A,
                path="scripts/verification/new_tool.py",
                symbol="scripts::verification::new_tool::build",
                disposition="missing_spec",
                findings=[
                    "missing_spec",
                    "missing_spec new fact absent from map and frozen baseline",
                ],
            ),
            _fact(
                CODE_B,
                path="scripts/verification/existing.py",
                symbol="scripts::verification::existing::run",
                disposition="specified",
                findings=[
                    "stale_spec_review body digest changed",
                    "invariant VERIFY_ONE crosses area without decision",
                ],
            ),
        ],
    }


def _code_map() -> dict:
    return {
        "schema_version": 1,
        "mappings": [
            {
                "schema_version": 1,
                "code_id": CODE_B,
                "source_kind": "python",
                "path": "scripts/verification/existing.py",
                "qualified_symbol": "scripts::verification::existing::run",
                "item_kind": "python_function",
                "signature_digest": DIGEST_1,
                "reviewed_body_digest": DIGEST_1,
                "owner": "verification",
                "area": "verification",
                "visibility": "private",
                "risk_signals": [],
                "disposition": "specified",
                "spec_ref": "SPEC_ONE#behavior",
                "invariant_ids": ["VERIFY_ONE"],
                "test_ids": ["TEST_ONE"],
                "last_reviewed": "2026-07-24",
            },
            {
                "schema_version": 1,
                "code_id": CODE_C,
                "source_kind": "python",
                "path": "scripts/verification/new_tool.py",
                "qualified_symbol": "scripts::verification::new_tool::build",
                "item_kind": "python_function",
                "signature_digest": DIGEST_1,
                "reviewed_body_digest": DIGEST_1,
                "owner": "verification",
                "area": "verification",
                "visibility": "private",
                "risk_signals": [],
                "disposition": "spec_gap",
                "spec_gap_ref": "SPEC_GAP_ONE",
                "last_reviewed": "2026-07-24",
            },
        ],
    }


class CodeSpecRefreshTests(unittest.TestCase):
    def build(self, report: dict | None = None, code_map: dict | None = None) -> dict:
        return build_refresh_proposals(
            report=report or _report(),
            source_report_path="target/frozen/code-spec-coverage.json",
            source_report_sha256=DIGEST_1,
            code_map=code_map or _code_map(),
            code_map_sha256=DIGEST_2,
        )

    def test_payload_is_closed_quarantine_and_partitions_every_finding(self) -> None:
        payload = self.build()
        schema = json.loads(SCHEMA_PATH.read_text())

        self.assertEqual([], validate_json_schema_instance(payload, schema))
        self.assertEqual("proposal_only", payload["mode"])
        self.assertFalse(payload["boundaries"]["automatic_writes"])
        self.assertFalse(payload["boundaries"]["creates_product_proof"])
        self.assertFalse(payload["boundaries"]["closes_spec_gaps"])
        self.assertFalse(payload["boundaries"]["infers_semantic_bindings"])
        self.assertEqual(6, payload["summary"]["source_findings"])
        self.assertEqual(6, payload["summary"]["partitioned_findings"])
        self.assertEqual([], payload["unclassified_findings"])
        self.assertEqual(1, len(payload["missing_mapping_proposals"]))
        self.assertEqual(1, len(payload["digest_rebind_proposals"]))
        self.assertEqual(1, len(payload["stale_identity_proposals"]))
        self.assertEqual(1, len(payload["semantic_review_groups"]))

    def test_generation_is_deterministic_and_canonical(self) -> None:
        first = self.build()
        reversed_report = copy.deepcopy(_report())
        reversed_report["facts"].reverse()
        reversed_report["findings"].reverse()
        reversed_report["digest_rebinds"].reverse()
        second = self.build(reversed_report)

        self.assertEqual(canonical_json(first), canonical_json(second))
        self.assertEqual(
            canonical_json(first),
            canonical_json(json.loads(canonical_json(first))),
        )

    def test_summary_count_drift_fails_closed(self) -> None:
        report = _report()
        report["summary"]["facts_total"] += 1

        with self.assertRaisesRegex(CodeSpecRefreshError, "facts_total"):
            self.build(report)

    def test_duplicate_live_identity_fails_closed(self) -> None:
        report = _report()
        report["facts"].append(copy.deepcopy(report["facts"][0]))
        report["summary"]["facts_total"] += 1

        with self.assertRaisesRegex(CodeSpecRefreshError, "duplicate live code identity"):
            self.build(report)

    def test_reported_fact_identity_must_derive_from_exact_tuple(self) -> None:
        report = _report()
        report["facts"][0]["path"] = "scripts/verification/invented.py"
        report["input_paths"].insert(1, "scripts/verification/invented.py")

        with self.assertRaisesRegex(CodeSpecRefreshError, "identity does not derive"):
            self.build(report)

    def test_unknown_finding_fails_closed_instead_of_disappearing(self) -> None:
        report = _report()
        report["findings"].append(f"{CODE_B}: novel unchecked condition")
        report["summary"]["findings_total"] += 1

        with self.assertRaisesRegex(CodeSpecRefreshError, "unclassified finding"):
            self.build(report)

    def test_stale_successor_requires_exact_path_symbol_and_kind(self) -> None:
        payload = self.build()
        stale = payload["stale_identity_proposals"][0]
        self.assertEqual([CODE_A], stale["exact_successor_code_ids"])

        changed_map = _code_map()
        changed_map["mappings"][1]["qualified_symbol"] = (
            "scripts::verification::new_tool::different"
        )
        payload = self.build(code_map=changed_map)
        self.assertEqual(
            [],
            payload["stale_identity_proposals"][0]["exact_successor_code_ids"],
        )

    def test_risk_only_change_routes_to_explicit_risk_review(self) -> None:
        report = _report()
        stale_body = f"{CODE_B}: stale_spec_review body digest changed"
        risk_change = f"{CODE_B}: stale_spec_review risk_signals changed"
        report["findings"].remove(stale_body)
        report["findings"].append(risk_change)
        report["facts"][1]["findings"].remove(
            "stale_spec_review body digest changed"
        )
        report["facts"][1]["findings"].append(
            "stale_spec_review risk_signals changed"
        )
        report["digest_rebinds"] = []

        payload = self.build(report)

        self.assertEqual([], payload["digest_rebind_proposals"])
        risk_group = next(
            row
            for row in payload["semantic_review_groups"]
            if row["kind"] == "risk_signal_review"
        )
        self.assertEqual([CODE_B], risk_group["affected_code_ids"])
        self.assertEqual(
            ["semantic_owner", "risk_owner"],
            risk_group["required_reviewer_roles"],
        )

    def test_schema_rejects_proof_or_automatic_acceptance_fields(self) -> None:
        payload = self.build()
        schema = json.loads(SCHEMA_PATH.read_text())
        forged = copy.deepcopy(payload)
        forged["proof_level"] = "T1"
        forged["missing_mapping_proposals"][0]["accepted"] = True

        self.assertTrue(validate_json_schema_instance(forged, schema))

    def test_schema_accepts_exact_source_report_dispositions(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text())
        canonical = (
            "specified",
            "spec_gap",
            "inherits_spec",
            "non_behavioral",
            "generated",
            "vendored_external",
            "missing_spec",
        )
        for disposition in canonical:
            payload = self.build()
            payload["missing_mapping_proposals"][0]["fact"][
                "observed_disposition"
            ] = disposition
            with self.subTest(disposition=disposition):
                self.assertEqual(
                    [],
                    validate_json_schema_instance(payload, schema),
                )
        for stale_value in ("inherits", "nonbehavioral", "vendored"):
            payload = self.build()
            payload["missing_mapping_proposals"][0]["fact"][
                "observed_disposition"
            ] = stale_value
            with self.subTest(stale_value=stale_value):
                self.assertTrue(validate_json_schema_instance(payload, schema))

    def test_at_rest_validation_returns_failures_for_malformed_rows(self) -> None:
        payload = self.build()
        payload["missing_mapping_proposals"] = [7]

        failures = validate_refresh_proposals(
            payload,
            source_report=_report(),
            source_report_path="target/frozen/code-spec-coverage.json",
            source_report_sha256=DIGEST_1,
            source_code_map=_code_map(),
            source_code_map_sha256=DIGEST_2,
        )

        self.assertEqual(
            [
                "refresh proposal rows are malformed: "
                "missing_mapping_proposals"
            ],
            failures,
        )

    def test_at_rest_validation_rejects_count_id_and_digest_tampering(self) -> None:
        payload = self.build()
        self.assertEqual(
            [],
            validate_refresh_proposals(
                payload,
                source_report=_report(),
                source_report_path="target/frozen/code-spec-coverage.json",
                source_report_sha256=DIGEST_1,
                source_code_map=_code_map(),
                source_code_map_sha256=DIGEST_2,
            ),
        )

        mutations = []
        wrong_count = copy.deepcopy(payload)
        wrong_count["summary"]["partitioned_findings"] -= 1
        mutations.append(wrong_count)
        wrong_id = copy.deepcopy(payload)
        wrong_id["missing_mapping_proposals"][0]["proposal_id"] = (
            "CODEMAP_CAND_" + "F" * 64
        )
        mutations.append(wrong_id)
        wrong_digest = copy.deepcopy(payload)
        wrong_digest["operation_set_digest"] = "sha256:" + "f" * 64
        mutations.append(wrong_digest)
        wrong_binding = copy.deepcopy(payload)
        wrong_binding["missing_mapping_proposals"][0]["code_id"] = CODE_B
        mutations.append(wrong_binding)

        for mutated in mutations:
            with self.subTest(mutated=mutated["summary"]):
                self.assertTrue(
                    validate_refresh_proposals(
                        mutated,
                        source_report=_report(),
                        source_report_path="target/frozen/code-spec-coverage.json",
                        source_report_sha256=DIGEST_1,
                        source_code_map=_code_map(),
                        source_code_map_sha256=DIGEST_2,
                    )
                )

    def test_at_rest_validation_rejects_recomputed_finding_tampering(self) -> None:
        payload = self.build()
        payload["missing_mapping_proposals"][0]["findings"][0] = (
            "invented replacement finding"
        )
        _rebind_generated_identity(payload, "missing_mapping_proposals")

        failures = validate_refresh_proposals(
            payload,
            source_report=_report(),
            source_report_path="target/frozen/code-spec-coverage.json",
            source_report_sha256=DIGEST_1,
            source_code_map=_code_map(),
            source_code_map_sha256=DIGEST_2,
        )

        self.assertIn(
            "proposal finding partition differs from exact source report",
            failures,
        )

    def test_at_rest_validation_rejects_recomputed_row_tampering(self) -> None:
        payload = self.build()
        payload["missing_mapping_proposals"][0]["fact"]["path"] = (
            "scripts/verification/invented.py"
        )
        _rebind_generated_identity(payload, "missing_mapping_proposals")

        failures = validate_refresh_proposals(
            payload,
            source_report=_report(),
            source_report_path="target/frozen/code-spec-coverage.json",
            source_report_sha256=DIGEST_1,
            source_code_map=_code_map(),
            source_code_map_sha256=DIGEST_2,
        )

        self.assertIn(
            "proposal payload differs from exact source reconstruction",
            failures,
        )

    def test_at_rest_validation_rejects_recomputed_role_tampering(self) -> None:
        report = _report()
        risk_finding = "risk acknowledgement body digest changed"
        report["findings"].append(f"{CODE_B}: {risk_finding}")
        report["facts"][1]["findings"].append(risk_finding)
        report["summary"]["findings_total"] += 1
        payload = self.build(report)
        payload["digest_rebind_proposals"][0]["required_reviewer_roles"] = [
            "semantic_owner"
        ]
        _rebind_generated_identity(payload, "digest_rebind_proposals")

        failures = validate_refresh_proposals(
            payload,
            source_report=report,
            source_report_path="target/frozen/code-spec-coverage.json",
            source_report_sha256=DIGEST_1,
            source_code_map=_code_map(),
            source_code_map_sha256=DIGEST_2,
        )

        self.assertTrue(
            any("reviewer roles differ from digest-rebind policy" in row for row in failures)
        )

    def test_cli_paths_stay_inside_the_quarantined_artifact_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / "source.json"
            source.write_text("{}")
            artifact_root = root / "target/gate-artifacts"
            artifact_root.mkdir(parents=True)
            output = _repo_path(
                root,
                "target/gate-artifacts/code-spec-refresh/proposals.json",
                must_exist=False,
            )
            _require_quarantine_output(root, output, inputs=(source,))

            with self.assertRaisesRegex(CodeSpecRefreshError, "quarantine"):
                _require_quarantine_output(
                    root,
                    root / "verification/proposals.json",
                    inputs=(source,),
                )
            with self.assertRaisesRegex(CodeSpecRefreshError, "quarantine"):
                _require_quarantine_output(
                    root,
                    root / "target/gate-artifacts/other/proposals.json",
                    inputs=(source,),
                )
            with self.assertRaisesRegex(CodeSpecRefreshError, "file below"):
                _require_quarantine_output(
                    root,
                    root / "target/gate-artifacts/code-spec-refresh",
                    inputs=(source,),
                )
            with self.assertRaisesRegex(CodeSpecRefreshError, "overwrite an input"):
                _require_quarantine_output(root, source, inputs=(source,))
            with self.assertRaisesRegex(CodeSpecRefreshError, "escapes"):
                _repo_path(root, "../outside.json", must_exist=False)
            with self.assertRaisesRegex(CodeSpecRefreshError, "relative"):
                _repo_path(root, str(source), must_exist=True)

            symlink = root / "linked"
            symlink.symlink_to(artifact_root, target_is_directory=True)
            with self.assertRaisesRegex(CodeSpecRefreshError, "symlink"):
                _repo_path(root, "linked/proposals.json", must_exist=False)

    def test_cli_binds_frozen_commit_bytes_and_preserves_output_on_drift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            schemas = root / "verification/schemas"
            schemas.mkdir(parents=True)
            shutil.copy(
                "verification/schemas/code-spec-coverage-report.schema.json",
                schemas,
            )
            shutil.copy(SCHEMA_PATH, schemas)
            code_map = root / "verification/code-spec-map.toml"
            code_map.write_text("schema_version = 1\n\nmappings = []\n")
            source = root / "scripts/verification/new_tool.py"
            source.parent.mkdir(parents=True)
            source.write_text("def build():\n    pass\n")
            (root / ".gitignore").write_text("target/\n")
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(
                ["git", "-C", str(root), "add", ".gitignore", "verification"],
                check=True,
            )
            subprocess.run(
                ["git", "-C", str(root), "add", "scripts/verification/new_tool.py"],
                check=True,
            )
            subprocess.run(
                [
                    "git",
                    "-C",
                    str(root),
                    "-c",
                    "user.name=Code Spec Refresh Test",
                    "-c",
                    "user.email=code-spec-refresh@example.invalid",
                    "commit",
                    "-qm",
                    "fixture",
                ],
                check=True,
            )
            commit = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            report_json_path = (
                root / "target/gate-artifacts/source/code-spec-coverage.json"
            )
            report_markdown_path = report_json_path.with_suffix(".md")
            report_json_path.parent.mkdir(parents=True)
            fact = CodeFact(
                code_id=CODE_A,
                source_kind="python",
                path="scripts/verification/new_tool.py",
                qualified_symbol="scripts::verification::new_tool::build",
                item_kind="python_function",
                signature="def build()",
                signature_digest=sha256_digest(b"def build()"),
                body_digest=DIGEST_2,
                owner="verification",
                area="verification",
                area_candidates=("verification",),
                line=1,
            )
            report = build_report_payload(
                facts={CODE_A: fact},
                mappings={},
                failures=(f"{CODE_A}: missing_spec",),
                provenance=CodeSpecProvenance(
                    commit=commit,
                    timestamp="2026-07-24T00:00:00Z",
                    platform="test",
                    input_paths=(
                        "scripts/verification/new_tool.py",
                        "verification/code-spec-map.toml",
                    ),
                    output_json=report_json_path.relative_to(root).as_posix(),
                    output_markdown=report_markdown_path.relative_to(root).as_posix(),
                ),
                input_digest=input_closure_digest(
                    root,
                    (
                        "scripts/verification/new_tool.py",
                        "verification/code-spec-map.toml",
                    ),
                ),
                denominator=CodeSpecDenominator({"python": 1}, {}),
            )
            report_bytes = canonical_report_json(report).encode()
            report_json_path.write_bytes(report_bytes)
            report_markdown_path.write_text(
                render_markdown(
                    report,
                    json_digest=hashlib.sha256(report_bytes).hexdigest(),
                )
            )
            output = (
                root
                / "target/gate-artifacts/code-spec-refresh/proposals.json"
            )
            arguments = [
                "--root",
                str(root),
                "--report-json",
                report_json_path.relative_to(root).as_posix(),
                "--report-markdown",
                report_markdown_path.relative_to(root).as_posix(),
                "--output",
                output.relative_to(root).as_posix(),
            ]

            self.assertEqual(0, main(arguments))
            first_output = output.read_bytes()
            generated = json.loads(first_output)
            self.assertEqual(1, len(generated["missing_mapping_proposals"]))

            code_map.write_text("schema_version = 1\n\nmappings = []\n# drift\n")
            with redirect_stderr(io.StringIO()):
                self.assertEqual(1, main(arguments))
            self.assertEqual(first_output, output.read_bytes())


if __name__ == "__main__":
    unittest.main()
