"""Fail-closed UI journey acceptance and freshness contract."""

from __future__ import annotations

import hashlib
import re
import subprocess
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any


MANIFEST_PATH = "verification/ui-acceptance.toml"
SCHEMA_VERSION = 1
STATUSES = {"evidence_missing", "provisional", "stale", "ux_accepted"}
JOURNEY_SOURCES = {"batch", "standalone"}
THEMES = {"dark", "light", "high_contrast"}
COMMON_FIELDS = {
    "id",
    "title",
    "surface",
    "status",
    "journey_source",
    "workflow_candidate_ids",
    "invariant_ids",
    "supporting_test_ids",
    "implementation_paths",
    "source_transformation",
    "last_reviewed",
}
EVIDENCE_FIELDS = COMMON_FIELDS | {
    "runner_paths",
    "source_commit",
    "implementer",
    "evidence",
}
EVIDENCE_ROW_FIELDS = {
    "theme",
    "screenshot_path",
    "screenshot_sha256",
    "result_path",
    "result_sha256",
}
_SHA_RE = re.compile(r"^[0-9a-f]{64}$")
_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
_DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def validate_ui_acceptance_document(
    root: Path,
    document: Mapping[str, Any],
    *,
    tests: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    workflow_reviews: Sequence[Mapping[str, Any]],
    batch_journey_ids: Sequence[str],
    changed_paths_by_journey: Mapping[str, Sequence[str]] | None = None,
    require_tracked_files: bool = False,
) -> list[str]:
    failures: list[str] = []
    if set(document) != {"schema_version", "batch_runner_path", "journeys"}:
        failures.append("UI acceptance root fields are not closed")
    if document.get("schema_version") != SCHEMA_VERSION:
        failures.append(f"UI acceptance must use schema_version = {SCHEMA_VERSION}")
    if not _safe_file(root, document.get("batch_runner_path")):
        failures.append("batch_runner_path must name a contained regular file")
    elif require_tracked_files and not _is_tracked(root, str(document["batch_runner_path"])):
        failures.append("batch_runner_path must be tracked")
    journeys = document.get("journeys")
    if not isinstance(journeys, list):
        return [*failures, "UI acceptance journeys must be an array"]

    workflow_ids = {
        str(row.get("discovery_id"))
        for row in workflow_reviews
        if row.get("disposition") == "workflow_spec"
    }
    changed_paths_by_journey = changed_paths_by_journey or {}
    seen: set[str] = set()
    batch_manifest_ids: list[str] = []
    accepted_by_invariant: dict[str, set[str]] = {}
    linked_invariants: set[str] = set()

    for index, journey in enumerate(journeys):
        where = f"journeys[{index}]"
        if not isinstance(journey, Mapping):
            failures.append(f"{where} must be an object")
            continue
        journey_id = journey.get("id")
        if not isinstance(journey_id, str) or not journey_id:
            failures.append(f"{where}.id must be a non-empty string")
            continue
        if journey_id in seen:
            failures.append(f"duplicate UI journey {journey_id}")
            continue
        seen.add(journey_id)
        status = journey.get("status")
        source = journey.get("journey_source")
        if source == "batch":
            batch_manifest_ids.append(journey_id)
        if source not in JOURNEY_SOURCES:
            failures.append(f"{journey_id} has unknown journey_source {source!r}")
        if status not in STATUSES:
            failures.append(f"{journey_id} has unknown status {status!r}")
        expected_fields = COMMON_FIELDS
        if status in {"provisional", "stale", "ux_accepted"}:
            expected_fields = EVIDENCE_FIELDS
        if status == "stale":
            expected_fields = EVIDENCE_FIELDS | {"stale_reason"}
        if status == "ux_accepted":
            expected_fields = EVIDENCE_FIELDS | {"reviewer"}
        extra = sorted(set(journey) - expected_fields)
        missing = sorted(expected_fields - set(journey))
        if extra:
            failures.append(f"{journey_id} has unexpected fields: {', '.join(extra)}")
        if missing:
            failures.append(f"{journey_id} missing fields: {', '.join(missing)}")
        for field in ("title", "surface"):
            if not isinstance(journey.get(field), str) or not journey[field]:
                failures.append(f"{journey_id}.{field} must be a non-empty string")
        if not _DATE_RE.fullmatch(str(journey.get("last_reviewed", ""))):
            failures.append(f"{journey_id}.last_reviewed must use YYYY-MM-DD")
        if not isinstance(journey.get("source_transformation"), bool):
            failures.append(f"{journey_id}.source_transformation must be boolean")

        linked_workflows = _string_array(journey, "workflow_candidate_ids", journey_id, failures)
        for workflow_id in linked_workflows:
            if workflow_id not in workflow_ids:
                failures.append(f"{journey_id} references unknown workflow candidate {workflow_id}")
        invariant_ids = _string_array(journey, "invariant_ids", journey_id, failures)
        supporting_ids = _string_array(journey, "supporting_test_ids", journey_id, failures)
        implementation_paths = _string_array(
            journey, "implementation_paths", journey_id, failures, require_nonempty=True
        )
        for relative in implementation_paths:
            if not _safe_relative(relative):
                failures.append(f"{journey_id} has unsafe implementation path {relative!r}")
        for invariant_id in invariant_ids:
            invariant = invariants.get(invariant_id)
            if invariant is None:
                failures.append(f"{journey_id} references unknown invariant {invariant_id}")
                continue
            linked_invariants.add(invariant_id)
            if journey.get("source_transformation") is True and invariant.get("risk") == "silent_corruption":
                pass
        if journey.get("source_transformation") is True and not any(
            invariants.get(invariant_id, {}).get("risk") == "silent_corruption"
            for invariant_id in invariant_ids
        ):
            failures.append(f"{journey_id} source transformation requires a silent_corruption invariant")

        for test_id in supporting_ids:
            test = tests.get(test_id)
            if test is None:
                failures.append(f"{journey_id} references unknown supporting test {test_id}")
                continue
            if test.get("discovery_source_kind") != "vscode_test" or "pr" not in test.get("suite_tiers", []):
                failures.append(f"{journey_id} supporting test {test_id} must be a PR-routed VS Code test")
            if not set(invariant_ids) <= set(test.get("invariants", [])):
                failures.append(f"{journey_id} supporting test {test_id} does not cover all journey invariants")

        changed = list(changed_paths_by_journey.get(journey_id, ()))
        if status == "stale":
            if not changed:
                failures.append(f"{journey_id} stale journey has no changed implementation path")
            if not isinstance(journey.get("stale_reason"), str) or not journey["stale_reason"]:
                failures.append(f"{journey_id}.stale_reason must be a non-empty string")
        elif status in {"provisional", "ux_accepted"} and changed:
            failures.append(
                f"{journey_id} visible implementation changed after evidence capture: {', '.join(changed)}"
            )

        if status in {"provisional", "stale", "ux_accepted"}:
            _validate_evidence(
                root,
                journey_id,
                journey,
                failures,
                require_tracked_files=require_tracked_files,
            )
        if status == "ux_accepted":
            if journey.get("reviewer") == journey.get("implementer"):
                failures.append(f"{journey_id} reviewer must differ from implementer")
            if not any(
                tests.get(test_id, {}).get("test_class") == "ui_journey_acceptance"
                for test_id in supporting_ids
            ):
                failures.append(f"{journey_id} ux_accepted requires a ui_journey_acceptance test")
            for invariant_id in invariant_ids:
                accepted_by_invariant.setdefault(invariant_id, set()).add(journey_id)

    expected_batch_ids = list(batch_journey_ids)
    for journey_id in expected_batch_ids:
        if journey_id not in batch_manifest_ids:
            failures.append(f"missing manifest journey for batch runner ID {journey_id}")
    for journey_id in batch_manifest_ids:
        if journey_id not in expected_batch_ids:
            failures.append(f"invented batch journey {journey_id}")
    if batch_manifest_ids != expected_batch_ids:
        failures.append("batch UI journeys must use runner declaration order")

    for invariant_id in linked_invariants:
        invariant = invariants.get(invariant_id, {})
        if invariant.get("status") == "validated" and not accepted_by_invariant.get(invariant_id):
            failures.append(
                f"{invariant_id} validated UI invariant requires an ux_accepted journey"
            )
    return failures


