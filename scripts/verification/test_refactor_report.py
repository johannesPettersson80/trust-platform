"""Canonical report model for Phase 2A existing-test refactor assessment."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


GENERATOR = "test-refactor-assessment"
GENERATOR_VERSION = 1
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/test-refactor-assessment.json")
DEFAULT_MARKDOWN_PATH = Path(
    "docs/internal/testing/evidence/plc-verification-program/2026-07-10/"
    "p2a-test-refactor-assessment.md"
)


@dataclass(frozen=True)
class RefactorAssessmentProvenance:
    command: tuple[str, ...]
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    output_json: str
    output_markdown: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "command": list(self.command),
            "commit": self.commit,
            "timestamp": self.timestamp,
            "platform": self.platform,
            "input_paths": list(self.input_paths),
            "output_paths": {
                "json": self.output_json,
                "markdown": self.output_markdown,
            },
        }


@dataclass(frozen=True)
class TestRefactorAssessmentReport:
    provenance: RefactorAssessmentProvenance
    input_digest: str
    scope: dict[str, Any]
    assessment: dict[str, Any]
    limitations: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": 1,
            "generator": GENERATOR,
            "generator_version": GENERATOR_VERSION,
            "report_status": "complete",
            "input_digest": self.input_digest,
            **self.provenance.to_dict(),
            "scope": self.scope,
            **self.assessment,
            "limitations": list(self.limitations),
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n"

    def to_markdown(self, *, json_digest: str) -> str:
        payload = self.to_dict()
        summary = payload["summary"]
        lines = [
            "# Existing-Test Refactor Assessment",
            "",
            f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
            f"Source revision: `{self.provenance.commit}`",
            f"Generated: `{self.provenance.timestamp}`",
            f"Platform: `{self.provenance.platform}`",
            f"Generated JSON SHA-256: `{json_digest}`",
            f"Input SHA-256: `{self.input_digest}`",
            "",
            "Size is a review signal, not a refactor decision.",
            "Mechanical similarity is candidate evidence only; it never authorizes",
            "a move, split, rename, fixture merge, or behavior change.",
            "",
            "## Summary",
            "",
        ]
        labels = (
            ("scanner_facts", "Scanner facts"),
            ("fact_files", "Fact-bearing files"),
            ("large_file_candidates", "Large-file candidates"),
            (
                "reviewed_mapping_diversity_candidates",
                "Reviewed mapping-diversity candidates",
            ),
            ("broad_claim_candidates", "Broad multi-invariant claim candidates"),
            ("exact_fact_file_duplicate_groups", "Exact fact-file duplicate groups"),
            (
                "whitespace_normalized_fact_file_duplicate_groups",
                "Whitespace-normalized fact-file duplicate groups",
            ),
            ("exact_case_input_duplicate_groups", "Exact case-input duplicate groups"),
            (
                "structural_case_input_peer_groups",
                "Same-table structural case-input peer groups",
            ),
            ("shared_case_reference_groups", "Shared case-file reference groups"),
            ("malformed_class_overlap_groups", "Malformed-class overlap groups"),
            ("vscode_facts", "VS Code facts"),
            ("vscode_files", "VS Code files"),
            ("vscode_registrations", "VS Code registrations"),
            ("vscode_large_candidates", "Large registered VS Code files"),
            ("catalog_records", "Catalog records"),
            ("scanner_duration_classified", "Scanner facts with reviewed duration"),
            ("scanner_duration_unclassified", "Scanner facts without reviewed duration"),
            ("catalog_slow_records", "Catalog rows explicitly classified slow"),
            ("proposals", "Reviewed proposal decisions"),
            ("supported_proposals", "Assessment-supported decisions"),
        )
        for key, label in labels:
            lines.append(f"- {label}: {summary[key]}")

        lines.extend(
            [
                "",
                "## Large Or Mixed-Purpose Signals",
                "",
                "| Path | Lines | Facts | Reviewed mappings | Signals |",
                "| --- | ---: | ---: | ---: | --- |",
            ]
        )
        candidates = [
            row
            for row in payload["file_assessment"]
            if row.get("candidate_reasons")
        ]
        for row in candidates:
            lines.append(
                f"| `{row['path']}` | {row['physical_lines']} | "
                f"{row['scanner_fact_count']} | {len(row['mapped_test_ids'])} | "
                f"{', '.join(f'`{item}`' for item in row['candidate_reasons'])} |"
            )
        if not candidates:
            lines.append("| none | 0 | 0 | 0 | none |")

        lines.extend(["", "## Broad Invariant Claims", ""])
        broad = payload["broad_claim_assessment"]
        if broad:
            for row in broad:
                lines.append(
                    f"- `{row['test_id']}` claims {row['invariant_count']} invariants; "
                    f"result `{row['result']}`."
                )
        else:
            lines.append("- No catalog row claims more than one invariant.")

        duplicate = payload["duplicate_assessment"]
        lines.extend(
            [
                "",
                "## Duplicate And Structural Signals",
                "",
                f"- Exact fact-file groups: {len(duplicate['exact_fact_file_groups'])}",
                "- Whitespace-normalized fact-file groups: "
                f"{len(duplicate['whitespace_normalized_fact_file_groups'])}",
                f"- Exact case-input groups: {len(duplicate['exact_case_input_groups'])}",
                "- Same-table structural case-input peer groups: "
                f"{len(duplicate['structural_case_input_peer_groups'])}",
                "- Shared case-file reference groups: "
                f"{len(duplicate['shared_case_reference_groups'])}",
                "- Explicit malformed-class overlap groups: "
                f"{len(duplicate['malformed_class_overlap_groups'])}",
                f"- Free-form source-body similarity: `{duplicate['source_body_similarity']}`",
            ]
        )
        for row in duplicate["exact_fact_file_groups"]:
            lines.append(
                f"- Exact source `{row['content_digest']}`: "
                f"{', '.join(f'`{item}`' for item in row['paths'])}"
            )
        for row in duplicate["whitespace_normalized_fact_file_groups"]:
            lines.append(
                f"- Normalized source `{row['content_digest']}`: "
                f"{', '.join(f'`{item}`' for item in row['paths'])}"
            )
        for row in duplicate["exact_case_input_groups"]:
            lines.append(
                f"- Exact case input `{row['input_digest']}`: cases "
                f"{', '.join(f'`{item}`' for item in row['case_ids'])}; files "
                f"{', '.join(f'`{item}`' for item in row['case_files'])}."
            )
        for row in duplicate["structural_case_input_peer_groups"]:
            lines.append(
                f"- Structural peers in `{row['case_file']}`: "
                f"{', '.join(f'`{item}`' for item in row['case_ids'])}; "
                f"shape `{row['shape_digest']}`."
            )
        for row in duplicate["shared_case_reference_groups"]:
            lines.append(
                f"- Shared case file `{row['case_file']}`: tests "
                f"{', '.join(f'`{item}`' for item in row['test_ids'])}; record paths "
                f"{', '.join(f'`{item}`' for item in row['record_paths'])}."
            )
        for row in duplicate["malformed_class_overlap_groups"]:
            lines.append(
                f"- Malformed class `{row['malformed_input_class_id']}`: tests "
                f"{', '.join(f'`{item}`' for item in row['test_ids'])}; paths "
                f"{', '.join(f'`{item}`' for item in row['paths'])}."
            )
        lines.extend(["", "## VS Code Registration", ""])
        vscode = payload["vscode_registration"]
        lines.extend(
            [
                f"- Discovered facts: {vscode['fact_count']}",
                f"- Test files: {vscode['test_file_count']}",
                f"- Literal registrations: {vscode['registration_count']}",
                f"- Diagnostics: {len(vscode['diagnostics'])}",
            ]
        )
        large_vscode = [row for row in vscode["files"] if row.get("large_candidate")]
        for row in large_vscode:
            lines.append(
                f"- `{row['path']}`: {row['physical_lines']} lines, "
                f"{row['fact_count']} facts."
            )

        duration = payload["duration_classification"]
        lines.extend(
            [
                "",
                "## Duration Classification",
                "",
                f"- Scanner facts listed: {len(duration['scanner_facts'])}",
                "- Artifact catalog rows listed separately: "
                f"{len(duration['artifact_catalog_records'])}",
                "- Ignored, nightly, hardware, and name signals never infer duration.",
            ]
        )
        for row in duration["scanner_facts"]:
            if row["classification_source"] == "hand_catalog":
                lines.append(
                    f"- Scanner `{row['discovery_id']}` / `{row['catalog_test_id']}`: "
                    f"`{row['duration_class']}` at `{row['path']}`."
                )
        for row in duration["artifact_catalog_records"]:
            lines.append(
                f"- Artifact `{row['test_id']}`: `{row['duration_class']}` "
                f"`{row['subject_kind']}` at `{row['path']}`; suites "
                f"{', '.join(f'`{item}`' for item in row['suite_tiers']) or 'none'}."
            )
        for field, label in (
            ("commandless_suite_ids", "Commandless suites"),
            ("placeholder_suite_ids", "Placeholder suites"),
            ("unassigned_tier_test_ids", "Catalog rows without suite tiers"),
            ("unknown_assigned_suite_ids", "Unknown assigned suites"),
        ):
            values = duration[field]
            lines.append(
                f"- {label}: {', '.join(f'`{item}`' for item in values) or 'none'}"
            )
        lines.extend(["", "## Reviewed Proposal Decisions", ""])
        proposals = payload["proposal_evaluations"]
        for row in proposals:
            signals = ", ".join(f"`{item}`" for item in row["observed_signals"])
            lines.append(
                f"- `{row['proposal_id']}`: disposition `{row['disposition']}`, "
                f"supported `{'yes' if row['supported'] else 'no'}`, "
                f"sources {', '.join(f'`{item}`' for item in row['source_paths'])}, "
                f"observed signals {signals or 'none'}."
            )
        if not proposals:
            lines.append("- No reviewed proposal decisions are recorded.")
        lines.extend(
            [
                "",
                "## Limitations",
                "",
            ]
        )
        lines.extend(f"- {item}" for item in self.limitations)
        return "\n".join(lines) + "\n"


def write_reports(
    report: TestRefactorAssessmentReport,
    *,
    json_path: Path,
    markdown_path: Path,
) -> None:
    rendered_json = report.to_json()
    digest = hashlib.sha256(rendered_json.encode()).hexdigest()
    json_path.parent.mkdir(parents=True, exist_ok=True)
    markdown_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(rendered_json)
    markdown_path.write_text(report.to_markdown(json_digest=digest))
