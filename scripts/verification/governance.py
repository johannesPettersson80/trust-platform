"""Phase 14 governance, staleness, ownership, and changed-file policy."""

from __future__ import annotations

import argparse
import sys
import tomllib
from datetime import date
from pathlib import Path
from typing import Any, Mapping, Sequence

from .metadata_validator.constants import AREAS, CASE_FAMILIES, ROOT
from .phase16_readiness import is_product_path, normalize_changed_path


GOVERNANCE_PATH = "verification/governance.toml"
RETIREMENTS_PATH = "verification/retirements.toml"
TOP_FIELDS = {
    "schema_version", "id", "owner", "status", "last_reviewed", "coverage_dimensions",
    "staleness", "suite_composition", "change_policy", "archive_policy", "grace_periods",
    "owner_aliases", "area_owner_rules", "coverage_templates", "review_cadences",
}
GRACE_IDS = ("ignored_unknown", "missing_oracle", "public_claim_disposition", "mutation_survivor")
CADENCE_IDS = ("ignored_test_audit", "hardware_security_release_audit", "mutation_fuzz_review")


def load_governance(root: Path) -> dict[str, Any]:
    return tomllib.loads((root / GOVERNANCE_PATH).read_text(encoding="utf-8"))


def load_retirements(root: Path) -> dict[str, Any]:
    return tomllib.loads((root / RETIREMENTS_PATH).read_text(encoding="utf-8"))


