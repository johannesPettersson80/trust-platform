"""Closed metadata and live-identity contract for ignored tests."""

from __future__ import annotations

import re
import subprocess
from collections import defaultdict
from collections.abc import Mapping, Sequence, Set
from pathlib import Path, PurePosixPath
from typing import Any

from ..ignored_test_models import IgnoredTestFact
from ..test_refactor_identity import validate_live_path
from .constants import AREAS, STATUSES


IGNORE_CLASSES = {
    "red_protective",
    "lab_required",
    "perf_soak",
    "manual",
    "flaky_quarantined",
    "obsolete",
    "unknown",
}
IGNORE_MECHANISMS = {
    "rust_attribute",
    "rust_cfg_attr",
    "vscode_runtime_skip",
    "playwright_literal_skip",
}
IGNORE_STATES = {"ignored", "conditional"}
IGNORED_SOURCE_KINDS = {
    "rust_integration_test",
    "rust_unit_test",
    "vscode_test",
    "playwright_test",
}
BASE_FIELDS = {
    "schema_version",
    "id",
    "discovery_id",
    "discovery_source_kind",
    "path",
    "name",
    "ignore_state",
    "ignore_reason",
    "ignore_mechanism",
    "owner",
    "area",
    "status",
    "ignore_class",
    "reason",
    "unblock_condition",
    "last_reviewed",
}
RED_FIELDS = {"linked_rows", "expected_red_symptom"}
LAB_FIELDS = {
    "required_env_vars",
    "hardware_topology",
    "hardware_topology_ref",
    "public_claim_impact",
}
FLAKY_FIELDS = {"last_observed_failure", "failure_signature", "evidence_ref"}
CLASS_FIELDS = RED_FIELDS | LAB_FIELDS | FLAKY_FIELDS
ALLOWED_FIELDS = BASE_FIELDS | {"test_id", "_path"} | CLASS_FIELDS

RECORD_ID_RE = re.compile(r"^IGNORED_[A-Z0-9_]+$")
TEST_ID_RE = re.compile(r"^TEST_[A-Z0-9_]+$")
DISCOVERY_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
ENV_RE = re.compile(r"^[A-Z][A-Z0-9_]*$")
CHECKBOX_RE = re.compile(
    r"^\s*-\s+\[(?: |x|X)\]\s+`(?P<id>[A-Z][A-Z0-9_.-]+)`"
)
SECRET_ENV_FRAGMENTS = {
    "API_KEY",
    "CREDENTIAL",
    "KEY_FILE",
    "KEY_MATERIAL",
    "KEY_PATH",
    "PASSWORD",
    "PRIVATE_KEY",
    "SECRET",
    "TOKEN",
}


def validate_ignored_test_records(
    *,
    root: Path,
    ignored_tests: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    checklist_row_ids: Mapping[str, Sequence[str]] | Set[str],
    facts: Sequence[IgnoredTestFact] | None = None,
) -> list[str]:
    """Validate hand-owned records, optionally joining the complete live inventory."""

    failures: list[str] = []
    discovery_owners: dict[str, list[str]] = defaultdict(list)
    test_owners: dict[str, list[str]] = defaultdict(list)

    for key in sorted(ignored_tests):
        record = ignored_tests[key]
        label = f"ignored test {key}"
        _validate_record(
            root=root,
            key=key,
            record=record,
            tests=tests,
            checklist_row_ids=checklist_row_ids,
            failures=failures,
        )
        discovery_id = record.get("discovery_id")
        if isinstance(discovery_id, str):
            discovery_owners[discovery_id].append(key)
        test_id = record.get("test_id")
        if isinstance(test_id, str):
            test_owners[test_id].append(key)

    for discovery_id, owners in sorted(discovery_owners.items()):
        if len(owners) != 1:
            failures.append(
                f"duplicate discovery_id {discovery_id} is used by {', '.join(owners)}"
            )
    for test_id, owners in sorted(test_owners.items()):
        if len(owners) != 1:
            failures.append(f"duplicate test_id {test_id} is used by {', '.join(owners)}")

    if facts is not None:
        _validate_exhaustive_join(ignored_tests, facts, failures)
    return sorted(set(failures))


