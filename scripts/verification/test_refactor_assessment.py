"""Compose catalog-evidence-only recommendations for existing test refactors."""

from __future__ import annotations

from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .test_refactor_duplicates import analyze_duplicate_fixtures
from .test_refactor_file_metrics import analyze_test_files, analyze_vscode_registration


SCHEMA_VERSION = 1
LIMITATIONS = (
    "Large-file findings are mechanical line counts at the reviewed inclusive threshold.",
    "Mixed-purpose findings require multiple reviewed catalog areas or test classes; names and source text never establish purpose.",
    "Broad-claim findings require multiple catalog invariants; catalog v2 has no authorized coverage-dimension field.",
    "Duplicate findings compare committed whole-file bytes and whitespace-normalized whole-file text; they do not infer semantic similarity.",
    "Fixture helper functions and helper-only files are not assessed as duplicate fixtures in this slice.",
    "Malformed-input overlap comes only from explicit malformed_input_class_ids in reviewed catalog rows.",
    "Duration classifications come only from hand-owned catalog metadata; unclassified scanner facts receive no inferred duration.",
    "A supported proposal means its disposition agrees with visible assessment signals; it does not authorize a move, split, or rename.",
)


def build_test_refactor_assessment(
    *,
    root: Path,
    scanner_facts: Sequence[Mapping[str, Any]],
    catalog_records: Sequence[Mapping[str, Any]],
    suites: Sequence[Mapping[str, Any]],
    vscode_registration_audit: Mapping[str, Any],
    large_file_threshold: int,
    proposals: Sequence[Mapping[str, Any]] = (),
) -> dict[str, Any]:
    """Build a deterministic assessment from source facts and reviewed metadata."""

    file_rows = analyze_test_files(
        root=root,
        scanner_facts=scanner_facts,
        catalog_records=catalog_records,
        large_file_threshold=large_file_threshold,
    )
    duplicate_analysis = analyze_duplicate_fixtures(
        root=root,
        paths=[row["path"] for row in file_rows],
        catalog_records=catalog_records,
    )
    broad_claims = _analyze_broad_claims(catalog_records)
    vscode = analyze_vscode_registration(vscode_registration_audit, file_rows)
    duration = _analyze_duration(scanner_facts, catalog_records, suites)
    signal_map = _signals_by_path(file_rows, broad_claims, duplicate_analysis, vscode)
    proposal_evaluations = _evaluate_proposals(proposals, signal_map)
    candidate_broad = [
        row for row in broad_claims if row["result"] == "candidate_missing_coverage_dimensions"
    ]
    duplicate_report = {
        "case_file_paths": duplicate_analysis["case_file_paths"],
        "exact_case_input_groups": duplicate_analysis["exact_case_input_groups"],
        "exact_fact_file_groups": _report_content_groups(
            duplicate_analysis["exact_groups"], "content_sha256"
        ),
        "malformed_class_overlap_groups": duplicate_analysis["malformed_class_overlaps"],
        "shared_case_reference_groups": duplicate_analysis[
            "shared_case_file_reference_groups"
        ],
        "source_body_similarity": duplicate_analysis["free_form_body_similarity"],
        "structural_case_input_peer_groups": duplicate_analysis[
            "same_table_structural_shape_groups"
        ],
        "whitespace_normalized_fact_file_groups": _report_content_groups(
            duplicate_analysis["whitespace_normalized_groups"],
            "normalized_content_sha256",
        ),
    }
    scanner_duration_classified = sum(
        row["classification_source"] == "hand_catalog" for row in duration["scanner_facts"]
    )
    summary = {
        "broad_claim_candidates": len(candidate_broad),
        "catalog_records": len(catalog_records),
        "catalog_slow_records": sum(
            row["duration_class"] == "slow"
            for row in [*duration["scanner_facts"], *duration["artifact_catalog_records"]]
            if row["duration_class"] is not None
        ),
        "exact_case_input_duplicate_groups": len(
            duplicate_report["exact_case_input_groups"]
        ),
        "exact_fact_file_duplicate_groups": len(duplicate_report["exact_fact_file_groups"]),
        "fact_files": len(file_rows),
        "large_file_candidates": sum(
            "large_file" in row["candidate_reasons"] for row in file_rows
        ),
        "malformed_class_overlap_groups": len(
            duplicate_report["malformed_class_overlap_groups"]
        ),
        "proposals": len(proposal_evaluations),
        "reviewed_mapping_diversity_candidates": sum(
            "reviewed_mapping_diversity" in row["candidate_reasons"] for row in file_rows
        ),
        "scanner_duration_classified": scanner_duration_classified,
        "scanner_duration_unclassified": len(duration["scanner_facts"])
        - scanner_duration_classified,
        "scanner_facts": len(scanner_facts),
        "shared_case_reference_groups": len(
            duplicate_report["shared_case_reference_groups"]
        ),
        "structural_case_input_peer_groups": len(
            duplicate_report["structural_case_input_peer_groups"]
        ),
        "supported_proposals": sum(row["supported"] for row in proposal_evaluations),
        "vscode_facts": vscode["fact_count"],
        "vscode_files": vscode["test_file_count"],
        "vscode_large_candidates": sum(row["large_candidate"] for row in vscode["files"]),
        "vscode_registrations": vscode["registration_count"],
        "whitespace_normalized_fact_file_duplicate_groups": len(
            duplicate_report["whitespace_normalized_fact_file_groups"]
        ),
    }
    return {
        "summary": summary,
        "file_assessment": file_rows,
        "broad_claim_assessment": broad_claims,
        "duplicate_assessment": duplicate_report,
        "vscode_registration": vscode,
        "duration_classification": duration,
        "proposal_evaluations": proposal_evaluations,
    }


