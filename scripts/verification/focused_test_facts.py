"""Exact per-file reconstruction of reviewed Rust discovery facts."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .test_catalog_common import ScanBatch, input_digest
from .test_catalog_models import InferredTestFact
from .test_catalog_rust import (
    integration_target,
    load_package_name,
    scan_rust_file,
)


RUST_SOURCE_KINDS = {"rust_unit_test", "rust_integration_test"}


def resolve_focused_rust_facts(
    *,
    root: Path,
    discovery_ids: Sequence[str],
    denominator: Mapping[str, Any],
) -> list[InferredTestFact]:
    """Reconstruct requested Rust facts without a repository-wide scan."""

    root = root.resolve()
    requested = sorted(discovery_ids)
    if not requested or len(requested) != len(set(requested)):
        raise ValueError("focused Rust discovery IDs must be non-empty and unique")
    reviews = denominator.get("reviews")
    if (
        not isinstance(reviews, Sequence)
        or isinstance(reviews, (str, bytes))
        or any(not isinstance(review, Mapping) for review in reviews)
    ):
        raise ValueError("test-catalog denominator reviews are malformed")
    reviews_by_id: dict[str, Mapping[str, Any]] = {}
    for review in reviews:
        discovery_id = review.get("discovery_id")
        if not isinstance(discovery_id, str) or not discovery_id:
            raise ValueError("test-catalog denominator review lacks discovery_id")
        if discovery_id in reviews_by_id:
            raise ValueError(
                f"test-catalog denominator duplicates {discovery_id}"
            )
        reviews_by_id[discovery_id] = review
    missing = sorted(set(requested) - set(reviews_by_id))
    if missing:
        raise ValueError(
            f"focused Rust facts are absent from reviewed denominator: {missing}"
        )

    by_path: dict[tuple[str, str], list[Mapping[str, Any]]] = {}
    for discovery_id in requested:
        review = reviews_by_id[discovery_id]
        source_kind = review.get("discovery_source_kind")
        if source_kind not in RUST_SOURCE_KINDS:
            raise ValueError(f"{discovery_id} is not a Rust test")
        path = review.get("path")
        if not isinstance(path, str) or not path:
            raise ValueError(f"{discovery_id} lacks a reviewed source path")
        by_path.setdefault((str(source_kind), path), []).append(review)

    resolved: dict[str, InferredTestFact] = {}
    for (source_kind, relative), path_reviews in sorted(by_path.items()):
        source = _reviewed_source(root, relative)
        package, command_prefix, authority = _rust_route(
            root=root,
            source=source,
            source_kind=source_kind,
        )
        batch = scan_rust_file(
            root,
            source,
            package=package,
            source_kind=source_kind,
            command_prefix=command_prefix,
            command_authority=authority,
        )
        errors = [
            diagnostic.message
            for diagnostic in batch.diagnostics
            if diagnostic.severity == "error"
        ]
        if errors:
            raise ValueError(
                f"focused Rust scan failed for {relative}: {errors}"
            )
        facts_by_id = {fact.stable_id: fact for fact in batch.facts}
        if len(facts_by_id) != len(batch.facts):
            raise ValueError(
                f"focused Rust source duplicates discovery IDs: {relative}"
            )
        for review in path_reviews:
            discovery_id = str(review["discovery_id"])
            fact = facts_by_id.get(discovery_id)
            if fact is None:
                raise ValueError(
                    f"{discovery_id} is absent from its reviewed live source file"
                )
            _validate_review_identity(review, fact)
            resolved[discovery_id] = fact
    return [resolved[discovery_id] for discovery_id in requested]


def focused_rust_input_digest(
    *,
    root: Path,
    facts: Sequence[InferredTestFact],
    denominator: Mapping[str, Any],
) -> str:
    """Bind the focused denominator rows and exact scanner input files."""

    discovery_ids = [fact.stable_id for fact in facts]
    if not discovery_ids or len(discovery_ids) != len(set(discovery_ids)):
        raise ValueError("focused Rust digest requires unique discovery facts")
    reviews = denominator.get("reviews")
    if not isinstance(reviews, Sequence) or isinstance(reviews, (str, bytes)):
        raise ValueError("test-catalog denominator reviews are malformed")
    by_id = {
        str(review.get("discovery_id")): dict(review)
        for review in reviews
        if isinstance(review, Mapping)
        and isinstance(review.get("discovery_id"), str)
    }
    if len(by_id) != len(reviews):
        raise ValueError("test-catalog denominator review identities are malformed")
    selected = [by_id[discovery_id] for discovery_id in sorted(discovery_ids)]
    paths = sorted(
        {
            path
            for fact in facts
            for path in (
                fact.path,
                f"crates/{Path(fact.path).parts[1]}/Cargo.toml",
            )
        }
    )
    payload = {
        "kind": "focused_reviewed_rust_test_inputs",
        "reviews": selected,
        "source_input_digest": input_digest(root.resolve(), paths),
    }
    encoded = json.dumps(
        payload,
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _reviewed_source(root: Path, relative: str) -> Path:
    candidate = root / relative
    try:
        resolved = candidate.resolve(strict=True)
        resolved.relative_to(root)
    except (OSError, ValueError) as error:
        raise ValueError(
            f"reviewed focused source is missing or escapes the repository: {relative}"
        ) from error
    if candidate.is_symlink() or not resolved.is_file():
        raise ValueError(f"reviewed focused source is not a regular file: {relative}")
    return resolved


def _rust_route(
    *,
    root: Path,
    source: Path,
    source_kind: str,
) -> tuple[str, str, str]:
    relative = source.relative_to(root)
    if len(relative.parts) < 4 or relative.parts[0] != "crates":
        raise ValueError(f"focused Rust source is outside crates/*: {relative}")
    crate_dir = root / relative.parts[0] / relative.parts[1]
    manifest_batch = ScanBatch()
    package = load_package_name(root, crate_dir / "Cargo.toml", manifest_batch)
    errors = [
        diagnostic.message
        for diagnostic in manifest_batch.diagnostics
        if diagnostic.severity == "error"
    ]
    if errors:
        raise ValueError(f"focused Rust package manifest is invalid: {errors}")
    if source_kind == "rust_unit_test":
        if not source.is_relative_to(crate_dir / "src"):
            raise ValueError("reviewed Rust unit test path is outside crate src")
        return package, f"cargo test -p {package}", "package_only"
    tests_dir = crate_dir / "tests"
    if not source.is_relative_to(tests_dir):
        raise ValueError("reviewed Rust integration test path is outside crate tests")
    target = integration_target(tests_dir, source)
    if target is None:
        return package, f"cargo test -p {package}", "package_only"
    return package, f"cargo test -p {package} --test {target}", "conservative"


def _validate_review_identity(
    review: Mapping[str, Any],
    fact: InferredTestFact,
) -> None:
    expected = {
        "discovery_id": fact.stable_id,
        "discovery_source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
        "ignore_state": fact.ignore_state,
    }
    for field, value in expected.items():
        if review.get(field) != value:
            raise ValueError(
                f"{fact.stable_id} reviewed {field} is stale: "
                f"{review.get(field)!r} != {value!r}"
            )