def load_checklist_row_ids(root: Path) -> dict[str, tuple[str, ...]]:
    """Return every checklist checkbox ID with all paths that declare it."""

    root = root.resolve()
    checklist_root = root / "docs/internal/testing/checklists"
    locations: dict[str, list[str]] = defaultdict(list)
    if not checklist_root.is_dir():
        return {}
    for path in sorted(checklist_root.rglob("*.md")):
        try:
            lines = path.read_text().splitlines()
        except (OSError, UnicodeError):
            continue
        relative = path.relative_to(root).as_posix()
        for line_number, line in enumerate(lines, start=1):
            match = CHECKBOX_RE.match(line)
            if match:
                locations[match.group("id")].append(f"{relative}:{line_number}")
    return {row_id: tuple(paths) for row_id, paths in sorted(locations.items())}


def _validate_record(
    *,
    root: Path,
    key: str,
    record: Mapping[str, Any],
    tests: Mapping[str, Mapping[str, Any]],
    checklist_row_ids: Mapping[str, Sequence[str]] | Set[str],
    failures: list[str],
) -> None:
    label = f"ignored test {key}"
    fields = set(record)
    for field in sorted(BASE_FIELDS - fields):
        failures.append(f"{label} missing required field {field}")
    for field in sorted(fields - ALLOWED_FIELDS):
        failures.append(f"{label} has additional field {field}")
    if record.get("schema_version") != 2:
        failures.append(f"{label} must use schema_version 2")
    if record.get("id") != key:
        failures.append(f"{label} id does not match record key")
    if not RECORD_ID_RE.fullmatch(str(record.get("id", ""))):
        failures.append(f"{label} id is invalid")

    for field in (
        "discovery_source_kind",
        "path",
        "name",
        "ignore_reason",
        "owner",
        "reason",
        "unblock_condition",
    ):
        if not _nonempty(record.get(field)):
            failures.append(f"{label} requires non-empty {field}")
    discovery_id = record.get("discovery_id")
    if not isinstance(discovery_id, str) or not DISCOVERY_ID_RE.fullmatch(discovery_id):
        failures.append(f"{label} has invalid discovery_id {discovery_id!r}")
    if record.get("discovery_source_kind") not in IGNORED_SOURCE_KINDS:
        failures.append(f"{label} has unsupported discovery_source_kind")
    if record.get("ignore_state") not in IGNORE_STATES:
        failures.append(f"{label} has unsupported ignore_state")
    if record.get("ignore_mechanism") not in IGNORE_MECHANISMS:
        failures.append(f"{label} has unsupported ignore_mechanism")
    _validate_mechanism_shape(record, label, failures)
    if not _safe_relative_path(record.get("path")):
        failures.append(f"{label} path must be normalized and workspace-relative")
    if record.get("area") not in AREAS:
        failures.append(f"{label} has unknown area {record.get('area')!r}")
    if record.get("status") not in STATUSES:
        failures.append(f"{label} has unknown status {record.get('status')!r}")
    if not DATE_RE.fullmatch(str(record.get("last_reviewed", ""))):
        failures.append(f"{label} last_reviewed must be YYYY-MM-DD")

    ignore_class = record.get("ignore_class")
    if ignore_class not in IGNORE_CLASSES:
        failures.append(f"{label} has unknown ignore_class {ignore_class!r}")
    if ignore_class == "unknown" and record.get("status") != "gap_open":
        failures.append(f"{label} unknown classification must use status gap_open")
    _validate_class_fields(
        root=root,
        record=record,
        ignore_class=ignore_class,
        checklist_row_ids=checklist_row_ids,
        label=label,
        failures=failures,
    )

    test_id = record.get("test_id")
    if test_id is not None:
        if not isinstance(test_id, str) or not TEST_ID_RE.fullmatch(test_id):
            failures.append(f"{label} has invalid optional test_id {test_id!r}")
        else:
            catalog = tests.get(test_id)
            if catalog is None:
                failures.append(f"{label} references unknown optional catalog test {test_id}")
            elif catalog.get("discovery_id") != discovery_id:
                failures.append(f"{label} catalog discovery_id does not match ignored record")


