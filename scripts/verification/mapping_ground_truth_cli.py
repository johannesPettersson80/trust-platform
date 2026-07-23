"""Qualify mapping candidates against existing reviewed catalog mappings."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from .code_spec_live import load_code_spec_metadata
from .mapping_ground_truth import (
    evaluate_ground_truth,
    partition_ground_truth_candidates,
)
from .test_catalog_json_schema import validate_json_schema_instance


def generate_ground_truth_report(
    *,
    root: Path,
    candidate_path: Path,
    review_path: Path,
    output_path: Path,
) -> dict[str, Any]:
    candidate_document = json.loads(candidate_path.read_text())
    if not isinstance(candidate_document, dict):
        raise ValueError("candidate report root must be an object")
    review_document = json.loads(review_path.read_text())
    if not isinstance(review_document, dict):
        raise ValueError("pilot review root must be an object")
    review_schema = json.loads(
        (
            root
            / "verification/schemas/mapping-ground-truth-review.schema.json"
        ).read_text()
    )
    review_failures = validate_json_schema_instance(
        review_document, review_schema
    )
    if review_failures:
        raise ValueError("; ".join(review_failures))
    if review_document.get("candidate_report_digest") != _digest(
        candidate_document
    ):
        raise ValueError("pilot review candidate report digest is stale")
    if review_document.get("source_commit") != candidate_document.get(
        "source_commit"
    ):
        raise ValueError("pilot review source commit is stale")
    metadata = load_code_spec_metadata(root)
    expected = {
        str(test["discovery_id"]): list(test.get("invariants", []))
        for test in metadata.tests.values()
        if test.get("subject_kind") == "generated_test"
        and isinstance(test.get("discovery_id"), str)
    }
    raw_candidates = candidate_document.get("candidates", [])
    if not isinstance(raw_candidates, list) or any(
        not isinstance(candidate, dict) for candidate in raw_candidates
    ):
        raise ValueError("candidate report candidates must be an object array")
    ground_truth_candidates, non_ground_truth_ids = (
        partition_ground_truth_candidates(
            candidates=raw_candidates,
            expected_invariants=expected,
        )
    )
    analysis = evaluate_ground_truth(
        candidates=ground_truth_candidates,
        expected_invariants=expected,
        accepted_reviews=review_document.get("reviews", []),
    )
    report = {
        "schema_version": 1,
        "generator": "mapping-ground-truth-pilot",
        "generator_version": "1",
        "source_commit": candidate_document.get("source_commit"),
        "candidate_report_digest": _digest(candidate_document),
        "pilot_review_digest": _digest(review_document),
        "candidate_signals": len(raw_candidates),
        "non_ground_truth_candidate_ids": non_ground_truth_ids,
        **analysis,
    }
    schema = json.loads(
        (
            root
            / "verification/schemas/mapping-ground-truth-report.schema.json"
        ).read_text()
    )
    failures = validate_json_schema_instance(report, schema)
    if failures:
        raise ValueError("; ".join(failures))
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--candidates", type=Path, required=True)
    parser.add_argument("--reviews", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        report = generate_ground_truth_report(
            root=root,
            candidate_path=_resolve(root, args.candidates),
            review_path=_resolve(root, args.reviews),
            output_path=_resolve(root, args.out),
        )
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"mapping ground-truth pilot failed: {error}", file=sys.stderr)
        return 2
    if not report["zero_false_accepted_mappings"]:
        print(
            "mapping ground-truth pilot found false or vacuous accepted mappings",
            file=sys.stderr,
        )
        return 1
    print(
        "mapping ground-truth pilot passed: "
        f"{len(report['exact_accepted_mappings'])}/"
        f"{report['reviewed_acceptances']} exact accepted, "
        "zero false accepted mappings; "
        f"candidate signals exact={len(report['exact_candidates'])}/"
        f"{report['pilot_candidates']} ground truth, "
        f"{len(report['non_ground_truth_candidate_ids'])} quarantined unknown"
    )
    return 0


def _digest(document: dict[str, Any]) -> str:
    encoded = json.dumps(
        document, sort_keys=True, separators=(",", ":")
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _resolve(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path