def validate_governance_document(
    document: Mapping[str, Any], *, invariants: Mapping[str, Mapping[str, Any]],
    suites: Mapping[str, Mapping[str, Any]], matrix: Mapping[str, Any],
    retirements: Mapping[str, Any], evidence: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    failures: list[str] = []
    if set(document) != TOP_FIELDS:
        failures.append("governance fields drift from the closed Phase 14 contract")
    for field, expected in (
        ("schema_version", 1), ("id", "VERIFICATION_GOVERNANCE_001"),
        ("owner", "verification"), ("status", "mapped"),
    ):
        if document.get(field) != expected:
            failures.append(f"governance {field} must equal {expected!r}")
    if document.get("coverage_dimensions") != _ordered_dimensions():
        failures.append("governance coverage dimensions drift from the canonical vocabulary")

    staleness = document.get("staleness", {})
    if set(staleness) != {"maximum_active_age_days", "date_field", "future_dates_forbidden"}:
        failures.append("staleness fields drift from the closed contract")
    if staleness.get("maximum_active_age_days") != 90 or staleness.get("date_field") != "last_reviewed" or staleness.get("future_dates_forbidden") is not True:
        failures.append("staleness must require non-future last_reviewed dates within 90 days")

    grace = document.get("grace_periods", [])
    if [row.get("id") for row in grace if isinstance(row, Mapping)] != list(GRACE_IDS):
        failures.append("grace periods drift from reviewed order")
    for row in grace:
        if set(row) != {"id", "trigger", "duration_days", "milestone"}:
            failures.append(f"grace period {row.get('id')} fields drift from the closed contract")
        if row.get("duration_days", -1) < 0:
            failures.append(f"grace period {row.get('id')} has negative duration")
        if row.get("duration_days") == 0 and not row.get("milestone"):
            failures.append(f"grace period {row.get('id')} needs a duration or milestone")

    aliases = document.get("owner_aliases", [])
    alias_map = {row.get("alias"): row.get("canonical") for row in aliases if isinstance(row, Mapping)}
    if len(alias_map) != len(aliases) or any(not key or not value for key, value in alias_map.items()):
        failures.append("owner aliases must be unique non-empty mappings")
    rules = document.get("area_owner_rules", [])
    rule_map = {row.get("area"): row.get("owners") for row in rules if isinstance(row, Mapping)}
    if set(rule_map) != AREAS:
        failures.append("area owner rules must cover every canonical area exactly once")
    for invariant_id, invariant in invariants.items():
        if invariant.get("owner") not in rule_map.get(invariant.get("area"), []):
            failures.append(f"{invariant_id} owner {invariant.get('owner')!r} is not allowed for {invariant.get('area')}")
    for suite_id, suite in suites.items():
        if suite.get("owner") != "verification":
            failures.append(f"suite {suite_id} must be owned by verification")

    composition = document.get("suite_composition", {})
    expected_composition = {
        "includes_semantics": "ordered_display_dependency_only",
        "excludes_semantics": "reviewed_constraint_labels_only",
        "command_inheritance": False,
        "execution_inheritance": False,
        "proof_inheritance": False,
    }
    for field, expected in expected_composition.items():
        if composition.get(field) != expected:
            failures.append(f"suite composition {field} must equal {expected!r}")
    allowed_excludes = set(composition.get("allowed_excludes", []))
    for suite_id, suite in suites.items():
        unknown = sorted(set(suite.get("excludes", [])) - allowed_excludes)
        if unknown:
            failures.append(f"suite {suite_id} uses unreviewed exclude labels: {unknown}")
    failures.extend(_suite_cycles(suites))

    templates = document.get("coverage_templates", [])
    template_map = {row.get("area"): row for row in templates if isinstance(row, Mapping)}
    if set(template_map) != AREAS:
        failures.append("coverage templates must cover every canonical area exactly once")
    matrix_rows = {row.get("id"): row for row in matrix.get("areas", []) if isinstance(row, Mapping)}
    for area in AREAS:
        template = template_map.get(area, {})
        if set(template) != {"area", "required_dimensions", "non_universal_disposition"}:
            failures.append(f"coverage template {area} fields drift from the closed contract")
            continue
        required = template.get("required_dimensions", [])
        if any(value not in CASE_FAMILIES for value in required) or len(required) != len(set(required)):
            failures.append(f"coverage template {area} has unknown or duplicate dimensions")
        matrix_required = matrix_rows.get(area, {}).get("required_case_families", [])
        if required != matrix_required:
            failures.append(f"coverage template {area} does not match matrix required families")
        if template.get("non_universal_disposition") != "per_invariant_required":
            failures.append(f"coverage template {area} must keep non-universal dimensions per invariant")

    policy = document.get("change_policy", {})
    if policy.get("product_change_requires") != ["invariant_update", "catalog_update"]:
        failures.append("product change policy must require invariant and catalog updates")
    if policy.get("public_claim_change_requires") != ["spec_source_update", "invariant_update"]:
        failures.append("public claim policy must require spec-source and invariant updates")

    cadences = document.get("review_cadences", [])
    if [row.get("id") for row in cadences if isinstance(row, Mapping)] != list(CADENCE_IDS):
        failures.append("review cadences drift from reviewed order")
    for row in cadences:
        if set(row) != {"id", "interval_days", "milestone", "last_completed"}:
            failures.append(f"review cadence {row.get('id')} fields drift from the closed contract")
        if row.get("interval_days", -1) < 0 or (row.get("interval_days") == 0 and not row.get("milestone")):
            failures.append(f"review cadence {row.get('id')} needs an interval or milestone")

    archive = document.get("archive_policy", {})
    if archive.get("registry_path") != RETIREMENTS_PATH or archive.get("delete_evidence_records") is not False or archive.get("delete_invariant_records") is not False:
        failures.append("archive policy must keep append-only evidence and invariant tombstones")
    failures.extend(_validate_retirements(retirements, invariants, evidence))
    return sorted(set(failures))


def validate_current_governance(
    document: Mapping[str, Any], *, today: date,
    record_groups: Sequence[tuple[str, Mapping[str, Mapping[str, Any]]]],
    ignored_tests: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]] | None = None,
    spec_sources: Mapping[str, Mapping[str, Any]] | None = None,
    changed_files: Sequence[str] = (),
) -> list[str]:
    failures: list[str] = []
    maximum_age = document["staleness"]["maximum_active_age_days"]
    for kind, records in record_groups:
        for record_id, record in records.items():
            try:
                reviewed = date.fromisoformat(str(record.get("last_reviewed", "")))
            except ValueError:
                failures.append(f"{kind} {record_id} has invalid last_reviewed")
                continue
            age = (today - reviewed).days
            if age < 0:
                failures.append(f"{kind} {record_id} has future last_reviewed {reviewed}")
            elif age > maximum_age:
                failures.append(f"{kind} {record_id} is stale at {age} days")
    grace = {row["id"]: row for row in document["grace_periods"]}
    unknown_days = grace["ignored_unknown"]["duration_days"]
    for record_id, record in ignored_tests.items():
        if record.get("ignore_class") != "unknown":
            continue
        reviewed = date.fromisoformat(str(record["last_reviewed"]))
        if (today - reviewed).days > unknown_days:
            failures.append(f"ignored test {record_id} exceeded the {unknown_days}-day unknown grace period")
    oracle_days = grace["missing_oracle"]["duration_days"]
    for invariant_id, invariant in (invariants or {}).items():
        if invariant.get("risk") not in document["change_policy"]["safety_risks"]:
            continue
        oracle_ref = str(invariant.get("oracle", {}).get("ref", "")).split("#", 1)[0]
        source = (spec_sources or {}).get(oracle_ref)
        eligible = bool(
            source
            and source.get("source_status") == "active"
            and source.get("oracle_eligible") is True
            and source.get("authority") != "public_claim"
        )
        if eligible:
            continue
        reviewed = date.fromisoformat(str(invariant["last_reviewed"]))
        if (today - reviewed).days > oracle_days:
            failures.append(f"invariant {invariant_id} exceeded the {oracle_days}-day missing-oracle grace period")
    for row in document["review_cadences"]:
        if row["interval_days"] <= 0:
            continue
        completed = date.fromisoformat(row["last_completed"])
        if (today - completed).days > row["interval_days"]:
            failures.append(f"review cadence {row['id']} is overdue")
    failures.extend(validate_changed_files(document, changed_files))
    return sorted(set(failures))