def _validate_class_fields(
    *,
    root: Path,
    record: Mapping[str, Any],
    ignore_class: Any,
    checklist_row_ids: Mapping[str, Sequence[str]] | Set[str],
    label: str,
    failures: list[str],
) -> None:
    required = {
        "red_protective": RED_FIELDS,
        "lab_required": LAB_FIELDS,
        "flaky_quarantined": FLAKY_FIELDS,
    }.get(ignore_class, set())
    for field in sorted(required - set(record)):
        failures.append(f"{label} {ignore_class} requires {field}")
    for field in sorted((CLASS_FIELDS - required) & set(record)):
        failures.append(f"{label} {ignore_class} forbids class-only field {field}")

    if ignore_class == "red_protective":
        linked_rows = _string_list(record.get("linked_rows"), f"{label} linked_rows", failures)
        if not linked_rows:
            failures.append(f"{label} red_protective linked_rows must not be empty")
        for row_id in linked_rows:
            locations = _checklist_locations(checklist_row_ids, row_id)
            if not locations:
                failures.append(f"{label} linked_rows references unknown checklist row {row_id}")
            elif len(locations) != 1:
                failures.append(
                    f"{label} linked_rows checklist row {row_id} resolves {len(locations)} times"
                )
        if not _nonempty(record.get("expected_red_symptom")):
            failures.append(f"{label} requires non-empty expected_red_symptom")
    elif ignore_class == "lab_required":
        env_vars = _string_list(
            record.get("required_env_vars"), f"{label} required_env_vars", failures
        )
        if not env_vars:
            failures.append(f"{label} lab_required required_env_vars must not be empty")
        for env_var in env_vars:
            if not ENV_RE.fullmatch(env_var):
                failures.append(f"{label} required_env_vars has invalid name {env_var!r}")
            if any(fragment in env_var for fragment in SECRET_ENV_FRAGMENTS):
                failures.append(
                    f"{label} required_env_vars {env_var} must not name secret-bearing configuration"
                )
        for field in ("hardware_topology", "public_claim_impact"):
            if not _nonempty(record.get(field)):
                failures.append(f"{label} requires non-empty {field}")
        _validate_durable_ref(root, record.get("hardware_topology_ref"), label, "hardware_topology_ref", failures)
    elif ignore_class == "flaky_quarantined":
        if not DATE_RE.fullmatch(str(record.get("last_observed_failure", ""))):
            failures.append(f"{label} last_observed_failure must be YYYY-MM-DD")
        if not _nonempty(record.get("failure_signature")):
            failures.append(f"{label} requires non-empty failure_signature")
        _validate_durable_ref(root, record.get("evidence_ref"), label, "evidence_ref", failures)


def _validate_exhaustive_join(
    ignored_tests: Mapping[str, Mapping[str, Any]],
    facts: Sequence[IgnoredTestFact],
    failures: list[str],
) -> None:
    facts_by_id: dict[str, list[IgnoredTestFact]] = defaultdict(list)
    for fact in facts:
        facts_by_id[fact.discovery_id].append(fact)
        if fact.ignore_state not in IGNORE_STATES:
            failures.append(
                f"discovered fact {fact.discovery_id} is not an ignored or conditional observation"
            )
    records_by_id: dict[str, list[tuple[str, Mapping[str, Any]]]] = defaultdict(list)
    for key, record in ignored_tests.items():
        discovery_id = record.get("discovery_id")
        if isinstance(discovery_id, str):
            records_by_id[discovery_id].append((key, record))

    for discovery_id, matches in sorted(facts_by_id.items()):
        if len(matches) != 1:
            failures.append(
                f"discovered ignore {discovery_id} resolves to {len(matches)} inventory facts"
            )
            continue
        records = records_by_id.get(discovery_id, [])
        if len(records) != 1:
            if not records:
                failures.append(f"discovered ignore {discovery_id} has no registry record")
            else:
                failures.append(
                    f"discovered ignore {discovery_id} resolves to {len(records)} registry records"
                )
            continue
        key, record = records[0]
        fact = matches[0]
        for field, actual in (
            ("path", fact.path),
            ("name", fact.name),
            ("discovery_source_kind", fact.discovery_source_kind),
            ("ignore_state", fact.ignore_state),
            ("ignore_reason", fact.ignore_reason),
            ("ignore_mechanism", fact.ignore_mechanism),
        ):
            if record.get(field) != actual:
                failures.append(
                    f"ignored test {key} {field} does not match discovered fact: "
                    f"registry {record.get(field)!r}, discovered {actual!r}"
                )
    for discovery_id, records in sorted(records_by_id.items()):
        if discovery_id not in facts_by_id:
            failures.append(
                f"registry discovery_id {discovery_id} is absent from discovered ignored facts"
            )


