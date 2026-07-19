"""Phase 4A spec-gap close-out and safety-critical validation rules."""

from __future__ import annotations

import subprocess
from collections.abc import Mapping
from pathlib import Path, PurePosixPath
from typing import Any

from .integrity import OPEN_GAP_RESOLUTIONS, RUNNABLE_TEST_STATUSES


RESOLUTION_SOURCE_AUTHORITIES = {
    "normative_product",
    "reviewed_decision",
    "reviewed_deviation",
}
DEFERRAL_SOURCE_AUTHORITIES = {"reviewed_decision", "reviewed_deviation"}


def validate_spec_gap_closure(
    *,
    root: Path,
    spec_gaps: Mapping[str, Mapping[str, Any]],
    spec_sources: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    required_specs: Mapping[str, Mapping[str, Any]] | None = None,
    risks: Mapping[str, Mapping[str, Any]] | None = None,
) -> list[str]:
    """Validate closed gaps and reject safety-critical validation over open gaps."""

    root = root.resolve()
    failures: list[str] = []
    required_specs = required_specs or {}
    risks = risks or {}
    for gap_id in sorted(spec_gaps):
        gap = spec_gaps[gap_id]
        if gap.get("resolution_status") == "closed":
            _validate_closed_gap(
                root=root,
                gap_id=gap_id,
                gap=gap,
                spec_sources=spec_sources,
                tests=tests,
                evidence=evidence,
                invariants=invariants,
                required_specs=required_specs,
                risks=risks,
                failures=failures,
            )

    open_gap_ids = {
        gap_id
        for gap_id, gap in spec_gaps.items()
        if gap.get("resolution_status") in OPEN_GAP_RESOLUTIONS
    }
    reverse_links: dict[str, set[str]] = {}
    for gap_id, gap in spec_gaps.items():
        if gap_id not in open_gap_ids:
            continue
        affected_invariants = gap.get("affected_invariants", [])
        if not isinstance(affected_invariants, list):
            affected_invariants = []
        for invariant_id in affected_invariants:
            if isinstance(invariant_id, str):
                reverse_links.setdefault(invariant_id, set()).add(gap_id)

    for invariant_id in sorted(invariants):
        record = invariants[invariant_id]
        if record.get("risk") != "safety_critical" or record.get("status") != "validated":
            continue
        spec_gap_refs = record.get("spec_gap_refs", [])
        if not isinstance(spec_gap_refs, list):
            spec_gap_refs = []
        referenced = {
            value for value in spec_gap_refs if isinstance(value, str) and value in open_gap_ids
        }
        referenced.update(reverse_links.get(invariant_id, set()))
        coverage = record.get("coverage", {})
        cells = coverage.get("cells", []) if isinstance(coverage, Mapping) else []
        if not isinstance(cells, list):
            cells = []
        for cell in cells:
            if not isinstance(cell, Mapping):
                continue
            gap_ref = cell.get("spec_gap_ref")
            if isinstance(gap_ref, str) and gap_ref in open_gap_ids:
                referenced.add(gap_ref)
        if referenced:
            failures.append(
                f"{invariant_id} safety-critical validated is blocked by open spec gaps: "
                f"{', '.join(sorted(referenced))}"
            )
    return sorted(set(failures))


def _validate_closed_gap(
    *,
    root: Path,
    gap_id: str,
    gap: Mapping[str, Any],
    spec_sources: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    evidence: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    required_specs: Mapping[str, Mapping[str, Any]],
    risks: Mapping[str, Mapping[str, Any]],
    failures: list[str],
) -> None:
    resolution_ref = gap.get("resolution_source_ref")
    source = spec_sources.get(resolution_ref) if isinstance(resolution_ref, str) else None
    if source is None:
        failures.append(
            f"{gap_id} closed gap requires resolution_source_ref naming the updated owning source"
        )
    else:
        if not _contains(gap.get("candidate_spec_sources"), resolution_ref):
            failures.append(
                f"{gap_id} resolution_source_ref must be one of candidate_spec_sources"
            )
        authority = source.get("authority")
        if authority not in RESOLUTION_SOURCE_AUTHORITIES:
            failures.append(
                f"{gap_id} resolution source authority {authority!r} cannot close a spec gap"
            )
        if source.get("source_status") != "active":
            failures.append(f"{gap_id} resolution source must be active")
        if source.get("oracle_eligible") is not True:
            failures.append(f"{gap_id} provenance-only resolution source cannot close a spec gap")
        _validate_resolution_source_path(
            root,
            source.get("path"),
            gap_id,
            failures,
        )
        source_reviewed = source.get("last_reviewed")
        gap_reviewed = gap.get("last_reviewed")
        if (
            isinstance(source_reviewed, str)
            and isinstance(gap_reviewed, str)
            and source_reviewed < gap_reviewed
        ):
            failures.append(
                f"{gap_id} resolution source review predates the gap close-out review"
            )

    affected_tests = gap.get("affected_tests", [])
    mapped_tests_ok = bool(affected_tests)
    if not isinstance(affected_tests, list):
        mapped_tests_ok = False
        affected_tests = []
    for test_id in affected_tests:
        record = tests.get(test_id) if isinstance(test_id, str) else None
        if record is None or record.get("status") not in RUNNABLE_TEST_STATUSES:
            mapped_tests_ok = False
            failures.append(f"{gap_id} affected test {test_id!r} is not a written mapped test")

    deferral_ref = gap.get("test_deferral_ref")
    deferred = _is_active_deferral(deferral_ref, spec_sources)
    if deferral_ref is not None and not deferred:
        failures.append(
            f"{gap_id} test_deferral_ref must name an active reviewed decision or deviation"
        )
    if not mapped_tests_ok and not deferred:
        failures.append(
            f"{gap_id} closure requires a written mapped test or explicit reviewed deferral"
        )

    evidence_ids = gap.get("closeout_evidence", [])
    if not isinstance(evidence_ids, list) or not evidence_ids:
        failures.append(f"{gap_id} closed gap requires closeout_evidence")
        return
    expected_linked_tests = sorted(
        test_id for test_id in affected_tests if isinstance(test_id, str)
    ) if mapped_tests_ok else []
    evidence_linked_tests: set[str] = set()
    for evidence_id in evidence_ids:
        record = evidence.get(evidence_id) if isinstance(evidence_id, str) else None
        if record is None:
            failures.append(f"{gap_id} closeout_evidence references unknown {evidence_id!r}")
            continue
        if not _contains(record.get("linked_spec_gaps"), gap_id):
            failures.append(
                f"{gap_id} closeout evidence {evidence_id} does not back-link the spec gap"
            )
        linked_tests = record.get("linked_tests", [])
        if not isinstance(linked_tests, list) or not all(
            isinstance(test_id, str) for test_id in linked_tests
        ):
            failures.append(
                f"{gap_id} closeout evidence {evidence_id} linked_tests must be a string array"
            )
        else:
            evidence_linked_tests.update(linked_tests)
    if sorted(evidence_linked_tests) != expected_linked_tests:
        failures.append(
            f"{gap_id} aggregate closeout evidence linked_tests do not match close-out disposition"
        )

    live_refs = _live_gap_references(
        gap_id=gap_id,
        required_specs=required_specs,
        invariants=invariants,
        tests=tests,
        risks=risks,
    )
    if live_refs:
        failures.append(
            f"{gap_id} closed gap remains referenced by active metadata: {', '.join(live_refs)}"
        )


