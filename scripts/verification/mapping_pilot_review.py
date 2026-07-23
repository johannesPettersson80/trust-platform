"""Generated semantic-review records for the hidden-ground-truth pilot."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .mapping_assertions import invoke_rust_assertion_inventory
from .metadata_validator.constants import HIGH_RISKS
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_scanner import scan_repository
from .code_spec_live import load_code_spec_metadata


def build_pilot_review(
    *,
    candidate: Mapping[str, Any],
    fact: Mapping[str, Any],
    assertion_inventory: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
    invariant_index: int,
    oracle_assertion_bindings: Mapping[int, Sequence[int]],
    setup_assertion_indices: Sequence[int],
    non_oracle_assertion_indices: Sequence[int],
    reviewer_ids: Sequence[str],
) -> dict[str, Any]:
    """Turn human semantic choices by displayed index into generated identities."""

    candidate_invariants = _strings(
        candidate.get("candidate_invariant_ids"), "candidate invariants"
    )
    invariant_id = _indexed(candidate_invariants, invariant_index, "invariant")
    invariant = invariants.get(invariant_id)
    if invariant is None:
        raise ValueError(f"selected invariant does not resolve: {invariant_id}")
    behaviors = invariant.get("behavior")
    if (
        not isinstance(behaviors, Sequence)
        or isinstance(behaviors, (str, bytes))
        or any(not isinstance(item, Mapping) for item in behaviors)
    ):
        raise ValueError(f"{invariant_id} behavior rows are invalid")
    assertions = assertion_inventory.get("assertions")
    if (
        not isinstance(assertions, Sequence)
        or isinstance(assertions, (str, bytes))
        or any(not isinstance(item, Mapping) for item in assertions)
    ):
        raise ValueError("pilot assertion inventory is invalid")
    unresolved = _strings(
        assertion_inventory.get("unresolved_assertion_helpers"),
        "unresolved assertion helpers",
    )
    if unresolved:
        raise ValueError(
            "pilot acceptance is blocked by unresolved assertion helpers: "
            f"{unresolved}"
        )
    oracle_bindings = _validated_oracle_bindings(
        oracle_assertion_bindings,
        assertion_count=len(assertions),
        behavior_count=len(behaviors),
        invariant_id=invariant_id,
    )
    oracle = sorted(oracle_bindings)
    setup = _validated_indices(
        setup_assertion_indices, len(assertions), "setup assertion"
    )
    non_oracle = _validated_indices(
        non_oracle_assertion_indices, len(assertions), "non-oracle assertion"
    )
    if not oracle:
        raise ValueError("pilot acceptance requires an oracle-bound assertion")
    selected_indices = [*oracle, *setup, *non_oracle]
    if sorted(selected_indices) != list(range(len(assertions))):
        raise ValueError(
            "pilot review must classify every displayed assertion exactly once"
        )
    reviewers = sorted(set(_strings(reviewer_ids, "reviewer IDs")))
    if not reviewers:
        raise ValueError("pilot review requires a named reviewer")
    if (
        invariant.get("risk") in HIGH_RISKS
        and len(reviewers) < 2
    ):
        raise ValueError("high-risk pilot acceptance requires two reviewers")
    assertion_reviews = []
    for index, assertion in enumerate(assertions):
        if index in oracle:
            disposition = "oracle_bound"
            refs = oracle_bindings[index]
            rationale = "Reviewer bound this generated assertion to the selected behavior."
        elif index in setup:
            disposition = "setup_or_precondition"
            refs = []
            rationale = "Reviewer classified this generated assertion as setup or precondition."
        else:
            disposition = "non_oracle"
            refs = []
            rationale = "Reviewer classified this generated assertion as non-oracle."
        assertion_reviews.append(
            {
                "assertion_id": str(assertion["assertion_id"]),
                "disposition": disposition,
                "behavior_refs": list(refs),
                "rationale": rationale,
            }
        )
    assertion_reviews.sort(key=lambda row: row["assertion_id"])
    selected_behavior_refs = sorted(
        {
            behavior_ref
            for refs in oracle_bindings.values()
            for behavior_ref in refs
        }
    )
    return {
        "discovery_id": _required_string(candidate, "discovery_id", "candidate"),
        "candidate_signal_digest": _digest(candidate),
        "test_source_digest": _required_string(
            assertion_inventory, "source_digest", "assertion inventory"
        ),
        "assertion_inventory_digest": _required_string(
            assertion_inventory, "inventory_digest", "assertion inventory"
        ),
        "reviewer_ids": reviewers,
        "selected_invariant_ids": [invariant_id],
        "selected_behavior_refs": selected_behavior_refs,
        "assertion_reviews": assertion_reviews,
    }


def generate_pilot_review(
    *,
    root: Path,
    candidates_path: Path,
    output_path: Path,
    candidate_index: int,
    invariant_index: int,
    oracle_assertion_bindings: Mapping[int, Sequence[int]],
    setup_assertion_indices: Sequence[int],
    non_oracle_assertion_indices: Sequence[int],
    reviewer_ids: Sequence[str],
) -> dict[str, Any]:
    candidate_document = json.loads(candidates_path.read_text())
    candidates = candidate_document.get("candidates")
    if (
        not isinstance(candidates, Sequence)
        or isinstance(candidates, (str, bytes))
        or any(not isinstance(item, Mapping) for item in candidates)
    ):
        raise ValueError("candidate report candidates are invalid")
    candidate = _indexed(list(candidates), candidate_index, "candidate")
    discovery_id = _required_string(candidate, "discovery_id", "candidate")
    scan = scan_repository(root)
    scanned = {
        fact.stable_id: {
            "discovery_id": fact.stable_id,
            "discovery_source_kind": fact.source_kind,
            "path": fact.path,
            "name": fact.name,
            "ignore_state": fact.ignore_state,
        }
        for fact in scan.inferred_facts
    }
    fact = scanned.get(discovery_id)
    if fact is None:
        raise ValueError(f"pilot candidate is absent from live scan: {discovery_id}")
    inventory = invoke_rust_assertion_inventory(
        root, [fact], environment=os.environ
    )[discovery_id]
    metadata = load_code_spec_metadata(root)
    review = build_pilot_review(
        candidate=candidate,
        fact=fact,
        assertion_inventory=inventory,
        invariants=metadata.invariants,
        invariant_index=invariant_index,
        oracle_assertion_bindings=oracle_assertion_bindings,
        setup_assertion_indices=setup_assertion_indices,
        non_oracle_assertion_indices=non_oracle_assertion_indices,
        reviewer_ids=reviewer_ids,
    )
    document = {
        "schema_version": 1,
        "generator": "mapping-pilot-semantic-review",
        "generator_version": "1",
        "source_commit": candidate_document.get("source_commit"),
        "candidate_report_digest": _digest(candidate_document),
        "reviews": [review],
    }
    schema = json.loads(
        (
            root
            / "verification/schemas/mapping-ground-truth-review.schema.json"
        ).read_text()
    )
    failures = validate_json_schema_instance(document, schema)
    if failures:
        raise ValueError("; ".join(failures))
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    return document


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--candidates", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--candidate-index", type=int, required=True)
    parser.add_argument("--invariant-index", type=int, required=True)
    parser.add_argument(
        "--oracle-binding",
        action="append",
        required=True,
        metavar="ASSERTION_INDEX:BEHAVIOR_INDEX[,BEHAVIOR_INDEX]",
    )
    parser.add_argument("--setup-assertion", type=int, action="append", default=[])
    parser.add_argument("--non-oracle-assertion", type=int, action="append", default=[])
    parser.add_argument("--reviewer", action="append", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        document = generate_pilot_review(
            root=root,
            candidates_path=_resolve(root, args.candidates),
            output_path=_resolve(root, args.out),
            candidate_index=args.candidate_index,
            invariant_index=args.invariant_index,
            oracle_assertion_bindings=_parse_oracle_bindings(
                args.oracle_binding
            ),
            setup_assertion_indices=args.setup_assertion,
            non_oracle_assertion_indices=args.non_oracle_assertion,
            reviewer_ids=args.reviewer,
        )
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"mapping pilot review failed: {error}", file=sys.stderr)
        return 2
    review = document["reviews"][0]
    print(
        "mapping pilot review generated: "
        f"{review['discovery_id']} -> {review['selected_invariant_ids'][0]}, "
        f"{len(review['assertion_reviews'])} assertions classified"
    )
    return 0


def _indexed(values: Sequence[Any], one_based: int, label: str) -> Any:
    if one_based < 1 or one_based > len(values):
        raise ValueError(
            f"{label} index {one_based} is outside displayed range 1..{len(values)}"
        )
    return values[one_based - 1]


def _validated_indices(
    values: Sequence[int], size: int, label: str
) -> list[int]:
    if any(
        not isinstance(value, int)
        or isinstance(value, bool)
        or value < 1
        or value > size
        for value in values
    ):
        raise ValueError(f"{label} indices must stay within displayed range 1..{size}")
    zero_based = [value - 1 for value in values]
    if len(zero_based) != len(set(zero_based)):
        raise ValueError(f"{label} indices must be distinct")
    return sorted(zero_based)


def _validated_oracle_bindings(
    value: Mapping[int, Sequence[int]],
    *,
    assertion_count: int,
    behavior_count: int,
    invariant_id: str,
) -> dict[int, list[str]]:
    if not isinstance(value, Mapping) or not value:
        raise ValueError("pilot acceptance requires oracle assertion bindings")
    resolved: dict[int, list[str]] = {}
    for assertion_index, behavior_indices in value.items():
        assertion = _validated_indices(
            [assertion_index], assertion_count, "oracle assertion"
        )[0]
        if assertion in resolved:
            raise ValueError("pilot oracle assertion bindings must be distinct")
        behaviors = _validated_indices(
            behavior_indices, behavior_count, "behavior"
        )
        if not behaviors:
            raise ValueError("each pilot oracle assertion requires a behavior")
        resolved[assertion] = [
            f"{invariant_id}#behavior:{behavior}" for behavior in behaviors
        ]
    return resolved


def _parse_oracle_bindings(values: Sequence[str]) -> dict[int, list[int]]:
    parsed: dict[int, list[int]] = {}
    for value in values:
        assertion_raw, separator, behaviors_raw = value.partition(":")
        if (
            not separator
            or not assertion_raw.isdigit()
            or not behaviors_raw
        ):
            raise ValueError(
                "oracle binding must be ASSERTION_INDEX:BEHAVIOR_INDEX[,..]"
            )
        behavior_parts = behaviors_raw.split(",")
        if any(not part.isdigit() for part in behavior_parts):
            raise ValueError(
                "oracle binding behavior indices must be positive integers"
            )
        assertion_index = int(assertion_raw)
        if assertion_index in parsed:
            raise ValueError(
                f"oracle binding duplicates assertion {assertion_index}"
            )
        parsed[assertion_index] = [int(part) for part in behavior_parts]
    return parsed


def _strings(value: Any, label: str) -> list[str]:
    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes))
        or any(not isinstance(item, str) or not item for item in value)
    ):
        raise ValueError(f"{label} must be a non-empty-string array")
    return list(value)


def _required_string(
    record: Mapping[str, Any], field: str, label: str
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _digest(value: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _resolve(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path
