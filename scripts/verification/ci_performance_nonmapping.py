"""Classify CI wall-clock probes that lack a product performance oracle."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.mapping_catalog_updates import (
    render_denominator_document,
)
from scripts.verification.mapping_transaction import apply_mapping_transaction
from scripts.verification.test_catalog_denominator import (
    DENOMINATOR_PATH,
    DENOMINATOR_SCHEMA_PATH,
    DATE_RE,
    validate_test_catalog_denominator_document,
)


PATH = "crates/trust-runtime/tests/ci_cicd_contract.rs"
SOURCE_REASON = "no_reviewed_spec_or_invariant_binding"
TARGET_REASON = "not_catalogable_product_oracle"
PERFORMANCE_TESTS = frozenset(
    {
        "ci_clean_setup_first_passing_test_is_under_ten_minutes",
        "ci_single_test_feedback_is_under_two_seconds_for_small_project",
        "ci_run_all_tests_scales_roughly_linearly_for_small_projects",
    }
)


def classify_ci_performance_probes(
    *,
    document: Mapping[str, Any],
    reviewed_date: str,
) -> dict[str, Any]:
    """Return an exact denominator update without inferring semantic mappings."""

    if not DATE_RE.fullmatch(reviewed_date):
        raise ValueError("CI performance review requires YYYY-MM-DD review date")
    reviews = document.get("reviews")
    if not isinstance(reviews, Sequence) or isinstance(reviews, (str, bytes)):
        raise ValueError("test-catalog denominator requires review rows")

    matches: dict[str, tuple[int, Mapping[str, Any]]] = {}
    for index, row in enumerate(reviews):
        if not isinstance(row, Mapping):
            raise ValueError(f"denominator review {index} must be a table")
        if row.get("path") != PATH:
            continue
        name = row.get("name")
        if name not in PERFORMANCE_TESTS:
            continue
        if not isinstance(name, str) or name in matches:
            raise ValueError("CI performance denominator identities are not unique")
        matches[name] = (index, row)
    if set(matches) != PERFORMANCE_TESTS:
        missing = sorted(PERFORMANCE_TESTS - set(matches))
        raise ValueError(
            "CI performance denominator differs from reviewed probes: "
            + ", ".join(missing)
        )

    reasons = {row.get("reason_code") for _, row in matches.values()}
    dispositions = {row.get("disposition") for _, row in matches.values()}
    if dispositions != {"reviewed_nonmapping"}:
        raise ValueError("CI performance probes must remain reviewed nonmappings")
    if reasons == {TARGET_REASON}:
        return dict(document)
    if reasons != {SOURCE_REASON}:
        raise ValueError("CI performance probe reasons are partially or unexpectedly classified")

    updated_reviews = [dict(row) for row in reviews]
    for index, row in matches.values():
        updated = dict(row)
        updated["reason_code"] = TARGET_REASON
        updated["last_reviewed"] = reviewed_date
        updated_reviews[index] = updated

    updated = dict(document)
    updated["reviews"] = updated_reviews
    updated["last_reviewed"] = reviewed_date
    return updated


def apply_ci_performance_classification(
    *,
    root: Path,
    reviewed_date: str,
) -> dict[str, Any]:
    denominator_path = root / DENOMINATOR_PATH
    original = denominator_path.read_bytes()
    document = tomllib.loads(original.decode("utf-8"))
    updated = classify_ci_performance_probes(
        document=document,
        reviewed_date=reviewed_date,
    )
    rendered = render_denominator_document(updated).encode("utf-8")
    schema = json.loads((root / DENOMINATOR_SCHEMA_PATH).read_text())

    def validate() -> list[str]:
        current = tomllib.loads(denominator_path.read_text())
        failures = validate_test_catalog_denominator_document(
            current,
            schema=schema,
        )
        try:
            classified = classify_ci_performance_probes(
                document=current,
                reviewed_date=reviewed_date,
            )
        except ValueError as error:
            failures.append(str(error))
        else:
            if classified != current:
                failures.append("CI performance classification did not persist")
        return failures

    if rendered != original:
        apply_mapping_transaction(
            root=root,
            replacements={DENOMINATOR_PATH: rendered},
            expected_originals={DENOMINATOR_PATH: original},
            validate=validate,
        )
    return {
        "schema_version": 1,
        "path": PATH,
        "tests": sorted(PERFORMANCE_TESTS),
        "reason_code": TARGET_REASON,
        "changed": rendered != original,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--reviewed-date", required=True)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()

    root = args.root.resolve()
    if args.apply:
        result = apply_ci_performance_classification(
            root=root,
            reviewed_date=args.reviewed_date,
        )
    else:
        document = tomllib.loads(
            (root / DENOMINATOR_PATH).read_text()
        )
        updated = classify_ci_performance_probes(
            document=document,
            reviewed_date=args.reviewed_date,
        )
        result = {
            "schema_version": 1,
            "path": PATH,
            "tests": sorted(PERFORMANCE_TESTS),
            "reason_code": TARGET_REASON,
            "changed": updated != document,
        }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
