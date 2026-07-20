"""Mechanical public-workflow candidates and their reviewed disposition ledger."""

from __future__ import annotations

import hashlib
import re
import subprocess
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .spec_source_markdown import scan_markdown_document


INVENTORY_PATH = "verification/public-workflow-inventory.toml"
SCHEMA_VERSION = 1
DISPOSITIONS = {"workflow_spec", "reviewed_nonworkflow"}
REVIEW_FIELDS = {
    "discovery_id",
    "path",
    "heading_path",
    "disposition",
    "rationale",
}
WORKFLOW_REVIEW_FIELDS = REVIEW_FIELDS | {"spec_source_id"}
WORKFLOW_SPEC_FIELDS = {
    "actor",
    "entry_point",
    "preconditions",
    "visible_steps",
    "success_state",
    "failure_status_behavior",
    "safety_authz_boundaries",
    "acceptance_evidence",
}
_ORDERED_ITEM_RE = re.compile(r"^[ \t]*\d+[.)][ \t]+")


@dataclass(frozen=True)
class WorkflowCandidate:
    discovery_id: str
    path: str
    heading_path: tuple[str, ...]
    line: int
    visible_steps: tuple[str, ...]


def discover_candidates_from_sources(
    sources: Mapping[str, str],
) -> tuple[WorkflowCandidate, ...]:
    candidates: list[WorkflowCandidate] = []
    for path, text in sorted(sources.items()):
        scan = scan_markdown_document(path, text)
        headings = {row.section_identity: row for row in scan.headings}
        steps_by_section: dict[tuple[str, ...], list[str]] = {}
        for block in scan.blocks:
            if block.block_kind != "list_item" or not _ORDERED_ITEM_RE.match(block.text):
                continue
            steps_by_section.setdefault(block.section_identity, []).append(
                block.visible_text
            )
        for section_identity, steps in steps_by_section.items():
            heading = headings.get(section_identity)
            if heading is None:
                continue
            identity = "\0".join((path, *section_identity))
            candidates.append(
                WorkflowCandidate(
                    discovery_id=(
                        "WORKFLOW_CANDIDATE_"
                        + hashlib.sha256(identity.encode()).hexdigest()[:24].upper()
                    ),
                    path=path,
                    heading_path=heading.heading_path,
                    line=heading.line,
                    visible_steps=tuple(steps),
                )
            )
    return tuple(candidates)


def discover_public_workflow_candidates(root: Path) -> tuple[WorkflowCandidate, ...]:
    root = root.resolve()
    result = subprocess.run(
        ["git", "ls-files", "-z", "--", "docs/public/*.md", "docs/public/**/*.md"],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    )
    paths = sorted(
        value.decode("utf-8")
        for value in result.stdout.split(b"\0")
        if value
    )
    sources: dict[str, str] = {}
    for path in paths:
        candidate = root / path
        if candidate.is_symlink() or not candidate.is_file():
            raise ValueError(f"public workflow source is missing or symlinked: {path}")
        sources[path] = candidate.read_text(encoding="utf-8")
    return discover_candidates_from_sources(sources)