def _is_active_deferral(
    value: Any,
    spec_sources: Mapping[str, Mapping[str, Any]],
) -> bool:
    if not isinstance(value, str):
        return False
    source = spec_sources.get(value)
    return bool(
        source
        and source.get("authority") in DEFERRAL_SOURCE_AUTHORITIES
        and source.get("source_status") == "active"
        and source.get("oracle_eligible") is True
    )


def _live_gap_references(
    *,
    gap_id: str,
    required_specs: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    risks: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    references: list[str] = []
    for record_id, record in required_specs.items():
        if record.get("spec_gap_ref") == gap_id:
            references.append(f"required_spec:{record_id}")
    for record_id, record in invariants.items():
        if _contains(record.get("spec_gap_refs"), gap_id):
            references.append(f"invariant:{record_id}:spec_gap_refs")
        coverage = record.get("coverage", {})
        cells = coverage.get("cells", []) if isinstance(coverage, Mapping) else []
        if isinstance(cells, list) and any(
            isinstance(cell, Mapping) and cell.get("spec_gap_ref") == gap_id
            for cell in cells
        ):
            references.append(f"invariant:{record_id}:coverage")
        behavior = record.get("behavior", [])
        if isinstance(behavior, list) and any(
            isinstance(row, Mapping) and row.get("spec_gap_ref") == gap_id
            for row in behavior
        ):
            references.append(f"invariant:{record_id}:behavior")
    for record_id, record in tests.items():
        if record.get("spec_gap_ref") == gap_id:
            references.append(f"test:{record_id}")
    for record_id, record in risks.items():
        if _contains(record.get("related_spec_gaps"), gap_id):
            references.append(f"risk:{record_id}")
    return sorted(references)


def _contains(value: Any, expected: str) -> bool:
    return isinstance(value, list) and expected in value


def _is_workspace_path(value: Any) -> bool:
    if not isinstance(value, str) or not value or "\\" in value or "://" in value:
        return False
    path = PurePosixPath(value)
    return not path.is_absolute() and ".." not in path.parts and "." not in path.parts


def _validate_resolution_source_path(
    root: Path,
    value: Any,
    gap_id: str,
    failures: list[str],
) -> None:
    label = f"{gap_id} resolution source"
    if not _is_workspace_path(value):
        failures.append(
            f"{label} must name a tracked workspace path, not external-reference-only metadata"
        )
        return

    relative = PurePosixPath(value)
    candidate = root
    for part in relative.parts:
        candidate /= part
        if candidate.is_symlink():
            failures.append(f"{label} path contains a symlink component: {value}")
            return
    try:
        candidate.resolve().relative_to(root)
    except ValueError:
        failures.append(f"{label} path escapes the workspace: {value}")
        return
    if not candidate.is_file():
        failures.append(f"{label} must identify a regular file: {value}")
        return

    ignored = subprocess.run(
        ["git", "check-ignore", "-q", "--", value],
        cwd=root,
        check=False,
    )
    if ignored.returncode == 0:
        failures.append(f"{label} path is gitignored: {value}")
        return
    if ignored.returncode != 1:
        failures.append(f"{label} git check-ignore failed for {value}")
        return
    tracked = subprocess.run(
        ["git", "ls-files", "--error-unmatch", "--", value],
        cwd=root,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if tracked.returncode != 0:
        failures.append(f"{label} must identify a tracked durable file: {value}")
