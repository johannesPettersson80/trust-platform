"""Atomic semantic review for quarantined test-catalog mapping proposals."""

from __future__ import annotations

import argparse
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

from .code_spec_live import load_code_spec_metadata
from .mapping_denominator import reconcile_denominator_document
from .mapping_assertions import (
    invoke_assertion_inventories,
    validate_proposal_assertion_inventories,
)
from .mapping_review_batch import MappingReviewBatch, build_review_batch
from .mapping_review_selection import build_review_input_from_selection
from .mapping_transaction import apply_mapping_transaction
from .metadata_validator.mapping_metadata import validate_mapping_control_plane
from .test_catalog_denominator import (
    DENOMINATOR_SCHEMA_PATH,
    analyze_test_catalog_denominator,
    validate_test_catalog_denominator_document,
)
from .test_catalog_scanner import scan_repository
from .test_catalog_staleness import validate_catalog_staleness
from .test_catalog_json_schema import validate_json_schema_instance


PROPOSAL_PATH = Path("verification/test-catalog-proposals.toml")
DECISION_PATH = Path("verification/test-catalog-mapping-decisions.toml")
CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")


def execute_review_batch(
    *,
    root: Path,
    review_selection_path: Path,
    reviewed_date: str,
) -> MappingReviewBatch:
    root = root.resolve()
    proposal_document = _load_toml(root / PROPOSAL_PATH)
    source_commit = _git_head(root)
    _validate_proposal_source_commit(proposal_document, source_commit)
    decision_file = root / DECISION_PATH
    decision_document = (
        _load_toml(decision_file)
        if decision_file.is_file()
        else {
            "schema_version": 1,
            "generator": "test-catalog-mapping-review",
            "generator_version": "1",
            "source_commit": source_commit,
            "proposal_snapshots": [],
            "decisions": [],
        }
    )
    selection_document = _load_json(review_selection_path)
    selection_schema = json.loads(
        (
            root
            / "verification/schemas/test-catalog-mapping-review-selection.schema.json"
        ).read_text()
    )
    review_failures = validate_json_schema_instance(
        selection_document, selection_schema
    )
    if review_failures:
        raise ValueError("; ".join(review_failures))
    metadata = load_code_spec_metadata(root)
    review_document = build_review_input_from_selection(
        proposal_document=proposal_document,
        selection_document=selection_document,
        invariants=metadata.invariants,
    )
    review_schema = json.loads(
        (
            root
            / "verification/schemas/test-catalog-mapping-review-input.schema.json"
        ).read_text()
    )
    review_failures = validate_json_schema_instance(review_document, review_schema)
    if review_failures:
        raise ValueError("; ".join(review_failures))
    review_inputs = _review_inputs(review_document)
    proposals = {
        str(proposal["id"]): proposal
        for proposal in proposal_document.get("proposals", [])
        if isinstance(proposal, Mapping)
        and isinstance(proposal.get("id"), str)
    }
    selected_proposals = [
        proposals[str(review["proposal_id"])]
        for review in review_inputs
        if str(review.get("proposal_id")) in proposals
    ]
    if len(selected_proposals) != len(review_inputs):
        raise ValueError("generated review input names an unknown pending proposal")
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
    selected_facts = []
    for proposal in selected_proposals:
        discovery_id = str(proposal["discovery_id"])
        fact = facts.get(discovery_id)
        if fact is None:
            raise ValueError(
                f"selected proposal is absent from live scan: {discovery_id}"
            )
        selected_facts.append(fact)
    fresh_inventories = invoke_assertion_inventories(
        root,
        selected_facts,
        environment=os.environ,
    )
    assertion_failures = validate_proposal_assertion_inventories(
        proposals=selected_proposals,
        inventories=fresh_inventories,
    )
    if assertion_failures:
        raise ValueError("; ".join(assertion_failures))
    selected_invariants = {
        invariant_id
        for review in review_inputs
        if review.get("outcome") == "accepted"
        for invariant_id in _string_array(
            review.get("selected_invariant_ids"),
            "selected_invariant_ids",
        )
    }
    invariant_sources: dict[str, tuple[str, str]] = {}
    for invariant_id in sorted(selected_invariants):
        path = _find_invariant_path(root, invariant_id)
        invariant_sources[invariant_id] = (
            path.relative_to(root).as_posix(),
            path.read_text(),
        )
    ignored_document = _load_toml(root / "verification/ignored-tests.toml")
    ignored = ignored_document.get("ignored_tests", [])
    if not isinstance(ignored, list):
        raise ValueError("ignored-test registry must contain an object array")
    denominator_document = reconcile_denominator_document(
        document=_load_toml(root / DENOMINATOR_PATH),
        facts=scan.inferred_facts,
        tests=list(metadata.tests.values()),
        ignored_tests=ignored,
        reviewed_date=reviewed_date,
    )
    plan = build_review_batch(
        proposal_document=proposal_document,
        decision_document=decision_document,
        review_inputs=review_inputs,
        catalog_source=(root / CATALOG_PATH).read_text(),
        denominator_document=denominator_document,
        invariant_sources=invariant_sources,
        invariants=metadata.invariants,
        reviewed_date=reviewed_date,
        source_commit=source_commit,
    )
    apply_mapping_transaction(
        root=root,
        replacements=plan.replacements,
        validate=lambda: validate_reviewed_mapping_state(root),
    )
    return plan


