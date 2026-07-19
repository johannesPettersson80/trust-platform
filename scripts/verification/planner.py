"""Spec-first verification test planner.

Phase 1B scope is intentionally bytecode/VM-only. The planner is a deterministic
read-only join over committed metadata; it never invents expected behavior.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .area_routing import (
    AreaRoutingError,
    classify_changed_path,
    intent_overlay,
    normalize_changed_path,
)
from .metadata_validator.constants import AREAS, HIGH_RISKS, INTENTS, ROOT, VERIFICATION
from .metadata_validator.core import Validator
from .metadata_validator.integrity import OPEN_GAP_RESOLUTIONS, test_counts_as_runnable


EXIT_CLEAR = 0
EXIT_MISSING_TESTS = 2
EXIT_SPEC_GAP = 3
EXIT_UNMAPPED = 4
EXIT_USAGE = 5
EXIT_METADATA_INVALID = 6
BEHAVIOR_INTENTS = {"bugfix", "feature"}
HIGHEST_RISK = "safety_critical"


class MetadataValidationError(RuntimeError):
    pass


@dataclass(frozen=True)
class PlanResult:
    intent: str
    areas: list[str]
    changed_files: list[str]
    unmapped_files: list[str]
    unknown_areas: list[str]
    uninventoried_areas: list[str]
    required_test_classes: list[str]
    required_case_families: list[str]
    matched_route_ids: list[str]
    required_suites: list[str]
    conditional_suites: list[str]
    spec_gaps: list[str]
    missing_test_classes: list[str]
    missing_test_classes_by_area: dict[str, list[str]]
    existing_tests: list[str]
    risk_notes: list[str]
    waiver_notes: list[str]
    risk_changes: list[str]
    baseline: str | None

    @property
    def exit_code(self) -> int:
        if self.unmapped_files:
            return EXIT_UNMAPPED
        if self.unknown_areas or self.uninventoried_areas or self.spec_gaps:
            return EXIT_SPEC_GAP
        if self.missing_test_classes:
            return EXIT_MISSING_TESTS
        return EXIT_CLEAR

    @property
    def verdict(self) -> str:
        return {
            EXIT_CLEAR: "clear",
            EXIT_MISSING_TESTS: "missing_tests",
            EXIT_SPEC_GAP: "spec_gap",
            EXIT_UNMAPPED: "unmapped",
        }[self.exit_code]


def load_toml(path: Path) -> dict[str, Any]:
    return tomllib.loads(path.read_text())


def wrapped(path: Path, key: str) -> list[dict[str, Any]]:
    data = load_toml(path)
    records = data.get(key, [])
    if not isinstance(records, list):
        raise ValueError(f"{path} must use [[{key}]]")
    return records


class Planner:
    def __init__(self) -> None:
        validate_metadata_or_raise()
        self.matrix = load_toml(VERIFICATION / "matrix.toml")
        self.areas = {area["id"]: area for area in self.matrix.get("areas", [])}
        self.intent_requirements = {
            row["intent"]: row for row in self.matrix.get("intent_requirements", [])
        }
        self.required_specs = wrapped(VERIFICATION / "spec-matrix.toml", "required_specs")
        self.spec_gap_records = wrapped(VERIFICATION / "spec-gaps.toml", "spec_gaps")
        self.spec_sources = {
            record["id"]: record
            for record in wrapped(VERIFICATION / "spec-sources.toml", "spec_sources")
        }
        self.tests = wrapped_optional(VERIFICATION / "test-catalog.toml", "tests")

    def classify_file(self, changed_file: str) -> list[str]:
        return list(classify_changed_path(self.matrix, changed_file).area_ids)

    def plan(
        self,
        intent: str,
        changed_files: list[str] | None,
        area: str | None,
        baseline: str | None,
    ) -> PlanResult:
        if intent not in INTENTS:
            raise ValueError(f"unknown intent {intent!r}")

        resolved_areas: set[str] = set()
        unmapped: list[str] = []
        unknown_areas: list[str] = []
        normalized_files: list[str] = []
        route_required_classes: set[str] = set()
        matched_route_ids: set[str] = set()
        required_suites: set[str] = set()
        conditional_suites: set[str] = set()
        invalid_path_notes: set[str] = set()
        if area:
            if area not in AREAS:
                unknown_areas.append(area)
            else:
                resolved_areas.add(area)
                required_suites.update(self.areas.get(area, {}).get("suite_tiers", []))
        for changed_file in changed_files or []:
            try:
                route = classify_changed_path(self.matrix, changed_file)
            except AreaRoutingError as exc:
                normalized_files.append(changed_file)
                unmapped.append(changed_file)
                invalid_path_notes.add(f"invalid changed path {changed_file!r}: {exc}")
                continue
            normalized_files.append(route.path)
            if route.unmapped:
                unmapped.append(route.path)
            resolved_areas.update(route.area_ids)
            matched_route_ids.update(route.route_ids)
            route_required_classes.update(route.required_test_classes)
            required_suites.update(route.suite_tiers)
            conditional_suites.update(route.conditional_suite_tiers)

        overlay = intent_overlay(self.matrix, intent)
        matched_route_ids.update(overlay.route_ids)
        route_required_classes.update(overlay.required_test_classes)
        required_suites.update(overlay.suite_tiers)
        conditional_suites.update(overlay.conditional_suite_tiers)

        if not resolved_areas and not unmapped and not unknown_areas:
            raise ValueError("provide --changed files or --area")

        required_classes: set[str] = (
            set(route_required_classes) if intent in BEHAVIOR_INTENTS else set()
        )
        required_classes.update(overlay.required_test_classes)
        required_families: set[str] = set()
        spec_gaps: set[str] = set()
        existing_tests: set[str] = set()
        missing_classes: set[str] = set()
        missing_classes_by_area: dict[str, list[str]] = {}
        risk_notes: set[str] = set()
        waiver_notes: set[str] = set()
        uninventoried: list[str] = []
        intent_row = self.intent_requirements.get(intent, {})

        if unmapped:
            risk_notes.add(
                f"unmapped files are unclassified and treated as highest risk: {HIGHEST_RISK}"
            )
        risk_notes.update(invalid_path_notes)
        for unknown_area in unknown_areas:
            risk_notes.add(
                f"{unknown_area} is not a canonical area and is treated as highest risk: {HIGHEST_RISK}"
            )

        for area_id in sorted(resolved_areas):
            area_row = self.areas.get(area_id)
            test_mapping_reqs = [
                row for row in self.required_specs
                if row.get("area") == area_id and row.get("blocks") == "test_mapping"
            ]
            if not area_row or not test_mapping_reqs:
                uninventoried.append(area_id)
                risk_notes.add(
                    f"{area_id} is uninventoried and treated as highest risk: {HIGHEST_RISK}"
                )
                continue

            risk_notes.add(area_risk_note(area_id, area_row))
            for requirement in test_mapping_reqs:
                if requirement.get("waived") is True:
                    waiver_notes.add(
                        f"{requirement['id']} waived by {requirement.get('decision_ref', '<missing decision_ref>')}"
                    )

            area_required_classes: set[str] = set(intent_row.get("required_test_classes", []))
            area_required_families: set[str] = set()
            if intent in BEHAVIOR_INTENTS:
                area_required_classes.update(area_row.get("required_test_classes", []))
                area_required_families.update(area_row.get("required_case_families", []))
            required_classes.update(area_required_classes)
            required_families.update(area_required_families)
            for requirement in test_mapping_reqs:
                gap_id = requirement.get("spec_gap_ref")
                if gap_id:
                    spec_gaps.add(gap_id)
            for gap in self.spec_gap_records:
                if gap.get("area") == area_id and gap.get("resolution_status") in OPEN_GAP_RESOLUTIONS:
                    spec_gaps.add(gap["id"])

            runnable_area_tests = [
                test for test in self.tests
                if test.get("area") == area_id and test_counts_as_runnable(test)
            ]
            for test in runnable_area_tests:
                existing_tests.add(test["id"])
            covered_classes = {test.get("test_class") for test in runnable_area_tests}
            area_missing_classes = sorted(area_required_classes - covered_classes)
            missing_classes.update(area_missing_classes)
            if area_missing_classes:
                missing_classes_by_area[area_id] = area_missing_classes

        if spec_gaps:
            missing_classes.clear()
            missing_classes_by_area.clear()

        return PlanResult(
            intent=intent,
            areas=sorted(resolved_areas),
            changed_files=normalized_files,
            unmapped_files=unmapped,
            unknown_areas=sorted(unknown_areas),
            uninventoried_areas=sorted(uninventoried),
            required_test_classes=sorted(required_classes),
            required_case_families=sorted(required_families),
            matched_route_ids=sorted(matched_route_ids),
            required_suites=sorted(required_suites),
            conditional_suites=sorted(conditional_suites - required_suites),
            spec_gaps=sorted(spec_gaps),
            missing_test_classes=sorted(missing_classes),
            missing_test_classes_by_area=missing_classes_by_area,
            existing_tests=sorted(existing_tests),
            risk_notes=sorted(risk_notes),
            waiver_notes=sorted(waiver_notes),
            risk_changes=baseline_risk_changes(
                baseline,
                resolved_areas,
                self.areas,
                self.spec_sources,
            ),
            baseline=baseline,
        )


def wrapped_optional(path: Path, key: str) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    data = load_toml(path)
    records = data.get(key, [])
    if not isinstance(records, list):
        raise ValueError(f"{path} must use [[{key}]]")
    return records


def render_text(result: PlanResult) -> str:
    lines = [
        "# Verification Test Plan",
        "",
        f"Intent: `{result.intent}`",
        f"Verdict: `{result.verdict}`",
        f"Exit code: `{result.exit_code}`",
    ]
    if result.baseline:
        lines.append(f"Baseline: `{result.baseline}`")
    if result.changed_files:
        lines.extend(["", "Changed files:"])
        lines.extend(f"- `{path}`" for path in result.changed_files)
    if result.areas:
        lines.extend(["", "Areas:"])
        lines.extend(f"- `{area}`" for area in result.areas)
    if result.unmapped_files:
        lines.extend(["", "Unmapped files:"])
        lines.extend(f"- `{path}`" for path in result.unmapped_files)
    if result.unknown_areas:
        lines.extend(["", "Unknown areas:"])
        lines.extend(f"- `{area}`" for area in result.unknown_areas)
    if result.uninventoried_areas:
        lines.extend(["", "Uninventoried areas:"])
        lines.extend(f"- `{area}`" for area in result.uninventoried_areas)
    if result.required_test_classes:
        lines.extend(["", "Required test classes:"])
        lines.extend(f"- `{name}`" for name in result.required_test_classes)
    if result.required_case_families:
        lines.extend(["", "Required case families:"])
        lines.extend(f"- `{name}`" for name in result.required_case_families)
    if result.matched_route_ids:
        lines.extend(["", "Matched code-area routes:"])
        lines.extend(f"- `{name}`" for name in result.matched_route_ids)
    if result.required_suites:
        lines.extend(["", "Direct required suites:"])
        lines.extend(f"- `{name}`" for name in result.required_suites)
    if result.conditional_suites:
        lines.extend(["", "Conditional suites (not directly required):"])
        lines.extend(f"- `{name}`" for name in result.conditional_suites)
    if result.spec_gaps:
        lines.extend(["", "Blocking spec gaps:"])
        lines.extend(f"- `{gap}`" for gap in result.spec_gaps)
    if result.missing_test_classes:
        lines.extend(["", "Missing test classes:"])
        lines.extend(f"- `{name}`" for name in result.missing_test_classes)
    if result.missing_test_classes_by_area:
        lines.extend(["", "Missing test classes by area:"])
        for area_id, names in result.missing_test_classes_by_area.items():
            lines.append(f"- `{area_id}`: {', '.join(f'`{name}`' for name in names)}")
    if result.existing_tests:
        lines.extend(["", "Existing mapped tests:"])
        lines.extend(f"- `{test_id}`" for test_id in result.existing_tests)
    if result.risk_notes:
        lines.extend(["", "Risk classification:"])
        lines.extend(f"- {note}" for note in result.risk_notes)
    if result.waiver_notes:
        lines.extend(["", "Waivers:"])
        lines.extend(f"- {note}" for note in result.waiver_notes)
    else:
        lines.extend(["", "Waivers:", "- none"])
    if result.baseline and result.risk_changes:
        lines.extend(["", "Risk changes since baseline:"])
        lines.extend(f"- {note}" for note in result.risk_changes)
    elif result.baseline:
        lines.extend(["", "Risk changes since baseline:", "- none detected"])
    lines.extend([
        "",
        "Planner note: expected behavior is intentionally omitted. If a spec gap",
        "is listed, update the owning spec/decision before writing cases or tests.",
    ])
    return "\n".join(lines) + "\n"


def result_to_json(result: PlanResult) -> str:
    return json.dumps(
        {
            "intent": result.intent,
            "verdict": result.verdict,
            "exit_code": result.exit_code,
            "areas": result.areas,
            "changed_files": result.changed_files,
            "unmapped_files": result.unmapped_files,
            "unknown_areas": result.unknown_areas,
            "uninventoried_areas": result.uninventoried_areas,
            "required_test_classes": result.required_test_classes,
            "required_case_families": result.required_case_families,
            "matched_route_ids": result.matched_route_ids,
            "required_suites": result.required_suites,
            "conditional_suites": result.conditional_suites,
            "spec_gaps": result.spec_gaps,
            "missing_test_classes": result.missing_test_classes,
            "missing_test_classes_by_area": result.missing_test_classes_by_area,
            "existing_tests": result.existing_tests,
            "risk_notes": result.risk_notes,
            "waiver_notes": result.waiver_notes,
            "risk_changes": result.risk_changes,
            "baseline": result.baseline,
        },
        indent=2,
        sort_keys=True,
    ) + "\n"


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Plan required verification tests from committed metadata.")
    parser.add_argument("--intent", required=True, choices=sorted(INTENTS))
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--changed", nargs="+", help="Changed files to classify")
    source.add_argument("--area", help="Canonical area to plan")
    parser.add_argument("--baseline", help="Baseline revision for risk-change reporting")
    parser.add_argument("--format", choices=["text", "json"], default="text")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv or sys.argv[1:])
    except SystemExit as exc:
        return int(exc.code) if exc.code == 0 else EXIT_USAGE
    try:
        result = Planner().plan(args.intent, args.changed, args.area, args.baseline)
    except MetadataValidationError as exc:
        print(str(exc), file=sys.stderr)
        return EXIT_METADATA_INVALID
    except Exception as exc:
        print(f"plan_tests error: {exc}", file=sys.stderr)
        return EXIT_UNMAPPED
    if args.format == "json":
        print(result_to_json(result), end="")
    else:
        print(render_text(result), end="")
    return result.exit_code


def normalize_changed_file(value: str) -> str:
    return normalize_changed_path(value)


def validate_metadata_or_raise() -> None:
    validator = Validator()
    validator.load_records()
    validator.validate()
    if not validator.failures:
        return
    details = "\n".join(
        f"- {failure.path}: {failure.message}" for failure in validator.failures[:20]
    )
    more = "" if len(validator.failures) <= 20 else f"\n- ... {len(validator.failures) - 20} more"
    raise MetadataValidationError(f"plan_tests metadata validation failed:\n{details}{more}")


def area_risk_note(area_id: str, area: dict[str, Any]) -> str:
    risk_default = area.get("risk_default", HIGHEST_RISK)
    high_risks = sorted(area.get("high_risks", []))
    high = ", ".join(high_risks) if high_risks else HIGHEST_RISK
    return f"{area_id}: risk_default={risk_default}; high_risks={high}"


def baseline_risk_changes(
    baseline: str | None,
    area_ids: set[str],
    current_areas: dict[str, dict[str, Any]],
    spec_sources: dict[str, dict[str, Any]],
) -> list[str]:
    if not baseline:
        return []
    result = subprocess.run(
        ["git", "show", f"{baseline}:verification/matrix.toml"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        return [f"baseline matrix unavailable at {baseline}: {result.stderr.strip()}"]
    try:
        baseline_matrix = tomllib.loads(result.stdout)
    except tomllib.TOMLDecodeError as exc:
        return [f"baseline matrix could not be parsed at {baseline}: {exc}"]
    baseline_areas = {area["id"]: area for area in baseline_matrix.get("areas", [])}
    return risk_changes_from_matrices(
        area_ids,
        current_areas,
        baseline_areas,
        spec_sources=spec_sources,
    )


def risk_changes_from_matrices(
    area_ids: set[str],
    current_areas: dict[str, dict[str, Any]],
    baseline_areas: dict[str, dict[str, Any]],
    *,
    spec_sources: dict[str, dict[str, Any]] | None = None,
) -> list[str]:
    changes: list[str] = []
    for area_id in sorted(area_ids):
        current = current_areas.get(area_id)
        previous = baseline_areas.get(area_id)
        if not current or not previous:
            changes.append(f"{area_id}: risk classification added or removed")
            continue
        if current.get("risk_default") != previous.get("risk_default"):
            changes.append(
                f"{area_id}: risk_default {previous.get('risk_default')} -> {current.get('risk_default')}"
            )
        if sorted(current.get("high_risks", [])) != sorted(previous.get("high_risks", [])):
            changes.append(
                f"{area_id}: high_risks {sorted(previous.get('high_risks', []))} -> {sorted(current.get('high_risks', []))}"
            )
        removed_high_risks = set(previous.get("high_risks", [])) - set(
            current.get("high_risks", [])
        )
        default_downgraded = (
            previous.get("risk_default") in HIGH_RISKS
            and current.get("risk_default") not in HIGH_RISKS
        )
        if removed_high_risks or default_downgraded:
            decision_ref = current.get("decision_ref")
            if not decision_ref:
                changes.append(f"{area_id}: risk downgrade requires decision_ref")
            elif not _valid_risk_decision(decision_ref, spec_sources):
                changes.append(
                    f"{area_id}: risk downgrade decision_ref {decision_ref!r} "
                    "is not an active oracle-eligible reviewed decision/deviation"
                )
    return changes


def _valid_risk_decision(
    decision_ref: Any,
    spec_sources: dict[str, dict[str, Any]] | None,
) -> bool:
    if not isinstance(decision_ref, str) or not spec_sources:
        return False
    source = spec_sources.get(decision_ref)
    return bool(
        source
        and source.get("authority") in {"reviewed_decision", "reviewed_deviation"}
        and source.get("source_status") == "active"
        and source.get("oracle_eligible") is True
    )
