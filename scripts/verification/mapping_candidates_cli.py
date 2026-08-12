"""Candidate report and quarantined proposal generation CLI."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tomllib
from collections.abc import Mapping, Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .code_spec_live import CodeSpecMetadata, load_code_spec_metadata
from .focused_mapping_candidates import generate_focused_mapping_candidates
from .mapping_catalog_updates import render_denominator_document
from .mapping_artifact_paths import resolve_mapping_artifact_output
from .mapping_denominator import reconcile_denominator_document
from .mapping_candidates import analyze_mapping_candidates
from .mapping_assertions import invoke_assertion_inventories
from .mapping_live_code_facts import validated_live_code_facts
from .mapping_proposals import (
    build_mapping_proposals,
    validate_mapping_quarantine,
)
from .mapping_review import (
    suppressed_proposal_digests,
    validate_mapping_decisions,
)
from .mapping_toml import render_decisions_toml, render_proposals_toml
from .mapping_transaction import apply_mapping_transaction
from .test_catalog_denominator import (
    DENOMINATOR_SCHEMA_PATH,
    analyze_test_catalog_denominator,
    load_test_catalog_denominator,
    validate_test_catalog_denominator_document,
)
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_scanner import scan_repository


def initial_decision_document(source_commit: str) -> dict[str, Any]:
    if re.fullmatch(r"[0-9a-f]{40}", source_commit) is None:
        raise ValueError("mapping decision initialization requires source commit")
    return {
        "schema_version": 1,
        "generator": "test-catalog-mapping-review",
        "generator_version": "1",
        "source_commit": source_commit,
        "proposal_snapshots": [],
        "decisions": [],
    }


def build_candidate_document(
    *,
    source_commit: str,
    trace_digest: str,
    analysis: Mapping[str, Any],
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "generator": "test-mapping-candidates",
        "generator_version": "1",
        "source_commit": source_commit,
        "trace_digest": trace_digest,
        "candidates": list(analysis.get("candidates", [])),
        "demands": list(analysis.get("demands", [])),
        "spec_demand_observations": list(
            analysis.get("spec_demand_observations", [])
        ),
        "demand_ranking": list(analysis.get("demand_ranking", [])),
        "structural_demand_ranking": list(
            analysis.get("structural_demand_ranking", [])
        ),
        "unmapped_code_demand_ranking": list(
            analysis.get("unmapped_code_demand_ranking", [])
        ),
    }


def generate_pilot_candidate_document(
    *,
    root: Path,
    trace: Mapping[str, Any],
    trace_digest: str,
    metadata: CodeSpecMetadata,
) -> dict[str, Any]:
    """Build execution-guided signals without scanning or mutating catalog state."""

    live_code_facts = validated_live_code_facts(
        root,
        trace,
        metadata.mappings,
    )
    analysis = analyze_mapping_candidates(
        trace_records=trace["tests"],
        code_mappings=metadata.mappings,
        invariants=metadata.invariants,
        live_code_facts=live_code_facts,
    )
    return build_candidate_document(
        source_commit=str(trace["source_commit"]),
        trace_digest=trace_digest,
        analysis=analysis,
    )


def select_quarantine_candidates(
    candidates: Sequence[Mapping[str, Any]],
    *,
    catalog_discovery_ids: set[str],
    denominator_reviews: Mapping[str, Mapping[str, Any]],
) -> list[Mapping[str, Any]]:
    selected = []
    for candidate in candidates:
        discovery_id = candidate.get("discovery_id")
        review = denominator_reviews.get(str(discovery_id))
        if (
            isinstance(discovery_id, str)
            and discovery_id not in catalog_discovery_ids
            and review is not None
            and review.get("disposition") == "reviewed_nonmapping"
            and not candidate.get("unmapped_code_ids")
        ):
            selected.append(candidate)
    return sorted(selected, key=lambda row: str(row["discovery_id"]))


def generate_mapping_candidates(
    *,
    root: Path,
    trace_path: Path,
    candidates_path: Path,
    proposals_path: Path,
    decisions_path: Path,
    timestamp: str | None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    root = root.resolve()
    generated = timestamp or datetime.now(timezone.utc).replace(
        microsecond=0
    ).isoformat().replace("+00:00", "Z")
    trace = _load_json(trace_path)
    trace_schema = _load_json(
        root / "verification/schemas/test-coverage-trace.schema.json"
    )
    failures = validate_json_schema_instance(trace, trace_schema)
    if failures:
        raise ValueError("; ".join(failures))
    metadata = load_code_spec_metadata(root)
    current_commit = _git_head(root)
    if trace.get("source_commit") != current_commit:
        raise ValueError(
            "trace source commit is stale: "
            f"trace={trace.get('source_commit')} current={current_commit}"
        )
    trace_digest = _canonical_digest(trace)
    candidate_document = generate_pilot_candidate_document(
        root=root,
        trace=trace,
        trace_digest=trace_digest,
        metadata=metadata,
    )
    candidate_schema = _load_json(
        root / "verification/schemas/mapping-candidates.schema.json"
    )
    failures = validate_json_schema_instance(candidate_document, candidate_schema)
    if failures:
        raise ValueError("; ".join(failures))

    scan = scan_repository(root)
    facts = {
        fact.stable_id: {
            "discovery_id": fact.stable_id,
            "discovery_source_kind": fact.source_kind,
            "path": fact.path,
            "name": fact.name,
            "ignore_state": fact.ignore_state,
        }
        for fact in scan.inferred_facts
    }
    denominator = load_test_catalog_denominator(root)
    ignored_document = tomllib.loads(
        (root / "verification/ignored-tests.toml").read_text()
    )
    ignored_tests = ignored_document.get("ignored_tests")
    if (
        not isinstance(ignored_tests, list)
        or any(not isinstance(record, Mapping) for record in ignored_tests)
    ):
        raise ValueError("ignored-test registry must contain object rows")
    denominator = reconcile_denominator_document(
        document=denominator,
        facts=scan.inferred_facts,
        tests=list(metadata.tests.values()),
        ignored_tests=ignored_tests,
        reviewed_date=generated[:10],
    )
    reviews = {
        str(review["discovery_id"]): review
        for review in denominator.get("reviews", [])
        if isinstance(review, Mapping)
        and isinstance(review.get("discovery_id"), str)
    }
    catalog_discovery_ids = {
        str(test["discovery_id"])
        for test in metadata.tests.values()
        if test.get("subject_kind") == "generated_test"
        and isinstance(test.get("discovery_id"), str)
    }
    selected = select_quarantine_candidates(
        candidate_document["candidates"],
        catalog_discovery_ids=catalog_discovery_ids,
        denominator_reviews=reviews,
    )
    trace_by_id = {
        str(record["discovery_id"]): record
        for record in trace["tests"]
        if isinstance(record, Mapping)
        and isinstance(record.get("discovery_id"), str)
    }
    assertion_inventories = invoke_assertion_inventories(
        root,
        [facts[str(candidate["discovery_id"])] for candidate in selected],
        environment=os.environ,
    )
    proposals = build_mapping_proposals(
        root=root,
        candidates=selected,
        trace_records=trace_by_id,
        facts=facts,
        invariants=metadata.invariants,
        assertion_inventories=assertion_inventories,
    )
    if decisions_path.is_file():
        decision_document = tomllib.loads(decisions_path.read_text())
        decision_schema = _load_json(
            root
            / "verification/schemas/test-catalog-mapping-decisions.schema.json"
        )
        decision_failures = validate_json_schema_instance(
            decision_document, decision_schema
        )
        snapshots = decision_document.get("proposal_snapshots", [])
        decisions = decision_document.get("decisions", [])
        if (
            not isinstance(snapshots, list)
            or any(not isinstance(snapshot, Mapping) for snapshot in snapshots)
            or not isinstance(decisions, list)
            or any(not isinstance(decision, Mapping) for decision in decisions)
        ):
            raise ValueError(
                "mapping decisions must contain snapshot and decision tables"
            )
        snapshot_by_id = {
            str(snapshot["id"]): snapshot
            for snapshot in snapshots
            if isinstance(snapshot.get("id"), str)
        }
        if len(snapshot_by_id) != len(snapshots):
            raise ValueError("mapping decision snapshots duplicate or lack IDs")
        proposal_schema = _load_json(
            root / "verification/schemas/test-catalog-proposals.schema.json"
        )
        snapshot_schema = {
            "$ref": "#/$defs/proposal",
            "$defs": proposal_schema.get("$defs", {}),
        }
        for snapshot in snapshots:
            decision_failures.extend(
                validate_json_schema_instance(snapshot, snapshot_schema)
            )
        decision_failures.extend(
            validate_mapping_decisions(
            decisions=decisions,
            proposals=snapshot_by_id,
            invariants=metadata.invariants,
            candidate_generator_id="test-catalog-proposals",
            )
        )
        if decision_failures:
            raise ValueError("; ".join(sorted(set(decision_failures))))
        suppressed = suppressed_proposal_digests(decisions)
        proposals = [
            proposal
            for proposal in proposals
            if proposal["proposal_digest"] not in suppressed
        ]
    else:
        decision_document = initial_decision_document(str(trace["source_commit"]))
    proposal_document = {
        "schema_version": 1,
        "generator": "test-catalog-proposals",
        "generator_version": "1",
        "source_commit": str(trace["source_commit"]),
        "trace_digest": trace_digest,
        "candidate_report_digest": _canonical_digest(candidate_document),
        "last_generated": generated,
        "demands": list(candidate_document["demands"]),
        "demand_ranking": list(candidate_document["demand_ranking"]),
        "structural_demand_ranking": list(
            candidate_document["structural_demand_ranking"]
        ),
        "unmapped_code_demand_ranking": list(
            candidate_document["unmapped_code_demand_ranking"]
        ),
        "proposals": proposals,
    }
    quarantine_failures = validate_mapping_quarantine(
        root=root,
        proposals=proposals,
        facts=facts,
        tests=metadata.tests,
        denominator_reviews=reviews,
        invariants=metadata.invariants,
    )
    proposal_schema = _load_json(
        root / "verification/schemas/test-catalog-proposals.schema.json"
    )
    quarantine_failures.extend(
        validate_json_schema_instance(proposal_document, proposal_schema)
    )
    if quarantine_failures:
        raise ValueError("; ".join(sorted(set(quarantine_failures))))
    denominator_schema = _load_json(root / DENOMINATOR_SCHEMA_PATH)
    denominator_failures = validate_test_catalog_denominator_document(
        denominator,
        schema=denominator_schema,
    )
    if denominator_failures:
        raise ValueError("; ".join(denominator_failures))
    analyze_test_catalog_denominator(
        facts=scan.inferred_facts,
        tests=list(metadata.tests.values()),
        reviews=denominator["reviews"],
        ignored_tests=ignored_tests,
    )
    replacements = {
        _relative_to_root(root, candidates_path): (
            json.dumps(candidate_document, indent=2, sort_keys=True) + "\n"
        ).encode(),
        _relative_to_root(root, proposals_path): render_proposals_toml(
            proposal_document
        ).encode(),
        _relative_to_root(root, decisions_path): render_decisions_toml(
            decision_document
        ).encode(),
        "verification/test-catalog-denominator.toml": (
            render_denominator_document(denominator).encode()
        ),
    }
    apply_mapping_transaction(
        root=root,
        replacements=replacements,
        validate=lambda: _validate_generated_files(
            root=root,
            candidates_path=candidates_path,
            proposals_path=proposals_path,
            decisions_path=decisions_path,
            candidate_document=candidate_document,
            proposal_document=proposal_document,
            decision_document=decision_document,
            denominator_document=denominator,
        ),
    )
    return candidate_document, proposal_document


def generate_pilot_candidates(
    *,
    root: Path,
    trace_path: Path,
    candidates_path: Path,
) -> dict[str, Any]:
    """Generate only the hidden-ground-truth candidate plane."""

    root = root.resolve()
    candidates_path = resolve_mapping_artifact_output(
        root,
        candidates_path,
        "mapping pilot candidate output",
    )
    trace = _load_json(trace_path)
    trace_schema = _load_json(
        root / "verification/schemas/test-coverage-trace.schema.json"
    )
    failures = validate_json_schema_instance(trace, trace_schema)
    if failures:
        raise ValueError("; ".join(failures))
    current_commit = _git_head(root)
    if trace.get("source_commit") != current_commit:
        raise ValueError(
            "trace source commit is stale: "
            f"trace={trace.get('source_commit')} current={current_commit}"
        )
    metadata = load_code_spec_metadata(root)
    candidate_document = generate_pilot_candidate_document(
        root=root,
        trace=trace,
        trace_digest=_canonical_digest(trace),
        metadata=metadata,
    )
    candidate_schema = _load_json(
        root / "verification/schemas/mapping-candidates.schema.json"
    )
    failures = validate_json_schema_instance(candidate_document, candidate_schema)
    if failures:
        raise ValueError("; ".join(failures))
    content = (
        json.dumps(candidate_document, indent=2, sort_keys=True) + "\n"
    ).encode()
    relative = _relative_to_root(root, candidates_path)
    apply_mapping_transaction(
        root=root,
        replacements={relative: content},
        validate=lambda: (
            []
            if candidates_path.is_file()
            and candidates_path.read_bytes() == content
            else ["pilot candidate report bytes differ"]
        ),
    )
    return candidate_document


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--trace", type=Path, required=True)
    parser.add_argument(
        "--candidates-out",
        type=Path,
        default=Path(
            "target/gate-artifacts/verification/mapping-candidates.json"
        ),
    )
    parser.add_argument(
        "--proposals-out",
        type=Path,
        default=Path("verification/test-catalog-proposals.toml"),
    )
    parser.add_argument(
        "--decisions",
        type=Path,
        default=Path("verification/test-catalog-mapping-decisions.toml"),
    )
    parser.add_argument(
        "--pilot-only",
        action="store_true",
        help="write candidate signals only; do not scan or mutate catalog planes",
    )
    parser.add_argument(
        "--focused-rust-discovery-id",
        action="append",
        default=[],
        help=(
            "generate production quarantine planes for this reviewed Rust DISC ID "
            "without the full repository test scan; repeat for a batch"
        ),
    )
    parser.add_argument(
        "--legacy-full-scan",
        action="store_true",
        help=(
            "run the deprecated repository-wide scanner instead of the "
            "focused production route"
        ),
    )
    parser.add_argument(
        "--authority-plan",
        type=Path,
        help=(
            "optional quarantined reviewed code-spec authority projection used "
            "only for candidate ranking"
        ),
    )
    parser.add_argument(
        "--authority-routes",
        type=Path,
        help=(
            "optional authenticated reviewed test-invariant routes for exact "
            "focused C4 demands"
        ),
    )
    parser.add_argument(
        "--trace-root",
        type=Path,
        help="focused raw coverage evidence root",
    )
    parser.add_argument(
        "--authenticated-trace-receipt",
        type=Path,
        help=(
            "validated focused trace receipt used to reauthenticate raw "
            "evidence without replaying Cargo or LLVM reconstruction"
        ),
    )
    parser.add_argument(
        "--cargo-target-dir",
        type=Path,
        help="focused coverage Cargo target directory",
    )
    parser.add_argument(
        "--llvm-tools-dir",
        type=Path,
        help="directory containing llvm-cov and llvm-profdata",
    )
    parser.add_argument(
        "--temporary-dir",
        type=Path,
        help="focused coverage temporary directory",
    )
    parser.add_argument("--timestamp")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        if args.pilot_only and args.focused_rust_discovery_id:
            raise ValueError(
                "--pilot-only and --focused-rust-discovery-id are mutually exclusive"
            )
        if args.legacy_full_scan and (
            args.pilot_only or args.focused_rust_discovery_id
        ):
            raise ValueError(
                "--legacy-full-scan is mutually exclusive with focused and "
                "pilot generation modes"
            )
        if args.authority_plan is not None and not args.focused_rust_discovery_id:
            raise ValueError(
                "--authority-plan requires --focused-rust-discovery-id"
            )
        if args.authority_routes is not None and not args.focused_rust_discovery_id:
            raise ValueError(
                "--authority-routes requires --focused-rust-discovery-id"
            )
        if args.focused_rust_discovery_id:
            required_paths = {
                "--trace-root": args.trace_root,
                "--cargo-target-dir": args.cargo_target_dir,
                "--llvm-tools-dir": args.llvm_tools_dir,
                "--temporary-dir": args.temporary_dir,
            }
            missing_paths = [
                option
                for option, value in required_paths.items()
                if value is None
            ]
            if missing_paths:
                raise ValueError(
                    "focused mapping candidate generation requires "
                    + ", ".join(missing_paths)
                )
        if args.pilot_only:
            candidates = generate_pilot_candidates(
                root=root,
                trace_path=_resolve(root, args.trace),
                candidates_path=_resolve(root, args.candidates_out),
            )
            print(
                "mapping pilot candidates generated: "
                f"{len(candidates['candidates'])} candidates, "
                f"{len(candidates['demands'])} specification demands"
            )
            return 0
        if (
            not args.focused_rust_discovery_id
            and not args.legacy_full_scan
        ):
            raise ValueError(
                "production mapping candidate generation requires explicit "
                "--focused-rust-discovery-id or --legacy-full-scan"
            )
        if args.focused_rust_discovery_id:
            candidates, proposals = generate_focused_mapping_candidates(
                root=root,
                trace_path=_resolve(root, args.trace),
                candidates_path=_resolve(root, args.candidates_out),
                proposals_path=_resolve(root, args.proposals_out),
                decisions_path=_resolve(root, args.decisions),
                rust_discovery_ids=args.focused_rust_discovery_id,
                authority_plan_path=(
                    _resolve(root, args.authority_plan)
                    if args.authority_plan is not None
                    else None
                ),
                authority_routes_path=(
                    _resolve(root, args.authority_routes)
                    if args.authority_routes is not None
                    else None
                ),
                authenticated_trace_receipt_path=(
                    _resolve(root, args.authenticated_trace_receipt)
                    if args.authenticated_trace_receipt is not None
                    else None
                ),
                trace_root=_resolve(root, args.trace_root),
                cargo_target_dir=_resolve(root, args.cargo_target_dir),
                llvm_tools_dir=_resolve(root, args.llvm_tools_dir),
                temporary_dir=_resolve(root, args.temporary_dir),
                timestamp=args.timestamp,
            )
        else:
            candidates, proposals = generate_mapping_candidates(
                root=root,
                trace_path=_resolve(root, args.trace),
                candidates_path=_resolve(root, args.candidates_out),
                proposals_path=_resolve(root, args.proposals_out),
                decisions_path=_resolve(root, args.decisions),
                timestamp=args.timestamp,
            )
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"mapping candidate generation failed: {error}", file=sys.stderr)
        return 2
    print(
        "mapping candidates generated: "
        f"{len(candidates['candidates'])} candidates, "
        f"{len(candidates['demands'])} specification demands, "
        f"{len(proposals['proposals'])} quarantined proposals"
    )
    return 0


def _canonical_digest(document: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        document, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _load_json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} root must be an object")
    return document


def _resolve(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _relative_to_root(root: Path, path: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(root).as_posix()
    except ValueError as error:
        raise ValueError(
            f"mapping candidate output escapes repository: {path}"
        ) from error


def _validate_generated_files(
    *,
    root: Path,
    candidates_path: Path,
    proposals_path: Path,
    decisions_path: Path,
    candidate_document: Mapping[str, Any],
    proposal_document: Mapping[str, Any],
    decision_document: Mapping[str, Any],
    denominator_document: Mapping[str, Any],
) -> list[str]:
    failures: list[str] = []
    try:
        if _load_json(candidates_path) != dict(candidate_document):
            failures.append("candidate report differs after atomic replacement")
        if tomllib.loads(proposals_path.read_text()) != dict(proposal_document):
            failures.append("proposal queue differs after atomic replacement")
        if tomllib.loads(decisions_path.read_text()) != dict(decision_document):
            failures.append("decision ledger differs after atomic replacement")
        if load_test_catalog_denominator(root) != dict(denominator_document):
            failures.append("denominator differs after atomic replacement")
    except (OSError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        failures.append(f"mapping candidate replacement cannot be read: {error}")
    return failures


def _git_head(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()