def validate_reviewed_mapping_state(root: Path) -> list[str]:
    root = root.resolve()
    metadata = load_code_spec_metadata(root)
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
    proposal_document = _load_toml(root / PROPOSAL_PATH)
    decision_document = _load_toml(root / DECISION_PATH)
    denominator = _load_toml(root / DENOMINATOR_PATH)
    reviews = {
        str(review["discovery_id"]): review
        for review in denominator.get("reviews", [])
        if isinstance(review, Mapping)
        and isinstance(review.get("discovery_id"), str)
    }
    failures = validate_mapping_control_plane(
        root=root,
        proposal_document=proposal_document,
        decision_document=decision_document,
        facts=facts,
        tests=metadata.tests,
        denominator_reviews=reviews,
        invariants=metadata.invariants,
    )
    schema = json.loads((root / DENOMINATOR_SCHEMA_PATH).read_text())
    failures.extend(
        validate_test_catalog_denominator_document(denominator, schema=schema)
    )
    ignored_document = _load_toml(root / "verification/ignored-tests.toml")
    ignored = ignored_document.get("ignored_tests", [])
    try:
        analyze_test_catalog_denominator(
            facts=scan.inferred_facts,
            tests=list(metadata.tests.values()),
            reviews=denominator.get("reviews", []),
            ignored_tests=ignored if isinstance(ignored, list) else [],
        )
    except ValueError as error:
        failures.append(str(error))
    failures.extend(
        validate_catalog_staleness(
            root=root,
            tests=metadata.tests,
            facts=scan.inferred_facts,
        )
    )
    for test_id, test in metadata.tests.items():
        if "mapping_decision_id" not in test:
            continue
        for invariant_id in test.get("invariants", []):
            invariant = metadata.invariants.get(str(invariant_id))
            if not isinstance(invariant, Mapping) or test_id not in invariant.get(
                "tests", []
            ):
                failures.append(
                    f"{test_id} is not reciprocally linked by invariant {invariant_id}"
                )
    return sorted(set(failures))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--selection", type=Path, required=True)
    parser.add_argument("--reviewed-date")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    reviewed_date = args.reviewed_date or datetime.now(timezone.utc).date().isoformat()
    root = args.root.resolve()
    review_selection_path = (
        args.selection
        if args.selection.is_absolute()
        else root / args.selection
    )
    try:
        plan = execute_review_batch(
            root=root,
            review_selection_path=review_selection_path,
            reviewed_date=reviewed_date,
        )
    except (
        OSError,
        ValueError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
        subprocess.SubprocessError,
    ) as error:
        print(f"mapping review failed: {error}", file=sys.stderr)
        return 2
    print(
        "mapping review committed atomically: "
        f"{len(plan.accepted_test_ids)} accepted, "
        f"{len(plan.rejected_proposal_ids)} rejected"
    )
    return 0


def _review_inputs(document: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    if set(document) != {"schema_version", "reviews"}:
        raise ValueError("review input root fields drift")
    if document.get("schema_version") != 1:
        raise ValueError("review input must use schema_version 1")
    reviews = document.get("reviews")
    if (
        not isinstance(reviews, Sequence)
        or isinstance(reviews, (str, bytes))
        or not reviews
        or any(not isinstance(review, Mapping) for review in reviews)
    ):
        raise ValueError("review input requires non-empty review object array")
    return list(reviews)


def _find_invariant_path(root: Path, invariant_id: str) -> Path:
    matches = []
    for path in (root / "verification/invariants").rglob("*.toml"):
        try:
            record = tomllib.loads(path.read_text())
        except (OSError, tomllib.TOMLDecodeError):
            continue
        if record.get("id") == invariant_id:
            matches.append(path)
    if len(matches) != 1:
        raise ValueError(
            f"selected invariant {invariant_id} does not resolve to one source file"
        )
    return matches[0]


def _load_toml(path: Path) -> dict[str, Any]:
    document = tomllib.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} root must be a table")
    return document


def _load_json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} root must be an object")
    return document


def _string_array(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an array")
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value):
        raise ValueError(f"{label} must contain non-empty strings")
    return result


def _git_head(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    head = completed.stdout.strip()
    if completed.returncode != 0 or COMMIT_RE.fullmatch(head) is None:
        raise ValueError("mapping review cannot resolve source commit")
    return head


def _validate_proposal_source_commit(
    proposal_document: Mapping[str, Any],
    source_commit: str,
) -> None:
    proposal_commit = proposal_document.get("source_commit")
    if (
        not isinstance(proposal_commit, str)
        or COMMIT_RE.fullmatch(proposal_commit) is None
        or proposal_commit != source_commit
    ):
        raise ValueError(
            "mapping review proposal source commit is stale: "
            f"proposal={proposal_commit} current={source_commit}"
        )
