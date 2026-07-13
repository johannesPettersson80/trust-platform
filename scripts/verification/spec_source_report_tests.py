"""Tests for the standalone specification-source audit report family."""

from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from scripts.verification.spec_source_analysis import analyze_spec_sources
from scripts.verification.spec_source_contract import (
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
)
from scripts.verification.spec_source_report import (
    SpecSourceAuditProvenance,
    SpecSourceAuditReport,
    render_markdown,
)
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "verification/schemas/spec-source-audit-report.schema.json"


def _fact(**values: object) -> SimpleNamespace:
    return SimpleNamespace(**values)


def _scan() -> SimpleNamespace:
    documents = (
        _fact(
            document_id="SPEC_DOC_docs_specs_contract_md_111111111111",
            path="docs/specs/contract.md",
            format="markdown",
            content_sha256="1" * 64,
            title="Runtime contract",
            in_spec_document_scope=True,
            primary_public_surface=False,
            public_entry_paths=(),
            headings=(),
            local_references=(),
        ),
        _fact(
            document_id="SPEC_DOC_README_md_222222222222",
            path="README.md",
            format="markdown",
            content_sha256="2" * 64,
            title="truST",
            in_spec_document_scope=True,
            primary_public_surface=True,
            public_entry_paths=("README.md",),
            headings=(),
            local_references=(),
        ),
    )
    public_blocks = (
        _fact(
            block_id="PUBLIC_BLOCK_README_md_10_10_aaaaaaaaaaaa",
            document_id="SPEC_DOC_README_md_222222222222",
            path="README.md",
            line_start=10,
            line_end=10,
            heading_path=("Runtime",),
            block_kind="list_item",
            text="One runtime, the right wire for each job.",
            text_sha256=hashlib.sha256(
                b"One runtime, the right wire for each job."
            ).hexdigest(),
            visible_text="One runtime, the right wire for each job.",
            visible_text_sha256=hashlib.sha256(
                b"One runtime, the right wire for each job."
            ).hexdigest(),
            local_references=(),
            public_entry_paths=("README.md",),
        ),
        _fact(
            block_id="PUBLIC_BLOCK_README_md_12_12_bbbbbbbbbbbb",
            document_id="SPEC_DOC_README_md_222222222222",
            path="README.md",
            line_start=12,
            line_end=12,
            heading_path=("Runtime",),
            block_kind="paragraph",
            text="A descriptive paragraph that has not been reviewed as a claim.",
            text_sha256=hashlib.sha256(
                b"A descriptive paragraph that has not been reviewed as a claim."
            ).hexdigest(),
            visible_text="A descriptive paragraph that has not been reviewed as a claim.",
            visible_text_sha256=hashlib.sha256(
                b"A descriptive paragraph that has not been reviewed as a claim."
            ).hexdigest(),
            local_references=(),
            public_entry_paths=("README.md",),
        ),
    )
    return SimpleNamespace(
        documents=documents,
        public_blocks=public_blocks,
        diagnostics=(),
        input_paths=("README.md", "docs/specs/contract.md"),
    )


def _metadata() -> tuple[dict[str, dict], dict[str, dict], dict[str, dict]]:
    sources = {
        "SPEC_RUNTIME": {
            "id": "SPEC_RUNTIME",
            "title": "Runtime contract",
            "area": "runtime_safety",
            "authority": "normative_product",
            "source_status": "active",
            "oracle_eligible": True,
            "visibility": "public",
            "locator_kind": "tracked_file",
            "path": "docs/specs/contract.md",
            "version": "current",
            "last_reviewed": "2026-07-01",
            "conflicts_with": [],
        },
        "PUBLIC_RUNTIME_WIRE": {
            "id": "PUBLIC_RUNTIME_WIRE",
            "title": "Runtime wire claim",
            "area": "protocols",
            "authority": "public_claim",
            "source_status": "active",
            "oracle_eligible": False,
            "visibility": "public",
            "locator_kind": "tracked_file",
            "path": "README.md",
            "version": "current",
            "surface_ref": "README.md#runtime",
            "claim_text": "One runtime, the right wire for each job.",
            "last_reviewed": "2026-07-01",
            "conflicts_with": [],
        },
    }
    required = {
        "REQ_RUNTIME": {
            "id": "REQ_RUNTIME",
            "area": "runtime_safety",
            "tag": "runtime_contract",
            "title": "Runtime contract",
            "owner": "trust-runtime",
            "status": "mapped",
            "source_ref": "SPEC_RUNTIME",
        },
        "REQ_VM_LIMIT": {
            "id": "REQ_VM_LIMIT",
            "area": "bytecode_vm",
            "tag": "vm_limit",
            "title": "VM limit",
            "owner": "trust-runtime-core",
            "status": "spec_gap",
            "spec_gap_ref": "SPEC_GAP_VM_LIMIT",
        },
    }
    gaps = {
        "SPEC_GAP_VM_LIMIT": {
            "id": "SPEC_GAP_VM_LIMIT",
            "area": "bytecode_vm",
            "resolution_status": "open",
        }
    }
    return sources, required, gaps


