"""Compile reviewed frozen static-test assignments into catalog metadata."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .mapping_proposals import (
    build_mapping_proposals,
    duration_class,
    validate_mapping_quarantine,
)
from .mapping_review_batch import build_review_batch
from .mapping_transaction import apply_mapping_transaction


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
PROPOSALS_PATH = Path("verification/test-catalog-proposals.toml")
DECISIONS_PATH = Path("verification/test-catalog-mapping-decisions.toml")


def compile_batch(
    *,
    root: Path,
    plan: Mapping[str, Any],
    inventory_document: Mapping[str, Any],
    results_document: Mapping[str, Any],
) -> tuple[dict[str, bytes], tuple[str, ...]]:
    root = root.resolve()
    path = _required_string(plan, "path")
    invariant_id = _required_string(plan, "invariant_id")
    reviewed_date = _required_string(plan, "reviewed_date")
    timestamp = _required_string(plan, "timestamp")
    command = _required_string(plan, "command")
    reviewers = _string_array(plan.get("reviewer_ids"), "reviewer_ids")
    expected_count = plan.get("expected_count")
    if not isinstance(expected_count, int) or isinstance(expected_count, bool):
        raise ValueError("plan expected_count must be an integer")
    expected_selection_digest = _required_string(plan, "selection_digest")

    denominator = _toml(root / DENOMINATOR_PATH)
    catalog_source = (root / CATALOG_PATH).read_text()
    catalog = tomllib.loads(catalog_source)
    proposal_document = _toml(root / PROPOSALS_PATH)
    decision_document = _toml(root / DECISIONS_PATH)
    if proposal_document.get("proposals") != []:
        raise ValueError("frozen static batch requires an empty proposal queue")

    reviews = [
        row
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("path") == path
        and row.get("disposition") == "reviewed_nonmapping"
    ]
    if not reviews:
        raise ValueError("frozen denominator selected no reviewed_nonmapping rows")
    if len(reviews) != expected_count:
        raise ValueError(
            f"frozen denominator selected {len(reviews)} rows, expected "
            f"{expected_count}"
        )
    actual_selection_digest = selection_digest(reviews)
    if actual_selection_digest != expected_selection_digest:
        raise ValueError(
            "frozen denominator selection digest drifted: expected "
            f"{expected_selection_digest}, actual {actual_selection_digest}"
        )
    facts = {
        str(row["discovery_id"]): {
            "discovery_id": row["discovery_id"],
            "discovery_source_kind": row["discovery_source_kind"],
            "path": row["path"],
            "name": row["name"],
            "ignore_state": row["ignore_state"],
        }
        for row in reviews
    }

    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("assertion inventory document lacks inventories")
    if set(inventories) != set(facts):
        raise ValueError("assertion inventory IDs do not equal the selected denominator")
    assignments = assign_behaviors(
        inventories=inventories,
        ranges=plan.get("ranges"),
    )
    validate_execution_provenance(
        root=root,
        plan=plan,
        results_document=results_document,
    )
    durations = measured_durations(
        results_document=results_document,
        expected_names={str(row["name"]) for row in reviews},
        allowed_extra_names={
            str(row["name"])
            for row in catalog.get("tests", [])
            if isinstance(row, Mapping)
            and row.get("path") == path
            and isinstance(row.get("name"), str)
        },
    )

    invariant_path, invariant = _load_selected_invariant(
        root=root,
        invariant_id=invariant_id,
    )
    invariants = _load_all_invariants(root)
    if invariants.get(invariant_id) != invariant:
        raise ValueError("selected invariant does not match invariant registry")

    trace_records: dict[str, dict[str, Any]] = {}
    candidates: list[dict[str, Any]] = []
    for discovery_id, fact in sorted(facts.items()):
        trace_records[discovery_id] = {
            "discovery_id": discovery_id,
            "trace_status": "observed",
            "command": command,
            "command_authority": "exact",
            "duration_ms": durations[str(fact["name"])],
        }
        candidates.append(
            {
                "discovery_id": discovery_id,
                "candidate_tier": "C3",
                "candidate_invariant_ids": [invariant_id],
                "covered_code_ids": [],
                "specified_code_ids": [],
                "spec_gap_code_ids": [],
                "spec_gap_refs": [],
                "mutation_kill_ids": [],
            }
        )

    proposals = build_mapping_proposals(
        root=root,
        candidates=candidates,
        trace_records=trace_records,
        facts=facts,
        invariants=invariants,
        assertion_inventories={
            str(key): value
            for key, value in inventories.items()
            if isinstance(value, Mapping)
        },
    )
    failures = validate_mapping_quarantine(
        root=root,
        proposals=proposals,
        facts=facts,
        tests={
            str(row["id"]): row
            for row in catalog.get("tests", [])
            if isinstance(row, Mapping) and isinstance(row.get("id"), str)
        },
        denominator_reviews={
            str(row["discovery_id"]): row for row in reviews
        },
        invariants=invariants,
    )
    if failures:
        raise ValueError("; ".join(failures))

    source_commit = _git_head(root)
    proposals_by_discovery = {
        str(proposal["discovery_id"]): proposal for proposal in proposals
    }
    proposal_document = {
        **proposal_document,
        "source_commit": source_commit,
        "trace_digest": _digest_document(results_document),
        "candidate_report_digest": _digest_document(plan),
        "last_generated": timestamp,
        "proposals": proposals,
    }
    behavior_rows = invariant.get("behavior")
    if not isinstance(behavior_rows, Sequence) or isinstance(
        behavior_rows, (str, bytes)
    ):
        raise ValueError("selected invariant lacks behavior rows")
    review_inputs = []
    for discovery_id, proposal in sorted(proposals_by_discovery.items()):
        assertion_assignments = assignments[discovery_id]
        selected_indices = tuple(dict.fromkeys(assertion_assignments))
        selected_behaviors: list[Mapping[str, Any]] = []
        for behavior_index in selected_indices:
            if behavior_index >= len(behavior_rows):
                raise ValueError(
                    f"{discovery_id} selects absent behavior {behavior_index}"
                )
            behavior = behavior_rows[behavior_index]
            if not isinstance(behavior, Mapping):
                raise ValueError("selected behavior row is not a table")
            selected_behaviors.append(behavior)
        selected_behavior_refs = [
            f"{invariant_id}#behavior:{behavior_index}"
            for behavior_index in selected_indices
        ]
        review_inputs.append(
            {
                "proposal_id": proposal["id"],
                "outcome": "accepted",
                "reviewer_ids": reviewers,
                "assertion_reviews": [
                    {
                        "assertion_id": assertion["assertion_id"],
                        "disposition": "oracle_bound",
                        "behavior_refs": [
                            f"{invariant_id}#behavior:{behavior_index}"
                        ],
                        "rationale": (
                            "The reviewed assertion locks this selected "
                            "VS Code product-contract behavior."
                        ),
                    }
                    for assertion, behavior_index in zip(
                        proposal["assertions"],
                        assertion_assignments,
                        strict=True,
                    )
                ],
                "selected_invariant_ids": [invariant_id],
                "selected_behavior_refs": selected_behavior_refs,
                "command": command,
                "command_authority": "verified_exact",
                "confirmed_catalog": {
                    "test_class": "behavior_lock",
                    "area": invariant["area"],
                    "owner": invariant["owner"],
                    "expected_result": " ".join(
                        str(behavior["rationale"])
                        for behavior in selected_behaviors
                    ),
                    "expected_failure_mode": (
                        "The static contract accepts missing, stale, "
                        "contradictory, or falsely successful product wiring."
                    ),
                    "suite_tiers": ["pr"],
                    "requires_hardware": False,
                    "requires_network": False,
                    "duration_class": duration_class(
                        int(proposal["duration_ms"])
                    ),
                },
            }
        )

    batch = build_review_batch(
        proposal_document=proposal_document,
        decision_document=decision_document,
        review_inputs=review_inputs,
        catalog_source=catalog_source,
        denominator_document=denominator,
        invariant_sources={
            invariant_id: (
                invariant_path.relative_to(root).as_posix(),
                invariant_path.read_text(),
            )
        },
        invariants=invariants,
        reviewed_date=reviewed_date,
        source_commit=source_commit,
    )
    if len(batch.accepted_test_ids) != expected_count:
        raise ValueError("review batch did not accept the complete selected set")
    return dict(batch.replacements), batch.accepted_test_ids


def assign_behaviors(
    *,
    inventories: Mapping[str, Any],
    ranges: Any,
) -> dict[str, tuple[int, ...]]:
    if not isinstance(ranges, Sequence) or isinstance(ranges, (str, bytes)):
        raise ValueError("plan ranges must be an array")
    normalized: list[tuple[int, int, int]] = []
    for index, row in enumerate(ranges):
        if not isinstance(row, Mapping) or set(row) != {
            "behavior_index",
            "min_line",
            "max_line",
        }:
            raise ValueError(f"plan range {index} fields drift")
        values = (
            row["behavior_index"],
            row["min_line"],
            row["max_line"],
        )
        if any(
            not isinstance(value, int) or isinstance(value, bool)
            for value in values
        ):
            raise ValueError(f"plan range {index} values must be integers")
        behavior_index, minimum, maximum = values
        if behavior_index < 0 or minimum < 1 or maximum < minimum:
            raise ValueError(f"plan range {index} is invalid")
        normalized.append((behavior_index, minimum, maximum))

    assignments: dict[str, tuple[int, ...]] = {}
    for discovery_id, inventory in inventories.items():
        if not isinstance(inventory, Mapping):
            raise ValueError(f"{discovery_id} assertion inventory is not an object")
        assertions = inventory.get("assertions")
        if (
            not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
            or not assertions
        ):
            raise ValueError(f"{discovery_id} lacks reviewed assertions")
        lines = [
            assertion.get("line")
            for assertion in assertions
            if isinstance(assertion, Mapping)
        ]
        if len(lines) != len(assertions) or any(
            not isinstance(line, int) or isinstance(line, bool) for line in lines
        ):
            raise ValueError(f"{discovery_id} assertion lines are invalid")
        matched_by_line = [
            {
                behavior_index
                for behavior_index, minimum, maximum in normalized
                if minimum <= line <= maximum
            }
            for line in lines
        ]
        if any(len(matches) != 1 for matches in matched_by_line):
            bad_line, matches = next(
                (line, matches)
                for line, matches in zip(lines, matched_by_line, strict=True)
                if len(matches) != 1
            )
            raise ValueError(
                f"{discovery_id} line {bad_line} has {len(matches)} "
                "behavior assignments"
            )
        assignments[str(discovery_id)] = tuple(
            next(iter(matches)) for matches in matched_by_line
        )
    return assignments


def validate_execution_provenance(
    *,
    root: Path,
    plan: Mapping[str, Any],
    results_document: Mapping[str, Any],
) -> None:
    execution = results_document.get("execution")
    if not isinstance(execution, Mapping) or set(execution) != {
        "command",
        "runner_path",
        "runner_sha256",
        "source_path",
        "source_sha256",
        "compiled_test_path",
        "compiled_test_sha256",
    }:
        raise ValueError("focused results lack closed execution provenance")
    expected = {
        "command": _required_string(plan, "command"),
        "runner_path": _required_string(plan, "runner_path"),
        "source_path": _required_string(plan, "path"),
        "compiled_test_path": _required_string(plan, "compiled_test_path"),
    }
    for field, value in expected.items():
        if execution.get(field) != value:
            raise ValueError(f"focused result {field} does not match plan")
    for path_field, digest_field in (
        ("runner_path", "runner_sha256"),
        ("source_path", "source_sha256"),
        ("compiled_test_path", "compiled_test_sha256"),
    ):
        path = root / str(execution[path_field])
        if (
            not path.is_file()
            or path.is_symlink()
            or execution.get(digest_field) != _digest_bytes(path.read_bytes())
        ):
            raise ValueError(
                f"focused result {digest_field} does not match live input"
            )


def measured_durations(
    *,
    results_document: Mapping[str, Any],
    expected_names: set[str],
    allowed_extra_names: set[str] | None = None,
) -> dict[str, int]:
    tests = results_document.get("tests")
    failures = results_document.get("failures")
    pending = results_document.get("pending")
    if not isinstance(tests, Sequence) or isinstance(tests, (str, bytes)):
        raise ValueError("focused results lack tests")
    if failures != [] or pending != []:
        raise ValueError("focused results contain failures or pending tests")
    durations: dict[str, int] = {}
    for row in tests:
        if not isinstance(row, Mapping):
            raise ValueError("focused result test is not an object")
        name = row.get("title")
        duration = row.get("duration")
        if (
            not isinstance(name, str)
            or not name
            or not isinstance(duration, int)
            or isinstance(duration, bool)
            or duration < 0
            or name in durations
        ):
            raise ValueError("focused result title or duration is invalid")
        durations[name] = duration
    allowed_names = expected_names | (allowed_extra_names or set())
    if set(durations) != allowed_names:
        raise ValueError("focused result titles do not equal denominator names")
    return {name: durations[name] for name in expected_names}


def validate_applied_batch(
    *,
    root: Path,
    selected_ids: set[str],
    invariant_id: str,
) -> list[str]:
    failures: list[str] = []
    catalog = _toml(root / CATALOG_PATH)
    denominator = _toml(root / DENOMINATOR_PATH)
    proposals = _toml(root / PROPOSALS_PATH)
    mapped_rows = [
        row
        for row in catalog.get("tests", [])
        if isinstance(row, Mapping) and row.get("discovery_id") in selected_ids
    ]
    if {str(row.get("discovery_id")) for row in mapped_rows} != selected_ids:
        failures.append("catalog does not contain the exact selected IDs")
    reviews = {
        str(row.get("discovery_id")): row
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping) and row.get("discovery_id") in selected_ids
    }
    if set(reviews) != selected_ids or any(
        row.get("disposition") != "catalog_mapped" for row in reviews.values()
    ):
        failures.append("denominator did not map the exact selected IDs")
    if proposals.get("proposals") != []:
        failures.append("accepted proposal queue is not empty")
    invariant = _load_all_invariants(root).get(invariant_id)
    expected_tests = {str(row.get("id")) for row in mapped_rows}
    if invariant is None or set(invariant.get("tests", [])) != expected_tests:
        failures.append("invariant test links do not equal accepted catalog rows")
    elif any(
        cell.get("state") != "covered"
        for cell in invariant.get("coverage", {}).get("cells", [])
        if isinstance(cell, Mapping)
    ):
        failures.append("invariant catalog-awaiting coverage did not close")
    elif "cataloged_behavior_tests" in invariant.get("missing", []):
        failures.append("invariant catalog debt remained after complete mapping")
    return failures


def _load_selected_invariant(
    *,
    root: Path,
    invariant_id: str,
) -> tuple[Path, dict[str, Any]]:
    matches = []
    for path in (root / "verification/invariants").rglob("*.toml"):
        document = _toml(path)
        if document.get("id") == invariant_id:
            matches.append((path, document))
    if len(matches) != 1:
        raise ValueError(
            f"selected invariant {invariant_id} has {len(matches)} sources"
        )
    return matches[0]


def _load_all_invariants(root: Path) -> dict[str, dict[str, Any]]:
    invariants: dict[str, dict[str, Any]] = {}
    for path in (root / "verification/invariants").rglob("*.toml"):
        document = _toml(path)
        invariant_id = document.get("id")
        if not isinstance(invariant_id, str) or not invariant_id:
            raise ValueError(f"invariant source lacks ID: {path}")
        if invariant_id in invariants:
            raise ValueError(f"duplicate invariant ID {invariant_id}")
        invariants[invariant_id] = document
    return invariants


def _toml(path: Path) -> dict[str, Any]:
    document = tomllib.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a TOML document")
    return document


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str = "plan",
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _string_array(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"plan {label} must be an array")
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value) or len(result) != len(set(result)) or not result:
        raise ValueError(f"plan {label} must contain distinct non-empty strings")
    return sorted(result)


def _digest_document(document: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        document,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def selection_digest(reviews: Sequence[Mapping[str, Any]]) -> str:
    """Bind a reviewed batch to its exact frozen denominator identities."""

    rows = [
        {
            "discovery_id": _required_string(
                row, "discovery_id", "denominator review"
            ),
            "discovery_source_kind": _required_string(
                row, "discovery_source_kind", "denominator review"
            ),
            "path": _required_string(row, "path", "denominator review"),
            "name": _required_string(row, "name", "denominator review"),
            "ignore_state": _required_string(
                row, "ignore_state", "denominator review"
            ),
        }
        for row in reviews
    ]
    rows.sort(key=lambda row: row["discovery_id"])
    return _digest_document({"reviews": rows})


def _digest_bytes(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _git_head(root: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if len(result) != 40:
        raise ValueError("repository lacks a full Git HEAD")
    return result


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--inventory", type=Path, required=True)
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument(
        "--freeze-selection",
        action="store_true",
        help="derive and record the exact denominator count and digest",
    )
    parser.add_argument("--apply", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        plan_path = args.plan if args.plan.is_absolute() else root / args.plan
        inventory_path = (
            args.inventory if args.inventory.is_absolute() else root / args.inventory
        )
        results_path = (
            args.results if args.results.is_absolute() else root / args.results
        )
        plan = json.loads(plan_path.read_text())
        inventory = json.loads(inventory_path.read_text())
        results = json.loads(results_path.read_text())
        if not all(
            isinstance(document, Mapping)
            for document in (plan, inventory, results)
        ):
            raise ValueError("plan, inventory, and results must be JSON objects")
        if args.freeze_selection and args.apply:
            raise ValueError("--freeze-selection and --apply are separate steps")
        if args.freeze_selection:
            denominator = _toml(root / DENOMINATOR_PATH)
            selected_reviews = [
                row
                for row in denominator.get("reviews", [])
                if isinstance(row, Mapping)
                and row.get("path") == plan.get("path")
                and row.get("disposition") == "reviewed_nonmapping"
            ]
            if not selected_reviews:
                raise ValueError(
                    "cannot freeze an empty reviewed_nonmapping selection"
                )
            plan = {
                **plan,
                "expected_count": len(selected_reviews),
                "selection_digest": selection_digest(selected_reviews),
            }
            plan_path.write_text(
                json.dumps(plan, indent=2, ensure_ascii=False) + "\n"
            )
            print(
                "frozen denominator selection recorded: "
                f"{len(selected_reviews)} tests"
            )
        replacements, accepted_ids = compile_batch(
            root=root,
            plan=plan,
            inventory_document=inventory,
            results_document=results,
        )
        invariant_id = _required_string(plan, "invariant_id")
        if args.apply:
            originals = {
                path: (root / path).read_bytes()
                for path in replacements
            }
            selected_ids = {
                str(row["discovery_id"])
                for row in _toml(root / DENOMINATOR_PATH).get("reviews", [])
                if isinstance(row, Mapping)
                and row.get("path") == plan.get("path")
                and row.get("disposition") == "reviewed_nonmapping"
            }
            apply_mapping_transaction(
                root=root,
                replacements=replacements,
                expected_originals=originals,
                validate=lambda: validate_applied_batch(
                    root=root,
                    selected_ids=selected_ids,
                    invariant_id=invariant_id,
                ),
            )
        verb = "applied" if args.apply else "ready"
        print(
            f"frozen static catalog batch {verb}: "
            f"{len(accepted_ids)} tests -> {invariant_id}"
        )
        return 0
    except (
        OSError,
        ValueError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"frozen static catalog batch failed: {error}")
        return 2