def load_ui_acceptance_document(root: Path) -> dict[str, Any]:
    path = root.resolve() / MANIFEST_PATH
    document = tomllib.loads(path.read_text(encoding="utf-8"))
    if not isinstance(document, dict):
        raise ValueError("UI acceptance document must be an object")
    return document


def batch_journey_ids(root: Path, document: Mapping[str, Any]) -> tuple[str, ...]:
    relative = document.get("batch_runner_path")
    if not _safe_file(root, relative):
        raise ValueError("batch runner is missing or unsafe")
    text = (root / str(relative)).read_text(encoding="utf-8")
    try:
        start = text.index("const JOURNEYS = [")
        end = text.index("\n];", start)
    except ValueError as exc:
        raise ValueError("batch runner JOURNEYS declaration cannot be resolved") from exc
    ids = tuple(re.findall(r'(?m)^\s{2,}id: "([^"]+)",$', text[start:end]))
    if not ids or len(ids) != len(set(ids)):
        raise ValueError("batch runner journey IDs must be nonempty and unique")
    return ids


def changed_paths_since_evidence(
    root: Path, document: Mapping[str, Any]
) -> tuple[dict[str, list[str]], list[str]]:
    changed: dict[str, list[str]] = {}
    failures: list[str] = []
    for journey in document.get("journeys", []):
        if not isinstance(journey, Mapping) or journey.get("status") == "evidence_missing":
            continue
        journey_id = str(journey.get("id", "<unknown>"))
        revision = journey.get("source_commit")
        if not _COMMIT_RE.fullmatch(str(revision or "")):
            continue
        ancestry = subprocess.run(
            ["git", "merge-base", "--is-ancestor", str(revision), "HEAD"],
            cwd=root,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if ancestry.returncode != 0:
            failures.append(f"{journey_id} source_commit must be an ancestor of HEAD")
            continue
        pathspecs = [
            *journey.get("implementation_paths", []),
            *journey.get("runner_paths", []),
        ]
        result = subprocess.run(
            ["git", "diff", "--name-only", str(revision), "HEAD", "--", *pathspecs],
            cwd=root,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        if result.returncode != 0:
            failures.append(f"{journey_id} evidence freshness diff failed")
            continue
        changed[journey_id] = [line for line in result.stdout.splitlines() if line]
    return changed, failures


def _validate_evidence(
    root: Path,
    journey_id: str,
    journey: Mapping[str, Any],
    failures: list[str],
    *,
    require_tracked_files: bool,
) -> None:
    if not _COMMIT_RE.fullmatch(str(journey.get("source_commit", ""))):
        failures.append(f"{journey_id}.source_commit must be a clean full Git SHA")
    if not isinstance(journey.get("implementer"), str) or not journey["implementer"]:
        failures.append(f"{journey_id}.implementer must be a non-empty string")
    runner_paths = _string_array(journey, "runner_paths", journey_id, failures, require_nonempty=True)
    for relative in runner_paths:
        if not _safe_file(root, relative):
            failures.append(f"{journey_id} runner path must name a contained regular file: {relative!r}")
        elif require_tracked_files and not _is_tracked(root, relative):
            failures.append(f"{journey_id} runner path must be tracked: {relative}")
    evidence = journey.get("evidence")
    if not isinstance(evidence, list):
        failures.append(f"{journey_id}.evidence must be an array")
        return
    themes: list[str] = []
    for index, row in enumerate(evidence):
        where = f"{journey_id}.evidence[{index}]"
        if not isinstance(row, Mapping) or set(row) != EVIDENCE_ROW_FIELDS:
            failures.append(f"{where} fields are not closed")
            continue
        theme = row.get("theme")
        if isinstance(theme, str):
            themes.append(theme)
        for kind in ("screenshot", "result"):
            relative = row.get(f"{kind}_path")
            expected = row.get(f"{kind}_sha256")
            if not _safe_file(root, relative):
                failures.append(f"{where}.{kind}_path must name a contained regular file")
                continue
            if require_tracked_files and not _is_tracked(root, str(relative)):
                failures.append(f"{where}.{kind}_path must be tracked")
            actual = hashlib.sha256((root / str(relative)).read_bytes()).hexdigest()
            if not _SHA_RE.fullmatch(str(expected)) or actual != expected:
                failures.append(f"{where} {kind} digest mismatch")
    if set(themes) != THEMES or len(themes) != len(THEMES):
        failures.append(f"{journey_id} evidence must contain the exact theme triplet")


def _string_array(
    record: Mapping[str, Any],
    field: str,
    owner: str,
    failures: list[str],
    *,
    require_nonempty: bool = False,
) -> list[str]:
    value = record.get(field)
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        failures.append(f"{owner}.{field} must be a string array")
        return []
    if require_nonempty and not value:
        failures.append(f"{owner}.{field} must not be empty")
    if len(value) != len(set(value)):
        failures.append(f"{owner}.{field} must not contain duplicates")
    return value


def _safe_relative(value: Any) -> bool:
    if not isinstance(value, str) or not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return (
        not path.is_absolute()
        and "." not in path.parts
        and ".." not in path.parts
        and path.as_posix() == value
    )


def _safe_file(root: Path, value: Any) -> bool:
    if not _safe_relative(value):
        return False
    candidate = root / str(value)
    if candidate.is_symlink() or not candidate.is_file():
        return False
    try:
        candidate.resolve().relative_to(root.resolve())
    except ValueError:
        return False
    return True


def _is_tracked(root: Path, relative: str) -> bool:
    result = subprocess.run(
        ["git", "ls-files", "--error-unmatch", "--", relative],
        cwd=root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return result.returncode == 0