def _validate_mechanism_shape(
    record: Mapping[str, Any], label: str, failures: list[str]
) -> None:
    source = record.get("discovery_source_kind")
    state = record.get("ignore_state")
    mechanism = record.get("ignore_mechanism")
    expected = {
        ("rust_integration_test", "ignored"): "rust_attribute",
        ("rust_unit_test", "ignored"): "rust_attribute",
        ("rust_integration_test", "conditional"): "rust_cfg_attr",
        ("rust_unit_test", "conditional"): "rust_cfg_attr",
        ("vscode_test", "conditional"): "vscode_runtime_skip",
        ("playwright_test", "ignored"): "playwright_literal_skip",
    }.get((source, state))
    if expected is None or mechanism != expected:
        failures.append(
            f"{label} ignore_mechanism does not match discovery source/state; expected {expected!r}"
        )


def _validate_durable_ref(
    root: Path,
    value: Any,
    label: str,
    field: str,
    failures: list[str],
) -> None:
    if not isinstance(value, str) or not _safe_relative_path(value):
        failures.append(f"{label} {field} must be a normalized workspace-relative path")
        return
    before = len(failures)
    validate_live_path(root, value, f"{label} {field}", failures)
    if len(failures) == before and not (root / value).is_file():
        failures.append(f"{label} {field} must identify a regular file")
    if len(failures) != before or not _is_git_worktree(root):
        return
    ignored = subprocess.run(
        ["git", "check-ignore", "-q", "--", value],
        cwd=root,
        check=False,
    )
    if ignored.returncode == 0:
        failures.append(f"{label} {field} path is gitignored: {value}")
        return
    if ignored.returncode != 1:
        failures.append(f"{label} {field} git check-ignore failed for {value}")
        return
    tracked = subprocess.run(
        ["git", "ls-files", "--error-unmatch", "--", value],
        cwd=root,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if tracked.returncode != 0:
        failures.append(f"{label} {field} must identify a tracked durable file: {value}")


def _checklist_locations(
    checklist_row_ids: Mapping[str, Sequence[str]] | Set[str], row_id: str
) -> tuple[str, ...]:
    if isinstance(checklist_row_ids, Mapping):
        return tuple(checklist_row_ids.get(row_id, ()))
    return ("test-fixture",) if row_id in checklist_row_ids else ()


def _string_list(value: Any, label: str, failures: list[str]) -> list[str]:
    if not isinstance(value, list) or not all(_nonempty(item) for item in value):
        failures.append(f"{label} must be a string array")
        return []
    if value != sorted(set(value)):
        failures.append(f"{label} must be sorted and unique")
    return list(value)


def _safe_relative_path(value: Any) -> bool:
    if not isinstance(value, str) or not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return (
        not path.is_absolute()
        and ".." not in path.parts
        and "." not in path.parts
        and path.as_posix() == value
    )


def _nonempty(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _is_git_worktree(root: Path) -> bool:
    result = subprocess.run(
        ["git", "rev-parse", "--is-inside-work-tree"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0 and result.stdout.strip() == "true"