def validate_public_workflow_inventory(
    root: Path,
    document: Mapping[str, Any],
    *,
    spec_sources: Mapping[str, Mapping[str, Any]],
    candidates: tuple[WorkflowCandidate, ...] | None = None,
) -> list[str]:
    failures: list[str] = []
    if candidates is None:
        try:
            candidates = discover_public_workflow_candidates(root)
        except (OSError, UnicodeError, subprocess.CalledProcessError, ValueError) as exc:
            return [f"public workflow discovery failed: {exc}"]

    if set(document) != {"schema_version", "reviews"}:
        failures.append("public workflow inventory root fields are not closed")
    if document.get("schema_version") != SCHEMA_VERSION:
        failures.append(f"public workflow inventory must use schema_version = {SCHEMA_VERSION}")
    reviews = document.get("reviews")
    if not isinstance(reviews, list):
        return [*failures, "public workflow inventory reviews must be an array"]

    candidate_by_id = {row.discovery_id: row for row in candidates}
    review_by_id: dict[str, Mapping[str, Any]] = {}
    review_ids: list[str] = []
    for index, review in enumerate(reviews):
        where = f"reviews[{index}]"
        if not isinstance(review, Mapping):
            failures.append(f"{where} must be an object")
            continue
        review_id = review.get("discovery_id")
        if not isinstance(review_id, str) or not review_id:
            failures.append(f"{where}.discovery_id must be a non-empty string")
            continue
        review_ids.append(review_id)
        if review_id in review_by_id:
            failures.append(f"duplicate review for {review_id}")
            continue
        review_by_id[review_id] = review
        candidate = candidate_by_id.get(review_id)
        if candidate is None:
            failures.append(f"invented review {review_id}")
            continue
        disposition = review.get("disposition")
        allowed_fields = (
            WORKFLOW_REVIEW_FIELDS if disposition == "workflow_spec" else REVIEW_FIELDS
        )
        extra = sorted(set(review) - allowed_fields)
        missing = sorted(allowed_fields - set(review))
        if extra:
            failures.append(f"{review_id} has unexpected fields: {', '.join(extra)}")
        if missing:
            failures.append(f"{review_id} missing fields: {', '.join(missing)}")
        if review.get("path") != candidate.path:
            failures.append(f"{review_id} path does not match discovered candidate")
        if review.get("heading_path") != list(candidate.heading_path):
            failures.append(f"{review_id} heading_path does not match discovered candidate")
        if disposition not in DISPOSITIONS:
            failures.append(f"{review_id} has unknown disposition {disposition!r}")
        if not isinstance(review.get("rationale"), str) or not review["rationale"]:
            failures.append(f"{review_id} rationale must be a non-empty string")
        if disposition == "workflow_spec":
            _validate_workflow_spec(review_id, review, candidate, spec_sources, failures)
        elif "spec_source_id" in review:
            failures.append(f"{review_id} reviewed_nonworkflow forbids spec_source_id")

    for candidate in candidates:
        if candidate.discovery_id not in review_by_id:
            failures.append(f"missing review for {candidate.discovery_id}")
    expected_order = [row.discovery_id for row in candidates]
    if review_ids != expected_order:
        failures.append("public workflow reviews must use canonical discovery order")
    return failures


def _validate_workflow_spec(
    review_id: str,
    review: Mapping[str, Any],
    candidate: WorkflowCandidate,
    spec_sources: Mapping[str, Mapping[str, Any]],
    failures: list[str],
) -> None:
    source_id = review.get("spec_source_id")
    source = spec_sources.get(source_id) if isinstance(source_id, str) else None
    if source is None:
        failures.append(f"{review_id} references unknown workflow spec source {source_id!r}")
        return
    if (
        source.get("authority") != "normative_product"
        or source.get("source_status") != "active"
        or source.get("oracle_eligible") is not True
    ):
        failures.append(
            f"{review_id} workflow requires an active oracle-eligible normative_product source"
        )
    if source.get("path") != candidate.path:
        failures.append(f"{review_id} workflow source path does not match candidate path")
    if f"public_workflow:{review_id}" not in source.get("covers", []):
        failures.append(f"{review_id} workflow source does not cover its discovery identity")
    if not WORKFLOW_SPEC_FIELDS <= set(source):
        failures.append(f"{review_id} source does not contain complete workflow fields")
        return
    if source.get("visible_steps") != list(candidate.visible_steps):
        failures.append(f"{review_id} source visible_steps do not match public workflow")
    for field in ("visible_steps", "acceptance_evidence"):
        value = source.get(field)
        if not isinstance(value, list) or not value or not all(
            isinstance(item, str) and item for item in value
        ):
            failures.append(f"{review_id} source does not contain complete workflow fields")
