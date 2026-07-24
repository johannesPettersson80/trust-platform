"""Compile numeric reviewer recommendations into an ID-free review selection."""

from __future__ import annotations

import argparse
import json
import sys
import tomllib
from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .mapping_review_selection import canonical_digest
from .test_catalog_json_schema import (
    validate_json_schema_instance,
)


RECOMMENDATION_SCHEMA = Path(
    "verification/schemas/test-catalog-mapping-review-recommendations.schema.json"
)
SELECTION_SCHEMA = Path(
    "verification/schemas/test-catalog-mapping-review-selection.schema.json"
)
ACCEPTED_FIELDS = {
    "proposal_index",
    "outcome",
    "evidence",
    "selected_invariant_indices",
    "oracle_bindings",
    "setup_assertion_indices",
    "non_oracle_assertion_indices",
    "requires_network",
}
REJECTED_FIELDS = {
    "proposal_index",
    "outcome",
    "evidence",
    "rejection_reason",
}


def compile_review_recommendations(
    *,
    proposal_document: Mapping[str, Any],
    recommendation_documents: Sequence[Mapping[str, Any]],
    recommendation_schema: Mapping[str, Any],
    selection_schema: Mapping[str, Any],
    require_complete_queue: bool,
) -> dict[str, Any]:
    proposals = proposal_document.get("proposals")
    if (
        not isinstance(proposals, Sequence)
        or isinstance(proposals, (str, bytes))
        or any(not isinstance(proposal, Mapping) for proposal in proposals)
    ):
        raise ValueError("mapping review proposal queue must contain object rows")
    collected: dict[int, list[tuple[str, Mapping[str, Any]]]] = defaultdict(list)
    for document_index, document in enumerate(recommendation_documents, start=1):
        failures = validate_json_schema_instance(document, recommendation_schema)
        if failures:
            raise ValueError(
                f"mapping review recommendation {document_index}: "
                + "; ".join(failures)
            )
        reviewer_id = _required_string(
            document,
            "reviewer_id",
            f"recommendation {document_index}",
        )
        review_range = document.get("range")
        if (
            not isinstance(review_range, Sequence)
            or isinstance(review_range, (str, bytes))
            or len(review_range) != 2
            or any(
                not isinstance(index, int)
                or isinstance(index, bool)
                or index < 1
                for index in review_range
            )
            or review_range[0] > review_range[1]
        ):
            raise ValueError(
                f"mapping review recommendation {document_index} range is invalid"
            )
        reviews = document.get("reviews")
        if not isinstance(reviews, list):
            raise ValueError(
                f"mapping review recommendation {document_index} lacks reviews"
            )
        seen: set[int] = set()
        for review in reviews:
            if not isinstance(review, Mapping):
                raise ValueError(
                    f"mapping review recommendation {document_index} row is invalid"
                )
            proposal_index = review.get("proposal_index")
            if (
                not isinstance(proposal_index, int)
                or isinstance(proposal_index, bool)
                or proposal_index < review_range[0]
                or proposal_index > review_range[1]
                or proposal_index > len(proposals)
            ):
                raise ValueError(
                    f"mapping review recommendation {document_index} proposal "
                    "index is outside its declared range or queue"
                )
            if proposal_index in seen:
                raise ValueError(
                    f"mapping review recommendation {document_index} duplicates "
                    f"proposal {proposal_index}"
                )
            seen.add(proposal_index)
            collected[proposal_index].append((reviewer_id, review))

    if require_complete_queue and set(collected) != set(range(1, len(proposals) + 1)):
        missing = sorted(set(range(1, len(proposals) + 1)) - set(collected))
        extra = sorted(set(collected) - set(range(1, len(proposals) + 1)))
        raise ValueError(
            "mapping review recommendations do not cover the complete queue: "
            f"missing={missing} extra={extra}"
        )
    if not collected:
        raise ValueError("mapping review recommendations contain no decisions")

    compiled = [
        _compile_proposal_recommendations(proposal_index, recommendations)
        for proposal_index, recommendations in sorted(collected.items())
    ]
    selection = {
        "schema_version": 1,
        "proposal_queue_digest": canonical_digest(proposal_document),
        "reviews": compiled,
    }
    failures = validate_json_schema_instance(selection, selection_schema)
    if failures:
        raise ValueError("; ".join(failures))
    return selection


def _compile_proposal_recommendations(
    proposal_index: int,
    recommendations: Sequence[tuple[str, Mapping[str, Any]]],
) -> dict[str, Any]:
    normalized: list[tuple[str, dict[str, Any]]] = []
    for reviewer_id, review in recommendations:
        outcome = review.get("outcome")
        expected_fields = (
            ACCEPTED_FIELDS if outcome == "accepted" else REJECTED_FIELDS
        )
        if set(review) != expected_fields:
            raise ValueError(
                f"mapping review proposal {proposal_index} recommendation fields drift"
            )
        semantic = {
            key: value
            for key, value in review.items()
            if key != "evidence"
        }
        normalized.append((reviewer_id, semantic))
    reference = normalized[0][1]
    if any(semantic != reference for _, semantic in normalized[1:]):
        raise ValueError(
            f"mapping review proposal {proposal_index} reviewers conflict"
        )
    result = dict(reference)
    result["reviewer_ids"] = sorted(
        {reviewer_id for reviewer_id, _ in normalized}
    )
    if result["outcome"] == "accepted":
        result["confirm_generated_catalog"] = True
    return result


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--proposals",
        type=Path,
        default=Path("verification/test-catalog-proposals.toml"),
    )
    parser.add_argument(
        "--recommendation",
        type=Path,
        action="append",
        required=True,
    )
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--require-complete-queue", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        proposal_document = _load_toml(_resolve(root, args.proposals))
        recommendation_documents = [
            _load_json(_resolve(root, path))
            for path in args.recommendation
        ]
        selection = compile_review_recommendations(
            proposal_document=proposal_document,
            recommendation_documents=recommendation_documents,
            recommendation_schema=_load_json(root / RECOMMENDATION_SCHEMA),
            selection_schema=_load_json(root / SELECTION_SCHEMA),
            require_complete_queue=args.require_complete_queue,
        )
        output = _resolve(root, args.out)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(selection, indent=2, sort_keys=True) + "\n")
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"mapping review recommendation compilation failed: {error}", file=sys.stderr)
        return 2
    print(
        "mapping review selection compiled: "
        f"{len(selection['reviews'])} numeric decisions"
    )
    return 0


def _load_json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} root must be an object")
    return document


def _load_toml(path: Path) -> dict[str, Any]:
    document = tomllib.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} root must be a table")
    return document


def _resolve(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _required_string(value: Mapping[str, Any], field: str, label: str) -> str:
    result = value.get(field)
    if not isinstance(result, str) or not result:
        raise ValueError(f"{label} requires non-empty {field}")
    return result