def _analysis() -> dict:
    sources, required, gaps = _metadata()
    return analyze_spec_sources(
        ROOT,
        scan=_scan(),
        spec_sources=sources,
        required_specs=required,
        spec_gaps=gaps,
    )


def _report(analysis: dict) -> SpecSourceAuditReport:
    return SpecSourceAuditReport(
        provenance=SpecSourceAuditProvenance(
            command=(
                "python3",
                "scripts/report_spec_source_audit.py",
                "--json-out",
                "target/gate-artifacts/verification/spec-source-audit.json",
                "--markdown-out",
                "target/gate-artifacts/verification/spec-source-audit.md",
                "--timestamp",
                "2026-07-13T10:00:00+02:00",
            ),
            commit="a" * 40,
            timestamp="2026-07-13T10:00:00+02:00",
            platform="test-platform",
            input_paths=("README.md", "docs/specs/contract.md"),
            output_json="target/gate-artifacts/verification/spec-source-audit.json",
            output_markdown="target/gate-artifacts/verification/spec-source-audit.md",
        ),
        input_digest="sha256:" + "b" * 64,
        analysis=analysis,
    )


class SpecSourceAnalysisTests(unittest.TestCase):
    def test_explicit_source_and_required_topic_joins_are_reported(self) -> None:
        analysis = _analysis()

        self.assertEqual(2, analysis["summary"]["documents_total"])
        self.assertEqual(2, analysis["summary"]["registered_sources"])
        self.assertEqual(1, analysis["summary"]["required_topics_mapped"])
        self.assertEqual(1, analysis["summary"]["required_topics_gap_open"])
        bindings = {row["source_id"]: row for row in analysis["source_bindings"]}
        self.assertEqual("bound", bindings["SPEC_RUNTIME"]["binding_state"])
        self.assertEqual(
            "SPEC_DOC_docs_specs_contract_md_111111111111",
            bindings["SPEC_RUNTIME"]["document_id"],
        )

    def test_names_titles_and_prose_do_not_create_source_or_topic_mappings(self) -> None:
        sources, required, gaps = _metadata()
        scan = _scan()
        decoy = _fact(
            document_id="SPEC_DOC_docs_specs_decoy_md_333333333333",
            path="docs/specs/decoy.md",
            content_sha256="3" * 64,
            source_kind="product_spec",
            title="Runtime contract VM limit SPEC_RUNTIME",
            local_references=(),
        )
        scan.documents = (*scan.documents, decoy)

        analysis = analyze_spec_sources(
            ROOT,
            scan=scan,
            spec_sources=sources,
            required_specs=required,
            spec_gaps=gaps,
        )

        document = next(
            row for row in analysis["documents"] if row["document_id"] == decoy.document_id
        )
        self.assertEqual([], document["registered_source_ids"])
        self.assertEqual("unreviewed_candidate", document["review_state"])
        topic = next(
            row for row in analysis["required_topics"] if row["topic_id"] == "REQ_VM_LIMIT"
        )
        self.assertEqual("SPEC_GAP_VM_LIMIT", topic["spec_gap_ref"])
        self.assertIsNone(topic["source_ref"])

    def test_public_blocks_are_exhaustive_but_semantic_review_remains_open(self) -> None:
        analysis = _analysis()

        self.assertEqual(2, analysis["summary"]["public_prose_blocks"])
        self.assertEqual(1, analysis["summary"]["registered_public_claims"])
        self.assertEqual(1, analysis["summary"]["unreviewed_public_blocks"])
        blocks = {row["block_id"]: row for row in analysis["public_prose_blocks"]}
        self.assertEqual(
            ["PUBLIC_RUNTIME_WIRE"],
            blocks["PUBLIC_BLOCK_README_md_10_10_aaaaaaaaaaaa"][
                "registered_claim_ids"
            ],
        )
        self.assertEqual(
            "unreviewed_candidate",
            blocks["PUBLIC_BLOCK_README_md_12_12_bbbbbbbbbbbb"]["review_state"],
        )
        self.assertTrue(analysis["scope"]["public_prose_denominator_exhaustive"])
        self.assertFalse(analysis["scope"]["semantic_claim_review_complete"])

    def test_public_claim_binding_requires_exact_path_and_text(self) -> None:
        sources, required, gaps = _metadata()
        for field, value in (
            ("path", "docs/public/other.md"),
            ("surface_ref", "docs/public/other.md#runtime"),
            ("claim_text", "One runtime, a superficially similar claim."),
        ):
            with self.subTest(field=field):
                mutated = copy.deepcopy(sources)
                mutated["PUBLIC_RUNTIME_WIRE"][field] = value
                analysis = analyze_spec_sources(
                    ROOT,
                    scan=_scan(),
                    spec_sources=mutated,
                    required_specs=required,
                    spec_gaps=gaps,
                )
                claim = analysis["registered_public_claims"][0]
                self.assertEqual("missing", claim["binding_state"])
                self.assertEqual([], claim["block_ids"])
                finding = next(
                    row
                    for row in analysis["findings"]
                    if row["code"] == "public_claim_missing"
                    and row["record_id"] == "PUBLIC_RUNTIME_WIRE"
                )
                self.assertEqual("error", finding["severity"])

    def test_external_source_locator_never_reads_expected_ignored_bytes(self) -> None:
        sources, required, gaps = _metadata()
        sources["SPEC_IEC_EXTERNAL"] = {
            "id": "SPEC_IEC_EXTERNAL",
            "title": "IEC 61131-3 Edition 3",
            "area": "compiler_iec",
            "authority": "normative_external",
            "source_status": "active",
            "oracle_eligible": False,
            "visibility": "external",
            "locator_kind": "external_reference",
            "external_ref": "IEC 61131-3:2013 Edition 3.0",
            "expected_local_path": "docs/internal/standards/iec61131-3.txt",
            "version": "Edition 3.0",
            "publication_date": "2013-02",
            "absence_blocks_proof": True,
            "last_reviewed": "2026-07-13",
            "conflicts_with": [],
        }

        with patch.object(Path, "is_file", side_effect=AssertionError("must not stat bytes")):
            analysis = analyze_spec_sources(
                ROOT,
                scan=_scan(),
                spec_sources=sources,
                required_specs=required,
                spec_gaps=gaps,
            )

        row = next(
            item
            for item in analysis["source_bindings"]
            if item["source_id"] == "SPEC_IEC_EXTERNAL"
        )
        self.assertEqual("external_reference", row["binding_state"])
        self.assertEqual("external_bytes_unbound", row["availability"])
        self.assertEqual("docs/internal/standards/iec61131-3.txt", row["expected_local_path"])

    def test_scanner_diagnostics_and_broken_metadata_refs_are_visible_findings(self) -> None:
        sources, required, gaps = _metadata()
        scan = _scan()
        scan.diagnostics = (
            _fact(
                severity="error",
                kind="missing_local_reference",
                path="README.md",
                line=20,
                message="missing tracked include",
            ),
        )
        required["REQ_RUNTIME"]["source_ref"] = "SPEC_MISSING"

        analysis = analyze_spec_sources(
            ROOT,
            scan=scan,
            spec_sources=sources,
            required_specs=required,
            spec_gaps=gaps,
        )

        finding_codes = {row["code"] for row in analysis["findings"]}
        self.assertIn("scanner_missing_local_reference", finding_codes)
        self.assertIn("required_topic_missing_source", finding_codes)
        self.assertGreaterEqual(analysis["summary"]["blocking_findings"], 2)

    def test_intermediate_gap_lifecycle_states_remain_actionable(self) -> None:
        sources, required, gaps = _metadata()
        for resolution in ("decision_recorded", "spec_updated", "test_mapped"):
            with self.subTest(resolution=resolution):
                updated_gaps = copy.deepcopy(gaps)
                updated_gaps["SPEC_GAP_VM_LIMIT"]["resolution_status"] = resolution
                analysis = analyze_spec_sources(
                    ROOT,
                    scan=_scan(),
                    spec_sources=sources,
                    required_specs=required,
                    spec_gaps=updated_gaps,
                )
                topic = next(
                    row
                    for row in analysis["required_topics"]
                    if row["topic_id"] == "REQ_VM_LIMIT"
                )
                self.assertEqual("gap_open", topic["mapping_state"])


class SpecSourceReportContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.analysis = _analysis()
        cls.report = _report(cls.analysis)
        cls.payload = cls.report.to_dict()
        cls.schema = json.loads(SCHEMA_PATH.read_text())

    def test_schema_payload_and_markdown_validate(self) -> None:
        self.assertEqual([], validate_schema_contract(self.schema))
        self.assertEqual([], validate_report_payload(self.payload, expected_analysis=self.analysis))
        self.assertEqual([], validate_json_schema_instance(self.payload, self.schema))
        json_bytes = self.report.to_json().encode()
        markdown = self.report.to_markdown(
            json_digest=hashlib.sha256(json_bytes).hexdigest()
        )
        self.assertEqual([], validate_markdown_binding(self.payload, json_bytes, markdown))

    def test_markdown_is_stable_after_canonical_json_reload(self) -> None:
        json_bytes = self.report.to_json().encode()
        digest = hashlib.sha256(json_bytes).hexdigest()

        self.assertEqual(
            self.report.to_markdown(json_digest=digest),
            render_markdown(json.loads(json_bytes), json_digest=digest),
        )

    def test_semantic_tamper_fails_live_recompute(self) -> None:
        tampered = copy.deepcopy(self.payload)
        tampered["documents"][0]["review_state"] = "unreviewed_candidate"
        failures = validate_report_payload(tampered, expected_analysis=self.analysis)
        self.assertIn("report rows do not match current specification-source analysis", failures)

    def test_noncanonical_json_and_markdown_tamper_fail(self) -> None:
        noncanonical = json.dumps(self.payload).encode()
        markdown = render_markdown(
            self.payload,
            json_digest=hashlib.sha256(noncanonical).hexdigest(),
        )
        self.assertIn(
            "specification-source JSON is not canonical",
            validate_markdown_binding(self.payload, noncanonical, markdown),
        )
        canonical = self.report.to_json().encode()
        failures = validate_markdown_binding(
            self.payload,
            canonical,
            self.report.to_markdown(json_digest=hashlib.sha256(canonical).hexdigest())
            + "tamper\n",
        )
        self.assertIn(
            "specification-source Markdown does not exactly match JSON payload",
            failures,
        )

    def test_unknown_fields_and_schema_drift_fail(self) -> None:
        tampered = copy.deepcopy(self.payload)
        tampered["invented"] = True
        self.assertIn("report fields drift", "\n".join(validate_report_payload(tampered)))

        schema = copy.deepcopy(self.schema)
        schema["properties"]["generator"]["const"] = "other"
        self.assertIn(
            "schema const for generator drifts",
            "\n".join(validate_schema_contract(schema)),
        )

    def test_hostile_nested_types_return_failures_without_exceptions(self) -> None:
        public_entries = copy.deepcopy(self.payload)
        public_entries["public_prose_blocks"][0]["public_entry_paths"] = 1
        failures = validate_report_payload(public_entries)
        self.assertIn("public_entry_paths is invalid", "\n".join(failures))

        severity = copy.deepcopy(self.payload)
        severity["findings"].append(
            {
                "code": "probe",
                "severity": [],
                "path": "README.md",
                "record_id": "probe",
                "message": "probe",
            }
        )
        failures = validate_report_payload(severity)
        self.assertIn("severity is invalid", "\n".join(failures))

    def test_recursive_row_type_mutations_are_rejected_without_tracebacks(self) -> None:
        sections = (
            "documents",
            "source_bindings",
            "required_topics",
            "obvious_missing_specs",
            "public_prose_blocks",
            "registered_public_claims",
            "findings",
        )
        mutations = 0
        for section in sections:
            if not self.payload[section]:
                continue
            for field, original in self.payload[section][0].items():
                with self.subTest(section=section, field=field):
                    tampered = copy.deepcopy(self.payload)
                    replacement: object
                    if isinstance(original, list):
                        replacement = {"hostile": True}
                    elif isinstance(original, dict):
                        replacement = []
                    elif isinstance(original, bool):
                        replacement = []
                    elif isinstance(original, int):
                        replacement = {}
                    elif isinstance(original, str):
                        replacement = []
                    else:
                        replacement = ["hostile"]
                    tampered[section][0][field] = replacement
                    failures = validate_report_payload(
                        tampered,
                        expected_analysis=self.analysis,
                    )
                    self.assertTrue(failures)
                    mutations += 1
        self.assertGreaterEqual(mutations, 70)


if __name__ == "__main__":
    unittest.main()
