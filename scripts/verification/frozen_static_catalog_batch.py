"""Compile reviewed frozen static-test assignments into catalog metadata."""

from __future__ import annotations

import argparse
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
from .frozen_rust_nextest_batch import normalize_nextest_jsonl
from .frozen_hir_family_batch import (
    validate_projection_execution,
    validate_projection_inventories,
)
from .frozen_catalog_identity import (
    digest_bytes as _digest_bytes,
    digest_document as _digest_document,
    selection_digest,
)
from .frozen_invariant_behavior_binding import (
    validate_invariant_behavior_digests,
)
from .frozen_st_batch import normalize_st_json
from .frozen_rust_nextest_target import (
    build_catalog_command as build_rust_catalog_command,
    no_default_feature_test_names,
    resolve_plan_target_profile,
)
from .frozen_static_semantic_assignments import (
    assign_behavior_refs,
    assign_behaviors,
    resolve_behavior_assignments,
)
from .mapping_review_batch import build_review_batch
from .mapping_transaction import apply_mapping_transaction


CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
PROPOSALS_PATH = Path("verification/test-catalog-proposals.toml")
DECISIONS_PATH = Path("verification/test-catalog-mapping-decisions.toml")
CATALOG_MAPPING_DEBT = frozenset(
    {"cataloged_behavior_tests", "catalog_mapping"}
)


