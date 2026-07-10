"""Tests for the Phase 4 invariant-seed import audit."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import tempfile
import textwrap
import tomllib
import unittest
from pathlib import Path

from .invariant_seed_contract import (
    P4_000_SEED_IDS,
    extract_written_seeds,
    load_seed_audit,
    validate_seed_records,
)
from .invariant_seed_live import build_live_seed_audit_state
from .invariant_seed_report import SeedAuditProvenance, SeedAuditReport, write_reports
from .test_catalog_json_schema import validate_json_schema_instance
from .invariant_seed_validation import (
    validate_report_files,
    validate_report_payload,
    validate_schema_contract,
)


ROOT = Path(__file__).resolve().parents[2]
AREAS_PATH = ROOT / "docs/internal/testing/checklists/plc-verification-program/verification-areas.md"
MANIFEST_SCHEMA_PATH = ROOT / "verification/schemas/invariant-seed-manifest.schema.json"
REPORT_SCHEMA_PATH = ROOT / "verification/schemas/invariant-seed-audit-report.schema.json"


class WrittenSeedExtractionTests(unittest.TestCase):
    def test_repository_source_contains_exactly_44_unique_written_seeds(self) -> None:
        seeds = extract_written_seeds(AREAS_PATH.read_text())
        self.assertEqual(44, len(seeds))
        self.assertEqual(44, len({row.seed_id for row in seeds}))
        self.assertEqual("RT_SAFE_PANIC_001", seeds[0].seed_id)
        self.assertEqual("REL_VERSION_001", seeds[-1].seed_id)
        self.assertEqual(
            {
                "IEC_TIMER_001",
                "RT_SAFE_NAN_001",
                "SEC_AUTHZ_001",
                "PROTO_OPCUA_001",
                "RT_RELOAD_001",
            },
            P4_000_SEED_IDS,
        )

    def test_duplicate_or_malformed_seed_source_fails_closed(self) -> None:
        text = AREAS_PATH.read_text()
        duplicated = text.replace(
            "- [x] `REL_VERSION_001`",
            "- [x] `RT_SAFE_PANIC_001` duplicate\n- [x] `REL_VERSION_001`",
        )
        with self.assertRaisesRegex(ValueError, "duplicate written seed"):
            extract_written_seeds(duplicated)

        malformed = text.replace("`REL_VERSION_001`", "REL_VERSION_001", 1)
        with self.assertRaisesRegex(ValueError, "malformed invariant seed checklist row"):
            extract_written_seeds(malformed)

        unchecked = text.replace("- [x] `RT_SAFE_PANIC_001`", "- [ ] `RT_SAFE_PANIC_001`")
        self.assertEqual(44, len(extract_written_seeds(unchecked)))


class SeedManifestContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tempdir.name)
        _write_fixture(self.root)

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def test_valid_fixture_maps_every_seed_and_only_permitted_merge(self) -> None:
        audit = load_seed_audit(self.root)
        self.assertEqual(44, len(audit.rows))
        self.assertEqual(43, len({row.canonical_invariant_id for row in audit.rows}))
        merged = [
            row.seed_id
            for row in audit.rows
            if row.canonical_invariant_id == "VM_SEAM_DECLARED_TYPE_001"
        ]
        self.assertEqual(["VM_SEAM_TYPE_001", "VM_SEAM_TYPE_002"], merged)
        self.assertEqual(5, sum(row.p4_000_risk_id is not None for row in audit.rows))

    def test_missing_seed_duplicate_mapping_and_unapproved_merge_fail(self) -> None:
        manifest = _load_manifest(self.root)
        manifest["seeds"].pop()
        _write_manifest(self.root, manifest)
        with self.assertRaisesRegex(ValueError, "fewer than 44|manifest seed IDs do not exactly match"):
            load_seed_audit(self.root)

        _write_fixture(self.root)
        manifest = _load_manifest(self.root)
        manifest["seeds"][1]["seed_id"] = manifest["seeds"][0]["seed_id"]
        _write_manifest(self.root, manifest)
        with self.assertRaisesRegex(ValueError, "manifest seed IDs do not exactly match"):
            load_seed_audit(self.root)

        _write_fixture(self.root)
        manifest = _load_manifest(self.root)
        manifest["seeds"][1]["canonical_invariant_id"] = manifest["seeds"][0][
            "canonical_invariant_id"
        ]
        _write_manifest(self.root, manifest)
        with self.assertRaisesRegex(ValueError, "canonical invariant must be|canonical invariant is mapped by multiple seeds"):
            load_seed_audit(self.root)

    def test_canonical_path_area_origin_and_unproven_state_are_enforced(self) -> None:
        row = _first_phase4_row(self.root)
        invariant_path = self.root / "verification/invariants" / row["area"] / f"{row['id']}.toml"
        text = invariant_path.read_text()
        invariant_path.write_text(text.replace('status = "gap_open"', 'status = "validated"'))
        with self.assertRaisesRegex(ValueError, "status must be gap_open or spec_gap"):
            load_seed_audit(self.root)

        _write_fixture(self.root)
        manifest = _load_manifest(self.root)
        phase4 = next(item for item in manifest["seeds"] if item["origin"] == "phase4")
        phase4["origin"] = "preexisting"
        _write_manifest(self.root, manifest)
        with self.assertRaisesRegex(ValueError, "origin does not match the reviewed preexisting set"):
            load_seed_audit(self.root)

        _write_fixture(self.root)
        row = _first_phase4_row(self.root)
        invariant_path = self.root / "verification/invariants" / row["area"] / f"{row['id']}.toml"
        invariant_path.write_text(
            invariant_path.read_text().replace("tests = []", 'tests = ["TEST_PREMATURE"]')
        )
        with self.assertRaisesRegex(ValueError, "phase4 seed must start with empty tests"):
            load_seed_audit(self.root)

    def test_gap_open_requires_active_non_claim_oracle_source(self) -> None:
        sources = self.root / "verification/spec-sources.toml"
        sources.write_text(sources.read_text().replace('authority = "normative_product"', 'authority = "public_claim"'))
        with self.assertRaisesRegex(ValueError, "gap_open oracle must use an active normative or reviewed source"):
            load_seed_audit(self.root)

        _write_fixture(self.root)
        sources = self.root / "verification/spec-sources.toml"
        sources.write_text(
            sources.read_text().replace(
                "oracle_eligible = true",
                "oracle_eligible = false",
            )
        )
        with self.assertRaisesRegex(ValueError, "gap_open oracle source is provenance-only"):
            load_seed_audit(self.root)

    def test_spec_gap_requires_open_focused_gap_in_oracle_and_coverage(self) -> None:
        gap_path = self.root / "verification/spec-gaps.toml"
        gap_path.write_text(gap_path.read_text().replace('resolution_status = "open"', 'resolution_status = "closed"'))
        with self.assertRaisesRegex(ValueError, "spec gap must remain open"):
            load_seed_audit(self.root)

        _write_fixture(self.root)
        invariant_path = self.root / "verification/invariants/runtime_safety/RT_SAFE_STOP_001.toml"
        invariant_path.write_text(
            invariant_path.read_text().replace('spec_gap_ref = "SPEC_GAP_FIXTURE_001"', 'spec_gap_ref = "SPEC_GAP_OTHER_001"')
        )
        with self.assertRaisesRegex(ValueError, "coverage spec_gap_ref"):
            load_seed_audit(self.root)

    def test_preexisting_associations_must_remain_non_closing(self) -> None:
        evidence = self.root / "verification/evidence-index.toml"
        evidence.write_text(evidence.read_text().replace('proof_kind = "none"', 'proof_kind = "green"'))
        with self.assertRaisesRegex(ValueError, "evidence must use proof_kind none"):
            load_seed_audit(self.root)

        _write_fixture(self.root)
        catalog = self.root / "verification/test-catalog.toml"
        catalog.write_text(catalog.read_text().replace('spec_gap_ref = "SPEC_GAP_FIXTURE_001"', 'spec_gap_ref = "SPEC_GAP_OTHER_001"'))
        with self.assertRaisesRegex(ValueError, "test must remain bound to an open invariant spec gap"):
            load_seed_audit(self.root)

    def test_exact_five_p4_000_review_risks_are_bidirectionally_linked(self) -> None:
        manifest = _load_manifest(self.root)
        target = next(item for item in manifest["seeds"] if item["seed_id"] in P4_000_SEED_IDS)
        target["p4_000_risk_id"] = None
        _write_manifest(self.root, manifest)
        with self.assertRaisesRegex(ValueError, "P4-000 seed requires a review risk|P4-000 risk links do not match"):
            load_seed_audit(self.root)

        _write_fixture(self.root)
        risks = self.root / "verification/risk-register.toml"
        risks.write_text(risks.read_text().replace('related_invariants = ["RT_SAFE_NAN_001"]', "related_invariants = []"))
        with self.assertRaisesRegex(ValueError, "risk must link back to canonical invariant"):
            load_seed_audit(self.root)

    def test_in_memory_validator_catches_corrupted_loaded_manifest(self) -> None:
        arguments = _loaded_contract_arguments(self.root)
        records = copy.deepcopy(arguments["seed_records"])
        records[0]["origin"] = "preexisting"
        failures = validate_seed_records(**{**arguments, "seed_records": records})
        self.assertEqual(1, len(failures))
        self.assertIn("origin does not match", failures[0])

    def test_gap_open_review_source_must_be_git_tracked(self) -> None:
        untracked = self.root / "fixture/untracked-review.md"
        untracked.write_text("Untracked review source.\n")
        sources = self.root / "verification/spec-sources.toml"
        sources.write_text(sources.read_text().replace('path = "fixture/source"', 'path = "fixture/untracked-review.md"'))
        with self.assertRaisesRegex(ValueError, "review source path is not git-tracked"):
            load_seed_audit(self.root)


class SeedAuditReportTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tempdir.name)
        _write_fixture(self.root, include_tooling=True)

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def test_live_state_and_report_revalidate_exactly_at_rest(self) -> None:
        state = build_live_seed_audit_state(
            self.root,
            timestamp="2026-07-10T12:00:00+00:00",
            require_clean_commit=True,
        )
        json_path = self.root / "target/gate-artifacts/verification/invariant-seed-audit.json"
        markdown_path = self.root / "target/gate-artifacts/verification/invariant-seed-audit.md"
        report = SeedAuditReport(
            provenance=SeedAuditProvenance(
                command=(
                    "python3",
                    "scripts/report_invariant_seed_audit.py",
                    "--json-out",
                    "target/gate-artifacts/verification/invariant-seed-audit.json",
                    "--markdown-out",
                    "target/gate-artifacts/verification/invariant-seed-audit.md",
                    "--timestamp",
                    state.timestamp,
                ),
                commit=state.commit,
                timestamp=state.timestamp,
                platform=state.platform,
                input_paths=state.input_paths,
                output_json="target/gate-artifacts/verification/invariant-seed-audit.json",
                output_markdown="target/gate-artifacts/verification/invariant-seed-audit.md",
            ),
            input_digest=state.input_digest,
            rows=state.audit.rows,
        )
        write_reports(report, json_path=json_path, markdown_path=markdown_path)
        markdown = markdown_path.read_text()
        self.assertIn("- Pre-existing seed mappings: 8", markdown)
        self.assertNotIn("Pre-existing canonical records", markdown)
        self.assertEqual(
            [],
            validate_report_files(
                self.root,
                json_path,
                markdown_path,
                self.root / "verification/schemas/invariant-seed-audit-report.schema.json",
            ),
        )

        payload = json.loads(json_path.read_text())
        payload["rows"][0]["status"] = "validated"
        json_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
        failures = validate_report_files(
            self.root,
            json_path,
            markdown_path,
            self.root / "verification/schemas/invariant-seed-audit-report.schema.json",
        )
        self.assertTrue(any("rows do not match current live seed audit" in item for item in failures), failures)

    def test_dirty_generation_and_schema_drift_fail_closed(self) -> None:
        (self.root / "verification/invariant-seeds.toml").write_text(
            (self.root / "verification/invariant-seeds.toml").read_text() + "\n# dirty\n"
        )
        with self.assertRaisesRegex(ValueError, "clean full Git SHA"):
            build_live_seed_audit_state(self.root, require_clean_commit=True)

        schema = json.loads(REPORT_SCHEMA_PATH.read_text())
        schema["properties"]["report_status"]["const"] = "partial"
        failures = validate_schema_contract(schema)
        self.assertTrue(any("report_status" in item for item in failures), failures)

    def test_payload_rejects_noncanonical_command_and_summary(self) -> None:
        state = build_live_seed_audit_state(self.root)
        payload = _payload_for_state(state)
        self.assertEqual([], validate_report_payload(payload, expected_rows=state.audit.rows))
        payload["command"][1] = "scripts/other.py"
        payload["summary"]["seeds"] = 43
        failures = validate_report_payload(payload, expected_rows=state.audit.rows)
        self.assertTrue(any("canonical invariant-seed audit invocation" in item for item in failures), failures)
        self.assertTrue(any("summary does not match" in item for item in failures), failures)

    def test_manifest_and_report_schemas_are_closed_and_drift_pinned(self) -> None:
        manifest_schema = json.loads(MANIFEST_SCHEMA_PATH.read_text())
        report_schema = json.loads(REPORT_SCHEMA_PATH.read_text())
        self.assertEqual([], validate_schema_contract(report_schema, manifest_schema=manifest_schema))
        self.assertFalse(manifest_schema["additionalProperties"])
        self.assertFalse(manifest_schema["$defs"]["seed"]["additionalProperties"])
        manifest = tomllib.loads((ROOT / "verification/invariant-seeds.toml").read_text())
        manifest["seeds"].append(copy.deepcopy(manifest["seeds"][0]))
        self.assertTrue(
            any(
                "more than 44 items" in failure
                for failure in validate_json_schema_instance(manifest, manifest_schema)
            )
        )


def _payload_for_state(state: object) -> dict[str, object]:
    report = SeedAuditReport(
        provenance=SeedAuditProvenance(
            command=(
                "python3",
                "scripts/report_invariant_seed_audit.py",
                "--json-out",
                "target/gate-artifacts/verification/invariant-seed-audit.json",
                "--markdown-out",
                "target/gate-artifacts/verification/invariant-seed-audit.md",
                "--timestamp",
                state.timestamp,
            ),
            commit=state.commit,
            timestamp=state.timestamp,
            platform=state.platform,
            input_paths=state.input_paths,
            output_json="target/gate-artifacts/verification/invariant-seed-audit.json",
            output_markdown="target/gate-artifacts/verification/invariant-seed-audit.md",
        ),
        input_digest=state.input_digest,
        rows=state.audit.rows,
    )
    return report.to_dict()


def _write_fixture(root: Path, *, include_tooling: bool = False) -> None:
    if root.exists():
        for child in sorted(root.rglob("*"), reverse=True):
            if child.is_file() or child.is_symlink():
                child.unlink()
            elif child.is_dir():
                child.rmdir()
    root.mkdir(parents=True, exist_ok=True)
    source = AREAS_PATH.read_text()
    _write(root, "docs/internal/testing/checklists/plc-verification-program/verification-areas.md", source)
    seeds = extract_written_seeds(source)
    alias = {
        "VM_SEAM_TYPE_001": "VM_SEAM_DECLARED_TYPE_001",
        "VM_SEAM_TYPE_002": "VM_SEAM_DECLARED_TYPE_001",
        "PROTO_DISC_001": "PROTO_DISCOVERY_TRUTH_001",
    }
    preexisting = {
        "RT_SAFE_STOP_001",
        "VM_SEAM_DECLARED_TYPE_001",
        "VM_SEAM_REF_001",
        "VM_SEAM_OWNER_001",
        "VM_SEAM_VALID_001",
        "VM_SEAM_ENC_001",
        "PROTO_DISCOVERY_TRUTH_001",
    }
    risk_by_seed = {seed_id: f"RISK_P4_{seed_id}" for seed_id in P4_000_SEED_IDS}
    records: list[dict[str, object]] = []
    written_by_id = {item.seed_id: item for item in seeds}
    for seed in seeds:
        canonical = alias.get(seed.seed_id, seed.seed_id)
        board_row, area = _row_and_area(seed.seed_id, seed.section)
        records.append(
            {
                "seed_id": seed.seed_id,
                "canonical_invariant_id": canonical,
                "board_row": board_row,
                "origin": "preexisting" if canonical in preexisting else "phase4",
                "p4_000_risk_id": risk_by_seed.get(seed.seed_id),
            }
        )
        path = root / "verification/invariants" / area / f"{canonical}.toml"
        if path.exists():
            continue
        spec_gap = canonical == "RT_SAFE_STOP_001"
        tests = '["TEST_FIXTURE"]' if canonical == "RT_SAFE_STOP_001" else "[]"
        evidence = '["EVID_FIXTURE"]' if canonical == "RT_SAFE_STOP_001" else "[]"
        if spec_gap:
            spec_block = textwrap.dedent(
                f'''\
                status = "spec_gap"
                spec_gap_refs = ["SPEC_GAP_FIXTURE_001"]
                tests = {tests}
                evidence_refs = {evidence}
                [spec]
                status = "missing"
                source_refs = ["SPEC_SOURCE_FIXTURE_001"]
                [oracle]
                kind = "trust_contract"
                ref = "SPEC_GAP_FIXTURE_001"
                [[coverage.cells]]
                dimension = "happy_path"
                state = "spec_gap"
                rationale = "Fixture behavior remains unspecified."
                spec_gap_ref = "SPEC_GAP_FIXTURE_001"
                '''
            )
        else:
            spec_block = textwrap.dedent(
                '''\
                status = "gap_open"
                spec_gap_refs = []
                tests = []
                evidence_refs = []
                [spec]
                status = "specified"
                source_refs = ["SPEC_SOURCE_FIXTURE_001"]
                [oracle]
                kind = "trust_contract"
                ref = "SPEC_SOURCE_FIXTURE_001"
                [[coverage.cells]]
                dimension = "happy_path"
                state = "gap_open"
                rationale = "No proof is associated with this seed."
                '''
            )
        _write(
            root,
            path.relative_to(root).as_posix(),
            textwrap.dedent(
                f'''\
                schema_version = 1
                id = "{canonical}"
                title = "{written_by_id[seed.seed_id].title.replace('"', "'")}"
                area = "{area}"
                risk = "safety_critical"
                owner = "fixture"
                last_reviewed = "2026-07-10"
                claim = "Fixture claim remains unproven."
                proof_level = "S0"
                contract_kind = "fault_scenario"
                source_refs = ["fixture/source"]
                gates = []
                missing = ["proof"]
                {spec_block}
                '''
            ),
        )
    _write_manifest(root, {"schema_version": 1, "seeds": records})
    _write(
        root,
        "verification/spec-sources.toml",
        textwrap.dedent(
            '''\
            [[spec_sources]]
            schema_version = 1
            id = "SPEC_SOURCE_FIXTURE_001"
            title = "Fixture reviewed source"
            area = "verification"
            owner = "fixture"
            status = "mapped"
            authority = "normative_product"
            source_status = "active"
            oracle_eligible = true
            visibility = "internal"
            path = "fixture/source"
            version = "current"
            last_reviewed = "2026-07-10"
            covers = ["fixture"]
            known_limitations = []
            conflicts_with = []
            '''
        ),
    )
    _write(root, "fixture/source", "Reviewed fixture source.\n")
    _write(
        root,
        "verification/schemas/invariant-seed-manifest.schema.json",
        MANIFEST_SCHEMA_PATH.read_text(),
    )
    _write(
        root,
        "verification/spec-gaps.toml",
        textwrap.dedent(
            '''\
            [[spec_gaps]]
            schema_version = 1
            id = "SPEC_GAP_FIXTURE_001"
            title = "Fixture gap"
            area = "runtime_safety"
            risk = "safety_critical"
            owner = "fixture"
            status = "spec_gap"
            gap_class = "missing_behavior"
            blocking_question = "What is the behavior?"
            affected_invariants = ["RT_SAFE_STOP_001"]
            affected_tests = ["TEST_FIXTURE"]
            candidate_spec_sources = ["SPEC_SOURCE_FIXTURE_001"]
            resolution_status = "open"
            closeout_evidence = []
            last_reviewed = "2026-07-10"
            '''
        ),
    )
    risk_blocks = []
    for seed_id, risk_id in sorted(risk_by_seed.items()):
        canonical = alias.get(seed_id, seed_id)
        row = next(item for item in records if item["seed_id"] == seed_id)
        _, area = _row_and_area(seed_id, written_by_id[seed_id].section)
        risk_blocks.append(
            textwrap.dedent(
                f'''\
                [[risks]]
                schema_version = 1
                id = "{risk_id}"
                title = "Imported review risk"
                area = "{area}"
                risk = "safety_critical"
                owner = "fixture"
                status = "planned"
                last_reviewed = "2026-07-10"
                description = "Confirmed review finding remains unproven."
                mitigation = "Keep the linked invariant open until proof exists."
                related_invariants = ["{canonical}"]
                related_spec_gaps = []
                evidence_refs = []
                source_refs = ["SPEC_SOURCE_FIXTURE_001"]
                '''
            )
        )
    _write(root, "verification/risk-register.toml", "\n".join(risk_blocks))
    _write(
        root,
        "verification/test-catalog.toml",
        textwrap.dedent(
            '''\
            [[tests]]
            schema_version = 2
            id = "TEST_FIXTURE"
            subject_kind = "case_table_artifact"
            status = "planned"
            invariants = ["RT_SAFE_STOP_001"]
            spec_gap_ref = "SPEC_GAP_FIXTURE_001"
            '''
        ),
    )
    _write(
        root,
        "verification/evidence-index.toml",
        textwrap.dedent(
            '''\
            [[evidence]]
            schema_version = 1
            id = "EVID_FIXTURE"
            proof_kind = "none"
            linked_invariants = ["RT_SAFE_STOP_001"]
            '''
        ),
    )
    if include_tooling:
        for relative in (
            "scripts/report_invariant_seed_audit.py",
            "scripts/validate_invariant_seed_audit_report.py",
            "scripts/verification/invariant_seed_cli.py",
            "scripts/verification/invariant_seed_contract.py",
            "scripts/verification/invariant_seed_live.py",
            "scripts/verification/invariant_seed_report.py",
            "scripts/verification/invariant_seed_validation.py",
            "scripts/verification/report_input_contract.py",
            "scripts/verification/test_catalog_common.py",
            "scripts/verification/test_catalog_json_schema.py",
            "scripts/verification/test_catalog_validation.py",
            "verification/schemas/invariant-seed-manifest.schema.json",
            "verification/schemas/invariant-seed-audit-report.schema.json",
        ):
            source_path = ROOT / relative
            if source_path.exists():
                _write(root, relative, source_path.read_text())
    _git(root, "init")
    _git(root, "config", "user.email", "fixture@example.com")
    _git(root, "config", "user.name", "Fixture")
    _git(root, "add", ".")
    _git(root, "commit", "-m", "fixture")


def _row_and_area(seed_id: str, section: str) -> tuple[str, str]:
    if section == "Compiler and IEC":
        return "VERIF-P4-001", "compiler_iec"
    if section == "PLCopen and Developer Tooling":
        return "VERIF-P4-005", "plcopen_devtools"
    if section == "HIR/VM Seam":
        return "VERIF-P4-002", "bytecode_vm"
    if section == "Runtime Safety":
        return "VERIF-P4-003", "runtime_safety"
    if section == "Protocols":
        return "VERIF-P4-004", "protocols"
    if seed_id.startswith("EDIT_") or seed_id == "DEBUG_PAUSE_001":
        return "VERIF-P4-005", "editor_safety"
    if seed_id == "UI_STATUS_001":
        return "VERIF-P4-006", "hmi_ui"
    if section == "Release and Docs":
        return "VERIF-P4-007", "release"
    return "VERIF-P4-008", "control_security" if seed_id in {"DEBUG_AUTH_001", "SEC_AUTHZ_001"} else "supply_chain_platform"


def _first_phase4_row(root: Path) -> dict[str, str]:
    manifest = _load_manifest(root)
    record = next(item for item in manifest["seeds"] if item["origin"] == "phase4")
    source = {row.seed_id: row for row in extract_written_seeds(AREAS_PATH.read_text())}[record["seed_id"]]
    _, area = _row_and_area(record["seed_id"], source.section)
    return {"id": record["canonical_invariant_id"], "area": area}


def _load_manifest(root: Path) -> dict[str, object]:
    return tomllib.loads((root / "verification/invariant-seeds.toml").read_text())


def _loaded_contract_arguments(root: Path) -> dict[str, object]:
    invariants: dict[str, dict[str, object]] = {}
    paths: dict[str, str] = {}
    for path in sorted((root / "verification/invariants").rglob("*.toml")):
        record = tomllib.loads(path.read_text())
        invariants[record["id"]] = record
        paths[record["id"]] = path.relative_to(root).as_posix()

    def index(relative: str, key: str) -> dict[str, dict[str, object]]:
        data = tomllib.loads((root / relative).read_text())
        return {row["id"]: row for row in data[key]}

    return {
        "written_seed_text": (
            root / "docs/internal/testing/checklists/plc-verification-program/verification-areas.md"
        ).read_text(),
        "seed_records": _load_manifest(root)["seeds"],
        "invariants": invariants,
        "invariant_paths": paths,
        "spec_sources": index("verification/spec-sources.toml", "spec_sources"),
        "spec_gaps": index("verification/spec-gaps.toml", "spec_gaps"),
        "risks": index("verification/risk-register.toml", "risks"),
        "tests": index("verification/test-catalog.toml", "tests"),
        "evidence": index("verification/evidence-index.toml", "evidence"),
    }


def _write_manifest(root: Path, manifest: dict[str, object]) -> None:
    lines = ["schema_version = 1"]
    for row in manifest["seeds"]:
        lines.extend(
            [
                "",
                "[[seeds]]",
                f'seed_id = "{row["seed_id"]}"',
                f'canonical_invariant_id = "{row["canonical_invariant_id"]}"',
                f'board_row = "{row["board_row"]}"',
                f'origin = "{row["origin"]}"',
                *(
                    [f'p4_000_risk_id = "{row["p4_000_risk_id"]}"']
                    if row.get("p4_000_risk_id") is not None
                    else []
                ),
            ]
        )
    _write(root, "verification/invariant-seeds.toml", "\n".join(lines) + "\n")


def _write(root: Path, relative: str, content: str) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)


def _git(root: Path, *args: str) -> None:
    subprocess.run(["git", "-C", str(root), *args], check=True, capture_output=True)


if __name__ == "__main__":
    unittest.main()