def validate_changed_files(document: Mapping[str, Any], changed_files: Sequence[str]) -> list[str]:
    normalized = sorted({path for value in changed_files if (path := normalize_changed_path(value))})
    product = [path for path in normalized if is_product_path(path)]
    public = [path for path in normalized if path == "README.md" or path.startswith("docs/public/")]
    failures: list[str] = []
    if product:
        if not any(path.startswith("verification/invariants/") for path in normalized):
            failures.append("product changes require an invariant update in the same diff")
        if "verification/test-catalog.toml" not in normalized:
            failures.append("product changes require a test-catalog update in the same diff")
    if public:
        if "verification/spec-sources.toml" not in normalized:
            failures.append("public claim changes require a spec-source update in the same diff")
        if not any(path.startswith("verification/invariants/") for path in normalized):
            failures.append("public claim changes require an invariant update in the same diff")
    return failures


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--today", type=date.fromisoformat, default=date.today())
    parser.add_argument("--changed-file", action="append", default=[])
    args = parser.parse_args(argv)
    try:
        from .metadata_validator.core import Validator
        validator = Validator()
        validator.load_records()
        validator.validate()
        document = load_governance(args.root)
        failures = [failure.message for failure in validator.failures]
        failures.extend(validate_current_governance(
            document, today=args.today,
            record_groups=(
                ("spec source", validator.spec_sources), ("spec gap", validator.spec_gaps),
                ("test", validator.tests), ("ignored test", validator.ignored_tests),
                ("risk", validator.risks), ("invariant", validator.invariants),
                ("suite", validator.suites),
            ),
            ignored_tests=validator.ignored_tests,
            invariants=validator.invariants,
            spec_sources=validator.spec_sources,
            changed_files=args.changed_file,
        ))
    except (OSError, ValueError, tomllib.TOMLDecodeError) as exc:
        print(f"verification governance: FAIL: {exc}", file=sys.stderr)
        return 1
    if failures:
        for failure in sorted(set(failures)):
            print(f"verification governance: FAIL: {failure}", file=sys.stderr)
        return 1
    print(f"verification governance: PASS ({len(args.changed_file)} changed paths)")
    return 0


def _validate_retirements(retirements, invariants, evidence):
    failures = []
    rows = retirements.get("retirements")
    if not isinstance(rows, list):
        return ["retirements registry must contain an array"]
    seen = set()
    for row in rows:
        if set(row) != {"kind", "id", "owner", "rationale", "replacement", "retired_at", "evidence_refs"}:
            failures.append("retirement fields drift from the append-only contract")
            continue
        key = (row["kind"], row["id"])
        if key in seen:
            failures.append(f"duplicate retirement {key}")
        seen.add(key)
        source = invariants if row["kind"] == "invariant" else evidence if row["kind"] == "evidence" else {}
        if row["id"] not in source:
            failures.append(f"retirement {key} does not retain its source record")
        for evidence_id in row["evidence_refs"]:
            if evidence_id not in evidence:
                failures.append(f"retirement {key} references unknown evidence {evidence_id}")
    return failures


def _suite_cycles(suites):
    failures = []
    def visit(suite_id, stack):
        if suite_id in stack:
            failures.append("suite includes graph contains a cycle: " + " -> ".join((*stack, suite_id)))
            return
        for child in suites.get(suite_id, {}).get("includes", []):
            visit(child, (*stack, suite_id))
    for suite_id in suites:
        visit(suite_id, ())
    return failures


def _ordered_dimensions() -> list[str]:
    return [
        "happy_path", "boundary_low", "boundary_high", "below_min", "above_max",
        "wrong_type_or_shape", "missing_required", "extra_or_unknown",
        "duplicate_or_collision", "ordering_or_lifecycle", "encoding_or_unicode",
        "resource_limit", "auth_or_permission", "persistence_or_recovery",
        "concurrency_or_cancellation", "time_or_clock_fault", "hardware_or_network_fault",
        "supply_chain_or_artifact_fault", "platform_or_filesystem_variation",
    ]


if __name__ == "__main__":
    raise SystemExit(main())