def compile_batch(
    *,
    root: Path,
    plan: Mapping[str, Any],
    inventory_document: Mapping[str, Any],
    results_document: Mapping[str, Any],
) -> tuple[dict[str, bytes], tuple[str, ...]]:
    root = root.resolve()
    path = _required_string(plan, "path")
    selected_invariant_ids = _plan_invariant_ids(plan)
    reviewed_date = _required_string(plan, "reviewed_date")
    timestamp = _required_string(plan, "timestamp")
    _required_string(plan, "command")
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
    if plan.get("execution_kind") == "rust_nextest_jsonl_projection":
        validate_projection_inventories(
            root=root,
            facts=facts,
            inventories=inventories,
        )
    assignments = resolve_behavior_assignments(
        inventories=inventories,
        plan=plan,
        default_invariant_id=(
            selected_invariant_ids[0]
            if len(selected_invariant_ids) == 1
            else None
        ),
        allowed_invariant_ids=set(selected_invariant_ids),
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
    if plan.get("execution_kind") in {
        "rust_nextest_jsonl",
        "rust_nextest_jsonl_projection",
    }:
        rust_profile = resolve_plan_target_profile(plan)
        no_default_feature_names = no_default_feature_test_names(
            profile=rust_profile,
            source=(root / path).read_text(),
            test_names=tuple(sorted(durations)),
        )
    else:
        no_default_feature_names = set()

    invariants = _load_all_invariants(root)
    validate_invariant_behavior_digests(
        plan=plan,
        invariant_ids=selected_invariant_ids,
        invariants=invariants,
    )
    invariant_sources: dict[str, tuple[str, str]] = {}
    for invariant_id in selected_invariant_ids:
        invariant_path, invariant = _load_selected_invariant(
            root=root,
            invariant_id=invariant_id,
        )
        if invariants.get(invariant_id) != invariant:
            raise ValueError(
                f"selected invariant {invariant_id} does not match invariant registry"
            )
        invariant_sources[invariant_id] = (
            invariant_path.relative_to(root).as_posix(),
            invariant_path.read_text(),
        )

    trace_records: dict[str, dict[str, Any]] = {}
    candidates: list[dict[str, Any]] = []
    for discovery_id, fact in sorted(facts.items()):
        catalog_command = _catalog_command(
            plan,
            test_name=str(fact["name"]),
            no_default_features=(
                str(fact["name"]) in no_default_feature_names
            ),
        )
        candidate_invariant_ids = sorted(
            {
                invariant_id
                for invariant_id, _behavior_index in assignments[discovery_id]
            }
        )
        trace_records[discovery_id] = {
            "discovery_id": discovery_id,
            "trace_status": "observed",
            "command": catalog_command,
            "command_authority": "exact",
            "duration_ms": durations[str(fact["name"])],
        }
        candidates.append(
            {
                "discovery_id": discovery_id,
                "candidate_tier": "C3",
                "candidate_invariant_ids": candidate_invariant_ids,
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
    review_inputs = []
    for discovery_id, proposal in sorted(proposals_by_discovery.items()):
        assertion_assignments = assignments[discovery_id]
        selected_indices = tuple(dict.fromkeys(assertion_assignments))
        selected_behaviors: list[Mapping[str, Any]] = []
        selected_behavior_refs: list[str] = []
        for invariant_id, behavior_index in selected_indices:
            invariant = invariants[invariant_id]
            behavior_rows = invariant.get("behavior")
            if not isinstance(behavior_rows, Sequence) or isinstance(
                behavior_rows, (str, bytes)
            ):
                raise ValueError(
                    f"selected invariant {invariant_id} lacks behavior rows"
                )
            if behavior_index >= len(behavior_rows):
                raise ValueError(
                    f"{discovery_id} selects absent behavior "
                    f"{invariant_id}#{behavior_index}"
                )
            behavior = behavior_rows[behavior_index]
            if not isinstance(behavior, Mapping):
                raise ValueError("selected behavior row is not a table")
            selected_behaviors.append(behavior)
            selected_behavior_refs.append(
                f"{invariant_id}#behavior:{behavior_index}"
            )
        proposal_invariant_ids = sorted(
            {invariant_id for invariant_id, _ in selected_indices}
        )
        areas = {str(invariants[value]["area"]) for value in proposal_invariant_ids}
        owners = {
            str(invariants[value]["owner"]) for value in proposal_invariant_ids
        }
        if len(areas) != 1 or len(owners) != 1:
            raise ValueError(
                f"{discovery_id} crosses catalog area or owner boundaries"
            )
        catalog_command = _catalog_command(
            plan,
            test_name=str(facts[discovery_id]["name"]),
            no_default_features=(
                str(facts[discovery_id]["name"])
                in no_default_feature_names
            ),
        )
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
                            "product-contract behavior."
                        ),
                    }
                    for assertion, (invariant_id, behavior_index) in zip(
                        proposal["assertions"],
                        assertion_assignments,
                        strict=True,
                    )
                ],
                "selected_invariant_ids": proposal_invariant_ids,
                "selected_behavior_refs": selected_behavior_refs,
                "command": catalog_command,
                "command_authority": "verified_exact",
                "confirmed_catalog": {
                    "test_class": "behavior_lock",
                    "area": next(iter(areas)),
                    "owner": next(iter(owners)),
                    "expected_result": " ".join(
                        str(behavior["rationale"])
                        for behavior in selected_behaviors
                    ),
                    "expected_failure_mode": (
                        "The contract accepts missing, stale, "
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
        invariant_sources=invariant_sources,
        invariants=invariants,
        reviewed_date=reviewed_date,
        source_commit=source_commit,
    )
    if len(batch.accepted_test_ids) != expected_count:
        raise ValueError("review batch did not accept the complete selected set")
    return dict(batch.replacements), batch.accepted_test_ids


def validate_execution_provenance(
    *,
    root: Path,
    plan: Mapping[str, Any],
    results_document: Mapping[str, Any],
) -> None:
    execution = results_document.get("execution")
    if not isinstance(execution, Mapping):
        raise ValueError("focused results lack closed execution provenance")
    if execution.get("kind") == "rust_nextest_jsonl":
        _validate_rust_execution_provenance(
            root=root,
            plan=plan,
            execution=execution,
            results_document=results_document,
        )
        return
    if execution.get("kind") == "rust_nextest_jsonl_projection":
        validate_projection_execution(
            root=root,
            plan=plan,
            execution=execution,
            results_document=results_document,
        )
        return
    if execution.get("kind") == "st_json":
        _validate_st_execution_provenance(
            root=root,
            plan=plan,
            execution=execution,
            results_document=results_document,
        )
        return
    if set(execution) != {
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


def _validate_st_execution_provenance(
    *,
    root: Path,
    plan: Mapping[str, Any],
    execution: Mapping[str, Any],
    results_document: Mapping[str, Any],
) -> None:
    if plan.get("execution_kind") != "st_json":
        raise ValueError("ST focused results do not match plan execution_kind")
    expected_fields = {
        "kind",
        "command",
        "runner_path",
        "runner_sha256",
        "shim_path",
        "shim_sha256",
        "source_path",
        "source_sha256",
        "raw_results_path",
        "raw_results_sha256",
        "git_head",
        "project_path",
        "test_prefix",
    }
    if set(execution) != expected_fields:
        raise ValueError("ST focused results lack closed execution provenance")
    expected = {
        "command": _required_string(plan, "command"),
        "runner_path": _required_string(plan, "runner_path"),
        "shim_path": _required_string(plan, "shim_path"),
        "source_path": _required_string(plan, "path"),
        "raw_results_path": _required_string(plan, "raw_results_path"),
        "git_head": _required_string(plan, "git_head"),
        "project_path": _required_string(plan, "project_path"),
        "test_prefix": _required_string(plan, "test_prefix"),
    }
    for field, value in expected.items():
        if execution.get(field) != value:
            raise ValueError(f"focused result {field} does not match plan")
    if execution["git_head"] != _git_head(root):
        raise ValueError("focused result git_head does not match live Git HEAD")
    for path_field, digest_field in (
        ("runner_path", "runner_sha256"),
        ("shim_path", "shim_sha256"),
        ("source_path", "source_sha256"),
        ("raw_results_path", "raw_results_sha256"),
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
    tests = results_document.get("tests")
    if not isinstance(tests, Sequence) or isinstance(tests, (str, bytes)):
        raise ValueError("ST focused results lack normalized tests")
    titles = [
        row.get("title")
        for row in tests
        if isinstance(row, Mapping)
    ]
    if len(titles) != len(tests) or any(
        not isinstance(title, str) or not title for title in titles
    ):
        raise ValueError("ST focused results contain invalid titles")
    normalized = normalize_st_json(
        (root / str(execution["raw_results_path"])).read_text(),
        expected_names=set(titles),
        expected_project=str(execution["project_path"]),
    )
    normalized_document = {
        field: results_document.get(field)
        for field in ("tests", "failures", "pending", "stats")
    }
    if normalized != normalized_document:
        raise ValueError("ST normalized results do not match raw JSON evidence")


def _validate_rust_execution_provenance(
    *,
    root: Path,
    plan: Mapping[str, Any],
    execution: Mapping[str, Any],
    results_document: Mapping[str, Any],
) -> None:
    if plan.get("execution_kind") != "rust_nextest_jsonl":
        raise ValueError("Rust focused results do not match plan execution_kind")
    profile = resolve_plan_target_profile(plan)
    expected_fields = {
        "kind",
        "batch_command",
        "runner_path",
        "runner_sha256",
        "source_path",
        "source_sha256",
        "raw_results_path",
        "raw_results_sha256",
        "nextest_version",
        "package",
        "target_kind",
        "binary_id",
        "test_prefix",
    }
    if set(execution) != expected_fields:
        raise ValueError("Rust focused results lack closed execution provenance")
    expected = {
        "batch_command": _required_string(plan, "command"),
        "runner_path": _required_string(plan, "runner_path"),
        "source_path": _required_string(plan, "path"),
        "raw_results_path": _required_string(plan, "raw_results_path"),
        "package": _required_string(plan, "package"),
        "target_kind": _required_string(plan, "target_kind"),
        "binary_id": _required_string(plan, "binary_id"),
        "test_prefix": _required_test_prefix(plan),
    }
    for field, value in expected.items():
        if execution.get(field) != value:
            raise ValueError(f"focused result {field} does not match plan")
    if (
        not isinstance(execution.get("nextest_version"), str)
        or not execution["nextest_version"]
    ):
        raise ValueError("focused result nextest_version is empty")
    for path_field, digest_field in (
        ("runner_path", "runner_sha256"),
        ("source_path", "source_sha256"),
        ("raw_results_path", "raw_results_sha256"),
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
    tests = results_document.get("tests")
    if not isinstance(tests, Sequence) or isinstance(tests, (str, bytes)):
        raise ValueError("Rust focused results lack normalized tests")
    titles = [
        row.get("title")
        for row in tests
        if isinstance(row, Mapping)
    ]
    if len(titles) != len(tests) or any(
        not isinstance(title, str) or not title for title in titles
    ):
        raise ValueError("Rust focused results contain invalid titles")
    expected_full_names = {
        str(execution["test_prefix"]) + str(title) for title in titles
    }
    raw_path = root / str(execution["raw_results_path"])
    normalized = normalize_nextest_jsonl(
        raw_path.read_text(),
        expected_full_names=expected_full_names,
        profile=profile,
    )
    if normalized != list(tests):
        raise ValueError(
            "Rust normalized results do not match raw nextest evidence"
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
    strict_results = (
        isinstance(results_document.get("execution"), Mapping)
        and results_document["execution"].get("kind")
        in {
            "rust_nextest_jsonl",
            "rust_nextest_jsonl_projection",
            "st_json",
        }
    )
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
        if strict_results and row.get("state") != "passed":
            raise ValueError("focused result state is not passed")
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
    if strict_results:
        expected_total = len(allowed_names)
        if results_document.get("stats") != {
            "tests": expected_total,
            "passes": expected_total,
            "failures": 0,
            "pending": 0,
        }:
            raise ValueError("focused result stats do not match passing tests")
    return {name: durations[name] for name in expected_names}


def validate_applied_batch(
    *,
    root: Path,
    selected_ids: set[str],
    invariant_ids: set[str],
    invariant_test_baselines: Mapping[str, set[str]],
    invariant_missing_baselines: Mapping[str, set[str]],
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
    invariants = _load_all_invariants(root)
    if (
        set(invariant_test_baselines) != invariant_ids
        or set(invariant_missing_baselines) != invariant_ids
    ):
        failures.append("invariant baselines do not match selected invariants")
        return failures
    if any(invariant_id not in invariants for invariant_id in invariant_ids):
        failures.append("a selected invariant is missing after acceptance")
        return failures
    mapped_by_invariant: dict[str, set[str]] = {
        invariant_id: set() for invariant_id in invariant_ids
    }
    for row in mapped_rows:
        test_id = str(row.get("id"))
        row_invariants = {
            str(value)
            for value in row.get("invariants", [])
            if isinstance(value, str)
        }
        selected_row_invariants = row_invariants & invariant_ids
        if not selected_row_invariants:
            failures.append(f"{test_id} lacks a selected invariant link")
            continue
        for invariant_id in selected_row_invariants:
            mapped_by_invariant[invariant_id].add(test_id)
            if not any(
                isinstance(ref, str)
                and ref.startswith(f"{invariant_id}#behavior:")
                for ref in row.get("mapping_behavior_refs", [])
            ):
                failures.append(
                    f"{test_id} lacks a behavior link for {invariant_id}"
                )
    for invariant_id in sorted(invariant_ids):
        expected = (
            set(invariant_test_baselines[invariant_id])
            | mapped_by_invariant[invariant_id]
        )
        actual = {
            str(test_id) for test_id in invariants[invariant_id].get("tests", [])
        }
        if not mapped_by_invariant[invariant_id]:
            failures.append(f"{invariant_id} accepted no selected tests")
        if actual != expected:
            failures.append(
                f"{invariant_id} test links do not equal its accepted mapping"
            )
        remaining_debt = (
            set(invariants[invariant_id].get("missing", []))
            & CATALOG_MAPPING_DEBT
        )
        baseline_debt = (
            set(invariant_missing_baselines[invariant_id])
            & CATALOG_MAPPING_DEBT
        )
        if remaining_debt and not baseline_debt:
            failures.append(
                f"{invariant_id} introduced catalog mapping debt"
            )
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


def _required_test_prefix(record: Mapping[str, Any]) -> str:
    value = record.get("test_prefix")
    if not isinstance(value, str):
        raise ValueError("test_prefix must be a string")
    return value


def _string_array(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"plan {label} must be an array")
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value) or len(result) != len(set(result)) or not result:
        raise ValueError(f"plan {label} must contain distinct non-empty strings")
    return sorted(result)


def _plan_invariant_ids(plan: Mapping[str, Any]) -> tuple[str, ...]:
    plural = plan.get("invariant_ids")
    singular = plan.get("invariant_id")
    if plural is not None and singular is not None:
        raise ValueError("plan must use invariant_id or invariant_ids, not both")
    if plural is not None:
        return tuple(_string_array(plural, "invariant_ids"))
    return (_required_string(plan, "invariant_id"),)


def _catalog_command(
    plan: Mapping[str, Any],
    *,
    test_name: str,
    no_default_features: bool = False,
) -> str:
    if plan.get("execution_kind") in {
        "rust_nextest_jsonl",
        "rust_nextest_jsonl_projection",
    }:
        profile = resolve_plan_target_profile(plan)
        if plan.get("catalog_command_template") is not None:
            raise ValueError(
                "Rust plan catalog commands are generated canonically"
            )
        return build_rust_catalog_command(
            profile,
            test_name,
            no_default_features=no_default_features,
        )
    template = plan.get("catalog_command_template")
    if template is None:
        return _required_string(plan, "command")
    if not isinstance(template, str) or not template:
        raise ValueError("plan catalog_command_template must be non-empty")
    if template.count("{name}") != 1:
        raise ValueError(
            "plan catalog_command_template must contain {name} exactly once"
        )
    command = template.replace("{name}", test_name)
    if "{" in command or "}" in command:
        raise ValueError("plan catalog_command_template has unsupported fields")
    return command


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
        invariant_ids = set(_plan_invariant_ids(plan))
        if args.apply:
            live_invariants = _load_all_invariants(root)
            invariant_test_baselines = {
                invariant_id: {
                    str(test_id)
                    for test_id in live_invariants[invariant_id].get("tests", [])
                }
                for invariant_id in invariant_ids
            }
            invariant_missing_baselines = {
                invariant_id: {
                    str(obligation)
                    for obligation in live_invariants[invariant_id].get(
                        "missing", []
                    )
                }
                for invariant_id in invariant_ids
            }
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
                    invariant_ids=invariant_ids,
                    invariant_test_baselines=invariant_test_baselines,
                    invariant_missing_baselines=invariant_missing_baselines,
                ),
            )
        verb = "applied" if args.apply else "ready"
        print(
            f"frozen static catalog batch {verb}: "
            f"{len(accepted_ids)} tests -> "
            f"{','.join(sorted(invariant_ids))}"
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