def _analyze_broad_claims(
    catalog_records: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    for record in catalog_records:
        test_id = _string(record.get("id"), "catalog test id")
        if test_id in seen_ids:
            raise ValueError(f"catalog duplicates test id {test_id}")
        seen_ids.add(test_id)
        invariants = _string_list(record.get("invariants"), f"{test_id} invariants")
        if not invariants:
            result = "no_invariant_claim"
        elif len(invariants) == 1:
            result = "single_invariant"
        else:
            result = "candidate_missing_coverage_dimensions"
        rows.append(
            {
                "coverage_dimensions": [],
                "invariant_count": len(invariants),
                "invariants": sorted(invariants),
                "path": _string(record.get("path"), f"{test_id} path"),
                "result": result,
                "test_id": test_id,
            }
        )
    return sorted(rows, key=lambda row: (row["path"], row["test_id"]))


def _analyze_duration(
    scanner_facts: Sequence[Mapping[str, Any]],
    catalog_records: Sequence[Mapping[str, Any]],
    suites: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    fact_by_id: dict[str, Mapping[str, Any]] = {}
    for fact in scanner_facts:
        discovery_id = _first_string(fact, ("stable_id", "discovery_id"), "scanner identity")
        if discovery_id in fact_by_id:
            raise ValueError(f"scanner duplicates discovery identity {discovery_id}")
        fact_by_id[discovery_id] = fact

    classification_by_id: dict[str, Mapping[str, Any]] = {}
    artifacts: list[dict[str, Any]] = []
    unassigned: list[str] = []
    assigned_suite_ids: set[str] = set()
    for record in catalog_records:
        test_id = _string(record.get("id"), "catalog test id")
        duration_class = _string(record.get("duration_class"), f"{test_id} duration_class")
        tiers = _string_list(record.get("suite_tiers"), f"{test_id} suite_tiers")
        if not tiers:
            unassigned.append(test_id)
        assigned_suite_ids.update(tiers)
        if record.get("subject_kind") == "generated_test":
            discovery_id = _string(record.get("discovery_id"), f"{test_id} discovery_id")
            if discovery_id not in fact_by_id:
                raise ValueError(f"catalog test {test_id} discovery identity is absent: {discovery_id}")
            if discovery_id in classification_by_id:
                raise ValueError(f"discovery identity {discovery_id} has multiple duration records")
            classification_by_id[discovery_id] = {
                "duration_class": duration_class,
                "test_id": test_id,
            }
        else:
            artifacts.append(
                {
                    "duration_class": duration_class,
                    "path": _string(record.get("path"), f"{test_id} path"),
                    "subject_kind": _string(
                        record.get("subject_kind"), f"{test_id} subject_kind"
                    ),
                    "suite_tiers": sorted(tiers),
                    "test_id": test_id,
                }
            )

    suite_rows: list[dict[str, Any]] = []
    suite_ids: set[str] = set()
    for suite in suites:
        suite_id = _string(suite.get("id"), "suite id")
        if suite_id in suite_ids:
            raise ValueError(f"duplicate suite id {suite_id}")
        suite_ids.add(suite_id)
        commands = suite.get("commands", [])
        if not isinstance(commands, list) or any(not isinstance(item, str) for item in commands):
            raise ValueError(f"suite {suite_id} commands must be a string list")
        placeholder = suite.get("placeholder", False)
        if not isinstance(placeholder, bool):
            raise ValueError(f"suite {suite_id} placeholder must be boolean")
        suite_rows.append(
            {
                "commands_configured": bool(commands),
                "placeholder": placeholder,
                "suite_id": suite_id,
            }
        )
    scanner_rows: list[dict[str, Any]] = []
    for discovery_id, fact in sorted(fact_by_id.items()):
        classification = classification_by_id.get(discovery_id)
        scanner_rows.append(
            {
                "catalog_test_id": classification["test_id"] if classification else None,
                "classification_source": "hand_catalog" if classification else "unclassified",
                "discovery_id": discovery_id,
                "duration_class": classification["duration_class"] if classification else None,
                "ignore_state": _string(fact.get("ignore_state"), f"{discovery_id} ignore_state"),
                "name": _string(fact.get("name"), f"{discovery_id} name"),
                "path": _string(fact.get("path"), f"{discovery_id} path"),
                "source_kind": _first_string(
                    fact,
                    ("source_kind", "discovery_source_kind"),
                    f"{discovery_id} source_kind",
                ),
            }
        )
    return {
        "artifact_catalog_records": sorted(artifacts, key=lambda row: row["test_id"]),
        "commandless_suite_ids": sorted(
            row["suite_id"] for row in suite_rows if not row["commands_configured"]
        ),
        "placeholder_suite_ids": sorted(
            row["suite_id"] for row in suite_rows if row["placeholder"]
        ),
        "scanner_facts": scanner_rows,
        "suite_tiers": sorted(suite_rows, key=lambda row: row["suite_id"]),
        "unassigned_tier_test_ids": sorted(unassigned),
        "unknown_assigned_suite_ids": sorted(assigned_suite_ids - suite_ids),
    }


def _signals_by_path(
    file_rows: Sequence[Mapping[str, Any]],
    broad_claims: Sequence[Mapping[str, Any]],
    duplicates: Mapping[str, Sequence[Mapping[str, Any]]],
    vscode: Mapping[str, Any],
) -> dict[str, set[str]]:
    signals: dict[str, set[str]] = defaultdict(set)
    known_paths = {str(row["path"]) for row in file_rows}
    for row in file_rows:
        path = str(row["path"])
        if "large_file" in row["candidate_reasons"]:
            signals[path].add(f"large_file:{path}")
        if "reviewed_mapping_diversity" in row["candidate_reasons"]:
            signals[path].add(f"mixed_purpose:{path}")
    for row in broad_claims:
        if row["result"] == "candidate_missing_coverage_dimensions":
            signals[str(row["path"])].add(f"broad_claim:{row['test_id']}")
    for key, prefix in (
        ("exact_groups", "exact_duplicate"),
        ("whitespace_normalized_groups", "whitespace_normalized_duplicate"),
    ):
        for index, group in enumerate(duplicates[key], start=1):
            for path in group["paths"]:
                signals[path].add(f"{prefix}:{index}")
    for row in duplicates["malformed_class_overlaps"]:
        for path in row["paths"]:
            signals[path].add(f"malformed_class_overlap:{row['malformed_input_class_id']}")
    for index, row in enumerate(duplicates["exact_case_input_groups"], start=1):
        for path in row["case_files"]:
            signals[path].add(f"exact_case_input_duplicate:{index}")
    for index, row in enumerate(
        duplicates["same_table_structural_shape_groups"], start=1
    ):
        signals[row["case_file"]].add(f"structural_case_input_peers:{index}")
    for row in duplicates["shared_case_file_reference_groups"]:
        for path in [row["case_file"], *row["record_paths"]]:
            signals[path].add(f"shared_case_file_reference:{row['case_file']}")
    for row in vscode["files"]:
        if row["large_candidate"]:
            signals[row["path"]].add(f"vscode_large_registered_file:{row['path']}")
    for field, prefix in (
        ("unregistered_files", "vscode_unregistered_file"),
        ("unregistered_fact_files", "vscode_unregistered_fact_file"),
        ("missing_targets", "vscode_missing_registration_target"),
        ("duplicate_targets", "vscode_duplicate_registration"),
    ):
        for path in vscode["registration_issues"][field]:
            signals[path].add(f"{prefix}:{path}")
    for diagnostic in vscode["diagnostics"]:
        if diagnostic["severity"] == "error" and diagnostic["path"] in known_paths:
            signals[diagnostic["path"]].add(
                f"vscode_registration_error:{diagnostic['kind']}"
            )
    for path in known_paths:
        signals.setdefault(path, set())
    return signals


def _report_content_groups(
    groups: Sequence[Mapping[str, Any]], digest_field: str
) -> list[dict[str, Any]]:
    return [
        {
            "content_digest": f"sha256:{row[digest_field]}",
            "paths": list(row["paths"]),
        }
        for row in groups
    ]


def _evaluate_proposals(
    proposals: Sequence[Mapping[str, Any]],
    signal_map: Mapping[str, set[str]],
) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    for proposal in proposals:
        proposal_id = _string(proposal.get("id"), "proposal id")
        if proposal_id in seen_ids:
            raise ValueError(f"duplicate proposal id {proposal_id}")
        seen_ids.add(proposal_id)
        disposition = _string(proposal.get("disposition"), f"{proposal_id} disposition")
        if disposition not in {"no_refactor_needed", "move", "rename", "split"}:
            raise ValueError(f"{proposal_id} has unsupported disposition {disposition!r}")
        source_paths_value = proposal.get("source_paths")
        if source_paths_value is None and "path" in proposal:
            source_paths_value = [proposal["path"]]
        source_paths = sorted(_string_list(source_paths_value, f"{proposal_id} source_paths"))
        if not source_paths:
            raise ValueError(f"{proposal_id} source_paths must not be empty")
        _string(proposal.get("rationale"), f"{proposal_id} rationale")
        observed: set[str] = set()
        for path in source_paths:
            if path not in signal_map:
                observed.add(f"unassessed_path:{path}")
            else:
                observed.update(signal_map[path])
        supported = not observed if disposition == "no_refactor_needed" else False
        results.append(
            {
                "disposition": disposition,
                "observed_signals": sorted(observed),
                "proposal_id": proposal_id,
                "source_paths": source_paths,
                "supported": supported,
            }
        )
    return sorted(results, key=lambda row: row["proposal_id"])


def _first_string(record: Mapping[str, Any], fields: tuple[str, ...], label: str) -> str:
    for field in fields:
        value = record.get(field)
        if isinstance(value, str) and value:
            return value
    raise ValueError(f"{label} must be a non-empty string")


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} must be a non-empty string")
    return value


def _string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
        raise ValueError(f"{label} must be a string list")
    if len(value) != len(set(value)):
        raise ValueError(f"{label} must not contain duplicates")
    return list(value)
