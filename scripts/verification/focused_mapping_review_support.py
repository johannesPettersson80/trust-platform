"""State, path, and transaction helpers for focused mapping review."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .code_spec_live import CodeSpecMetadata
from .focused_mapping_context import FocusedMappingContext
from .mapping_review_batch import MappingReviewBatch
from .test_catalog_denominator import (
    analyze_test_catalog_denominator,
    validate_test_catalog_denominator_document,
)
from .test_catalog_json_schema import validate_json_schema_instance


PROPOSAL_PATH = Path("verification/test-catalog-proposals.toml")
DECISION_PATH = Path("verification/test-catalog-mapping-decisions.toml")
CATALOG_PATH = Path("verification/test-catalog.toml")
CODE_MAP_PATH = Path("verification/code-spec-map.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
IGNORED_PATH = Path("verification/ignored-tests.toml")
SELECTION_SCHEMA_PATH = Path(
    "verification/schemas/test-catalog-mapping-review-selection.schema.json"
)
REVIEW_SCHEMA_PATH = Path(
    "verification/schemas/test-catalog-mapping-review-input.schema.json"
)
PROPOSAL_SCHEMA_PATH = Path(
    "verification/schemas/test-catalog-proposals.schema.json"
)
DECISION_SCHEMA_PATH = Path(
    "verification/schemas/test-catalog-mapping-decisions.schema.json"
)
DEFAULT_RECEIPT_PATH = Path(
    "target/gate-artifacts/verification/focused-mapping-review-receipt.json"
)
QUARANTINE_ROOT = Path("target/gate-artifacts")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")


def validate_focused_post_state(
    *,
    root: Path,
    context: FocusedMappingContext,
    selected_proposals: Sequence[Mapping[str, Any]],
    review_inputs: Sequence[Mapping[str, Any]],
    batch: MappingReviewBatch,
    ignored_tests: Sequence[Mapping[str, Any]],
    denominator_schema: Mapping[str, Any],
) -> list[str]:
    failures: list[str] = []
    catalog = load_toml_file(root / CATALOG_PATH)
    tests = index_records(catalog.get("tests"), "catalog test")
    denominator = load_toml_file(root / DENOMINATOR_PATH)
    failures.extend(
        validate_test_catalog_denominator_document(
            denominator, schema=denominator_schema
        )
    )
    try:
        analyze_test_catalog_denominator(
            facts=context.frozen_facts,
            tests=list(tests.values()),
            reviews=denominator.get("reviews", []),
            ignored_tests=ignored_tests,
        )
    except ValueError as error:
        failures.append(str(error))
    reviews = {
        str(row.get("discovery_id")): row
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and isinstance(row.get("discovery_id"), str)
    }
    review_by_proposal = {
        str(review.get("proposal_id")): review for review in review_inputs
    }
    catalog_by_discovery = {
        str(test.get("discovery_id")): test
        for test in tests.values()
        if test.get("subject_kind") == "generated_test"
        and isinstance(test.get("discovery_id"), str)
    }
    accepted_ids = set(batch.accepted_test_ids)
    rejected_ids = set(batch.rejected_proposal_ids)
    for proposal in selected_proposals:
        proposal_id = str(proposal.get("id"))
        discovery_id = str(proposal.get("discovery_id"))
        review = review_by_proposal.get(proposal_id)
        if review is None:
            failures.append(f"{proposal_id} lacks focused review input")
            continue
        if review.get("outcome") == "accepted":
            test_id = "TEST_AUTO_" + discovery_id.removeprefix("DISC_")
            test = tests.get(test_id)
            denominator_review = reviews.get(discovery_id)
            if test_id not in accepted_ids or test is None:
                failures.append(
                    f"{proposal_id} accepted test {test_id} is absent"
                )
                continue
            for field in (
                "discovery_id",
                "discovery_source_kind",
                "path",
                "name",
            ):
                if test.get(field) != proposal.get(field):
                    failures.append(
                        f"{test_id} {field} differs from selected proposal"
                    )
            selected_invariants = string_array(
                review.get("selected_invariant_ids"),
                f"{proposal_id} selected invariants",
            )
            if not set(selected_invariants).issubset(
                set(test.get("invariants", []))
            ):
                failures.append(
                    f"{test_id} lacks selected invariant forward links"
                )
            for invariant_id in selected_invariants:
                try:
                    source = find_invariant_file(root, invariant_id)
                    invariant = load_toml_file(source)
                except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
                    failures.append(str(error))
                    continue
                if test_id not in invariant.get("tests", []):
                    failures.append(
                        f"{test_id} is not reciprocally linked by "
                        f"invariant {invariant_id}"
                    )
            if (
                not isinstance(denominator_review, Mapping)
                or denominator_review.get("disposition") != "catalog_mapped"
                or denominator_review.get("catalog_test_id") != test_id
            ):
                failures.append(
                    f"{test_id} lacks exact mapped denominator review"
                )
        else:
            if proposal_id not in rejected_ids:
                failures.append(f"{proposal_id} rejection was not recorded")
            denominator_review = reviews.get(discovery_id)
            if (
                discovery_id in catalog_by_discovery
                or not isinstance(denominator_review, Mapping)
                or denominator_review.get("disposition")
                != "reviewed_nonmapping"
            ):
                failures.append(
                    f"{proposal_id} rejected fact changed catalog disposition"
                )
    return sorted(set(failures))


def build_receipt(
    *,
    source_commit: str,
    reviewed_date: str,
    proposal_bytes: bytes,
    selection_bytes: bytes,
    context: FocusedMappingContext,
    batch: MappingReviewBatch,
    projected_denominator: Mapping[str, Any],
    replacements: Mapping[str, bytes],
    authority_fields: Mapping[str, Any],
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "kind": "focused_mapping_review_receipt",
        "basis": "frozen_denominator_plus_live_selected_rust",
        "source_commit": source_commit,
        "reviewed_date": reviewed_date,
        "proposal_queue_sha256": digest(proposal_bytes),
        "review_selection_sha256": digest(selection_bytes),
        "frozen_fact_count": len(context.frozen_facts),
        "live_selected_discovery_ids": sorted(context.selected_facts),
        "accepted_test_ids": list(batch.accepted_test_ids),
        "rejected_proposal_ids": list(batch.rejected_proposal_ids),
        "denominator_before": denominator_counts(context.denominator),
        "denominator_after": denominator_counts(projected_denominator),
        "output_sha256": {
            path: digest(content)
            for path, content in sorted(replacements.items())
        },
        "authority": dict(authority_fields),
        "broad_reports_regenerated": False,
        "limitations": [
            "Unselected test identities come from the reviewed frozen "
            "denominator, not a new live repository scan.",
            "Canonical broad debt and code-spec coverage reports remain "
            "unchanged until the final frozen campaign.",
        ],
    }


def denominator_counts(document: Mapping[str, Any]) -> dict[str, int]:
    reviews = document.get("reviews")
    if not isinstance(reviews, Sequence) or isinstance(
        reviews, (str, bytes)
    ):
        raise ValueError("denominator count source lacks reviews")
    mapped = sum(
        1
        for row in reviews
        if isinstance(row, Mapping)
        and row.get("disposition") == "catalog_mapped"
    )
    nonmapping = sum(
        1
        for row in reviews
        if isinstance(row, Mapping)
        and row.get("disposition") == "reviewed_nonmapping"
    )
    if mapped + nonmapping != len(reviews):
        raise ValueError("denominator count source has unknown dispositions")
    return {
        "frozen_facts": len(reviews),
        "catalog_mapped": mapped,
        "reviewed_nonmapping": nonmapping,
    }


def read_only_preconditions(
    *,
    root: Path,
    selection_file: Path,
    selection_bytes: bytes,
    authority_file: Path | None,
    selected_facts: Mapping[str, Mapping[str, Any]],
    ignored_bytes: bytes,
) -> dict[str, bytes | None]:
    expected: dict[str, bytes | None] = {
        selection_file.relative_to(root).as_posix(): selection_bytes,
        IGNORED_PATH.as_posix(): ignored_bytes,
    }
    if authority_file is not None:
        expected[authority_file.relative_to(root).as_posix()] = (
            authority_file.read_bytes()
        )
    for discovery_id, fact in sorted(selected_facts.items()):
        path = required_string(fact, "path", discovery_id)
        source = repository_file(root, root / path, f"{discovery_id} source")
        expected[source.relative_to(root).as_posix()] = source.read_bytes()
        parts = Path(path).parts
        if len(parts) >= 2 and parts[0] == "crates":
            manifest = repository_file(
                root,
                root / parts[0] / parts[1] / "Cargo.toml",
                f"{discovery_id} Cargo manifest",
            )
            expected[manifest.relative_to(root).as_posix()] = (
                manifest.read_bytes()
            )
    return expected


def replacement_preconditions(
    root: Path, replacements: Mapping[str, bytes]
) -> dict[str, bytes | None]:
    expected: dict[str, bytes | None] = {}
    for relative in replacements:
        target = repository_output(root, root / relative, relative)
        expected[relative] = target.read_bytes() if target.exists() else None
    return expected


def validate_exact_replacements(
    root: Path, replacements: Mapping[str, bytes]
) -> list[str]:
    return [
        f"{relative}: focused replacement differs"
        for relative, expected in replacements.items()
        if not (root / relative).is_file()
        or (root / relative).read_bytes() != expected
    ]


def validate_read_only_inputs(
    root: Path, expected: Mapping[str, bytes | None]
) -> list[str]:
    failures = []
    for relative, before in expected.items():
        target = root / relative
        after = target.read_bytes() if target.is_file() else None
        if after != before:
            failures.append(
                f"{relative}: focused read-only input changed during transaction"
            )
    return failures


def projected_invariants(
    *,
    metadata: CodeSpecMetadata,
    replacements: Mapping[str, bytes],
    invariant_sources: Mapping[str, tuple[str, str]],
) -> dict[str, Mapping[str, Any]]:
    projected = dict(metadata.invariants)
    for invariant_id, (path, _) in invariant_sources.items():
        content = replacements.get(path)
        if content is not None:
            projected[invariant_id] = load_toml_bytes(
                content, f"projected invariant {invariant_id}"
            )
    return dict(sorted(projected.items()))


def load_selected_invariant_sources(
    root: Path, invariant_ids: Sequence[str]
) -> dict[str, tuple[str, str]]:
    result = {}
    for invariant_id in sorted(set(invariant_ids)):
        path = find_invariant_file(root, invariant_id)
        result[invariant_id] = (
            path.relative_to(root).as_posix(),
            path.read_text(),
        )
    return result


def find_invariant_file(root: Path, invariant_id: str) -> Path:
    candidates = sorted(
        (root / "verification/invariants").rglob(f"{invariant_id}.toml")
    )
    matches = []
    for candidate in candidates:
        path = repository_file(root, candidate, f"invariant {invariant_id}")
        record = load_toml_file(path)
        if record.get("id") == invariant_id:
            matches.append(path)
    if len(matches) != 1:
        raise ValueError(
            f"selected invariant {invariant_id} does not resolve to one "
            "regular source file"
        )
    return matches[0]


def capture_initial_originals(
    *,
    root: Path,
    canonical_files: Mapping[Path, Path],
    decision_file: Path | None,
    selection_file: Path,
    receipt_file: Path,
    authority_file: Path | None,
) -> dict[str, bytes | None]:
    paths = [*canonical_files.values(), selection_file, receipt_file]
    if authority_file is not None:
        paths.append(authority_file)
    originals = {
        path.relative_to(root).as_posix(): (
            path.read_bytes() if path.is_file() else None
        )
        for path in paths
    }
    originals[DECISION_PATH.as_posix()] = (
        decision_file.read_bytes() if decision_file is not None else None
    )
    invariant_root = root / "verification/invariants"
    if not invariant_root.is_dir() or invariant_root.is_symlink():
        raise ValueError("focused invariant root must be a regular directory")
    for candidate in sorted(invariant_root.rglob("*")):
        if candidate.is_symlink():
            raise ValueError(
                f"focused invariant path may not be a symlink: {candidate}"
            )
    for candidate in sorted(invariant_root.rglob("*.toml")):
        path = repository_file(root, candidate, "focused invariant source")
        originals[path.relative_to(root).as_posix()] = path.read_bytes()
    return dict(sorted(originals.items()))


def capture_selected_source_originals(
    root: Path,
    proposals: Sequence[Mapping[str, Any]],
) -> dict[str, bytes]:
    originals: dict[str, bytes] = {}
    for proposal in proposals:
        discovery_id = required_string(
            proposal, "discovery_id", "selected proposal"
        )
        relative = required_string(proposal, "path", discovery_id)
        source = repository_file(
            root, root / relative, f"{discovery_id} source"
        )
        originals[source.relative_to(root).as_posix()] = source.read_bytes()
        parts = Path(relative).parts
        if len(parts) < 2 or parts[0] != "crates":
            raise ValueError(
                f"{discovery_id} focused Rust source is outside a crate"
            )
        manifest = repository_file(
            root,
            root / parts[0] / parts[1] / "Cargo.toml",
            f"{discovery_id} Cargo manifest",
        )
        originals[manifest.relative_to(root).as_posix()] = (
            manifest.read_bytes()
        )
    return dict(sorted(originals.items()))


def capture_assertion_source_originals(
    root: Path,
    inventories: Mapping[str, Mapping[str, Any]],
) -> dict[str, bytes]:
    """Freeze every source byte authenticated by fresh assertion inventories."""

    originals: dict[str, bytes] = {}
    for discovery_id, inventory in inventories.items():
        source_digests = inventory.get("source_digests")
        if not isinstance(source_digests, Mapping) or not source_digests:
            raise ValueError(
                f"{discovery_id} assertion inventory lacks source closure"
            )
        for relative, expected_digest in source_digests.items():
            if (
                not isinstance(relative, str)
                or not relative
                or not isinstance(expected_digest, str)
            ):
                raise ValueError(
                    f"{discovery_id} assertion source closure is malformed"
                )
            source = repository_file(
                root,
                root / relative,
                f"{discovery_id} assertion source",
            )
            content = source.read_bytes()
            live_digest = "sha256:" + hashlib.sha256(content).hexdigest()
            if live_digest != expected_digest:
                raise ValueError(
                    f"{discovery_id} assertion source changed during review: "
                    f"{relative}"
                )
            previous = originals.setdefault(relative, content)
            if previous != content:
                raise ValueError(
                    f"assertion source closure conflicts at {relative}"
                )
    return dict(sorted(originals.items()))


def selected_proposals_from_numbered_selection(
    *,
    proposal_document: Mapping[str, Any],
    selection_document: Mapping[str, Any],
) -> list[Mapping[str, Any]]:
    proposals = proposal_document.get("proposals")
    reviews = selection_document.get("reviews")
    if (
        not isinstance(proposals, list)
        or not isinstance(reviews, list)
        or not reviews
    ):
        raise ValueError("numbered review selection requires proposal rows")
    selected = []
    seen = set()
    for review in reviews:
        if not isinstance(review, Mapping):
            raise ValueError("numbered review selections must be objects")
        index = review.get("proposal_index")
        if (
            not isinstance(index, int)
            or isinstance(index, bool)
            or index < 1
            or index > len(proposals)
            or index in seen
        ):
            raise ValueError(
                "numbered review proposal indices must be unique and in range"
            )
        seen.add(index)
        proposal = proposals[index - 1]
        if not isinstance(proposal, Mapping):
            raise ValueError("numbered review selected proposal must be an object")
        selected.append(proposal)
    return selected


def captured_bytes(
    root: Path,
    originals: Mapping[str, bytes | None],
    path: Path,
) -> bytes:
    relative = path.relative_to(root).as_posix()
    content = originals.get(relative)
    if content is None:
        raise ValueError(f"required initial byte snapshot is absent: {relative}")
    return content


def require_unchanged_originals(
    root: Path, originals: Mapping[str, bytes | None]
) -> None:
    for relative, expected in originals.items():
        target = root / relative
        actual = target.read_bytes() if target.is_file() else None
        if actual != expected:
            raise ValueError(
                f"focused input changed after initial snapshot: {relative}"
            )


def selected_invariant_sources_from_snapshot(
    *,
    originals: Mapping[str, bytes | None],
    invariant_ids: Sequence[str],
) -> dict[str, tuple[str, str]]:
    indexed: dict[str, tuple[str, str]] = {}
    for relative, content in originals.items():
        if (
            not relative.startswith("verification/invariants/")
            or not relative.endswith(".toml")
            or content is None
        ):
            continue
        record = load_toml_bytes(content, relative)
        invariant_id = record.get("id")
        if not isinstance(invariant_id, str) or not invariant_id:
            raise ValueError(f"focused invariant source lacks ID: {relative}")
        if invariant_id in indexed:
            raise ValueError(f"focused invariant ID is duplicated: {invariant_id}")
        indexed[invariant_id] = (relative, content.decode())
    missing = sorted(set(invariant_ids) - set(indexed))
    if missing:
        raise ValueError(
            "selected invariants lack initial byte snapshots: "
            + ", ".join(missing)
        )
    return {
        invariant_id: indexed[invariant_id]
        for invariant_id in sorted(set(invariant_ids))
    }


def require_exact_replacement_paths(
    replacements: Mapping[str, bytes],
    *,
    allowed: set[str],
    label: str,
) -> None:
    actual = set(replacements)
    unexpected = sorted(actual - allowed)
    missing = sorted(allowed - actual)
    if unexpected or missing:
        details = []
        if unexpected:
            details.append("disallowed: " + ", ".join(unexpected))
        if missing:
            details.append("missing: " + ", ".join(missing))
        raise ValueError(
            f"{label} replacement paths differ from the exact allowlist: "
            + "; ".join(details)
        )


def expected_authority_replacement_paths(
    *, root: Path, authority_file: Path | None
) -> set[str]:
    if authority_file is None:
        return set()
    output = authority_file.with_name(
        authority_file.stem + ".application.json"
    )
    projected = output.with_name(
        output.stem + ".projected-code-spec-map.toml"
    )
    return {
        "verification/code-spec-map.toml",
        output.relative_to(root).as_posix(),
        projected.relative_to(root).as_posix(),
    }


def validate_authority_batch_scope(
    *,
    application_manifest: Mapping[str, Any],
    review_inputs: Sequence[Mapping[str, Any]],
    selected_proposals: Sequence[Mapping[str, Any]],
    canonical_mappings: Mapping[str, Mapping[str, Any]],
) -> None:
    reviews = {
        required_string(review, "proposal_id", "mapping review input"): review
        for review in review_inputs
    }
    accepted_by_discovery = {}
    for proposal in selected_proposals:
        proposal_id = required_string(proposal, "id", "selected proposal")
        review = reviews[proposal_id]
        if review.get("outcome") != "accepted":
            continue
        discovery_id = required_string(
            proposal, "discovery_id", proposal_id
        )
        accepted_by_discovery[discovery_id] = (proposal, review)
    rows = application_manifest.get("rows")
    if not isinstance(rows, list) or not rows:
        raise ValueError("authority application manifest requires rows")
    authority_invariants_by_discovery: dict[str, dict[str, set[str]]] = {}
    for row in rows:
        if not isinstance(row, Mapping):
            raise ValueError("authority application rows must be objects")
        code_id = required_string(row, "code_id", "authority row")
        invariant_ids = set(
            string_array(row.get("invariant_ids"), f"{code_id} invariants")
        )
        if not invariant_ids:
            raise ValueError(f"{code_id} authority row requires invariants")
        discovery_ids = string_array(
            row.get("discovery_ids"), f"{code_id} discoveries"
        )
        if not discovery_ids:
            raise ValueError(f"{code_id} authority row requires discoveries")
        for discovery_id in discovery_ids:
            accepted = accepted_by_discovery.get(discovery_id)
            if accepted is None:
                raise ValueError(
                    f"{code_id} authority discovery is not an accepted "
                    f"current proposal: {discovery_id}"
                )
            proposal, review = accepted
            reviewed_invariants = set(
                string_array(
                    review.get("selected_invariant_ids"),
                    f"{discovery_id} reviewed invariants",
                )
            )
            if not invariant_ids <= reviewed_invariants:
                raise ValueError(
                    f"{code_id} authority invariants are not a subset of "
                    f"the accepted review for {discovery_id}"
                )
            covered = set(
                string_array(
                    proposal.get("covered_code_ids"),
                    f"{discovery_id} covered code IDs",
                )
            )
            if code_id not in covered:
                raise ValueError(
                    f"{code_id} is not trace-covered for accepted discovery "
                    f"{discovery_id}"
                )
            canonical = canonical_mappings.get(code_id)
            if canonical is None or canonical.get("disposition") != "spec_gap":
                raise ValueError(
                    f"{code_id} is not a canonical spec-gap code fact"
                )
            by_code = authority_invariants_by_discovery.setdefault(
                discovery_id, {}
            )
            if code_id in by_code:
                raise ValueError(
                    f"{code_id} has duplicate authority rows for "
                    f"{discovery_id}"
                )
            by_code[code_id] = invariant_ids

    for discovery_id, (proposal, review) in accepted_by_discovery.items():
        covered = set(
            string_array(
                proposal.get("covered_code_ids"),
                f"{discovery_id} covered code IDs",
            )
        )
        missing_canonical = sorted(covered - set(canonical_mappings))
        if missing_canonical:
            raise ValueError(
                f"{discovery_id} covered code IDs are absent from the "
                "canonical code map: "
                + ", ".join(missing_canonical)
            )
        authority_by_code = authority_invariants_by_discovery.get(
            discovery_id, {}
        )

        projected_invariants: set[str] = set()
        for code_id in covered:
            canonical = canonical_mappings[code_id]
            if canonical.get("disposition") == "spec_gap":
                projected_invariants.update(
                    authority_by_code.get(code_id, set())
                )
            elif canonical.get("disposition") == "specified":
                projected_invariants.update(
                    string_array(
                        canonical.get("invariant_ids"),
                        f"{code_id} canonical invariants",
                    )
                )
        reviewed_invariants = set(
            string_array(
                review.get("selected_invariant_ids"),
                f"{discovery_id} reviewed invariants",
            )
        )
        unsupported = sorted(reviewed_invariants - projected_invariants)
        if unsupported:
            raise ValueError(
                f"{discovery_id} accepted review invariants lack projected "
                "code authority: "
                + ", ".join(unsupported)
            )


def validate_projected_authority_review_support(
    *,
    application_manifest: Mapping[str, Any],
    review_inputs: Sequence[Mapping[str, Any]],
    selected_proposals: Sequence[Mapping[str, Any]],
    projected_mappings: Mapping[str, Mapping[str, Any]],
) -> None:
    """Recheck accepted review authority against the committed code map."""

    for row in application_manifest.get("rows", []):
        if not isinstance(row, Mapping):
            raise ValueError("authority application rows must be objects")
        code_id = required_string(row, "code_id", "authority row")
        projected = projected_mappings.get(code_id)
        if projected is None or projected.get("disposition") != "specified":
            raise ValueError(
                f"{code_id} is not specified in the projected code map"
            )
        row_invariants = set(
            string_array(row.get("invariant_ids"), f"{code_id} invariants")
        )
        projected_invariants = set(
            string_array(
                projected.get("invariant_ids"),
                f"{code_id} projected invariants",
            )
        )
        if projected_invariants != row_invariants:
            raise ValueError(
                f"{code_id} projected invariants differ from the authority "
                "application"
            )

    proposals_by_id = {
        required_string(proposal, "id", "selected proposal"): proposal
        for proposal in selected_proposals
    }
    for review in review_inputs:
        if review.get("outcome") != "accepted":
            continue
        proposal_id = required_string(
            review, "proposal_id", "mapping review input"
        )
        proposal = proposals_by_id.get(proposal_id)
        if proposal is None:
            raise ValueError(
                f"accepted mapping review names unknown proposal {proposal_id}"
            )
        discovery_id = required_string(
            proposal, "discovery_id", proposal_id
        )
        covered = string_array(
            proposal.get("covered_code_ids"),
            f"{discovery_id} covered code IDs",
        )
        supported: set[str] = set()
        for code_id in covered:
            projected = projected_mappings.get(code_id)
            if projected is None:
                raise ValueError(
                    f"{discovery_id} covered code is absent from the "
                    f"projected code map: {code_id}"
                )
            if projected.get("disposition") == "specified":
                supported.update(
                    string_array(
                        projected.get("invariant_ids"),
                        f"{code_id} projected invariants",
                    )
                )
        selected = set(
            string_array(
                review.get("selected_invariant_ids"),
                f"{discovery_id} reviewed invariants",
            )
        )
        unsupported = sorted(selected - supported)
        if unsupported:
            raise ValueError(
                f"{discovery_id} accepted review invariants lack committed "
                "code authority: "
                + ", ".join(unsupported)
            )


def index_code_mappings(
    document: Mapping[str, Any], label: str
) -> dict[str, Mapping[str, Any]]:
    rows = document.get("mappings")
    if not isinstance(rows, list):
        raise ValueError(f"{label} mappings must be an array")
    result: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        if not isinstance(row, Mapping):
            raise ValueError(f"{label} mapping rows must be objects")
        code_id = required_string(row, "code_id", label)
        if code_id in result:
            raise ValueError(f"{label} duplicates {code_id}")
        result[code_id] = row
    return result


def index_records(value: Any, label: str) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, list):
        raise ValueError(f"{label} collection must be an array")
    result = {}
    for record in value:
        if not isinstance(record, Mapping):
            raise ValueError(f"{label} rows must be objects")
        record_id = required_string(record, "id", label)
        if record_id in result:
            raise ValueError(f"duplicate {label} id {record_id}")
        result[record_id] = record
    return result


def forbid_broad_report_replacements(replacements: Mapping[str, bytes]) -> None:
    forbidden = (
        "unmapped-test-debt",
        "code-spec-coverage",
        "test-all",
    )
    violations = [
        path
        for path in replacements
        if any(marker in path for marker in forbidden)
    ]
    if violations:
        raise ValueError(
            "focused mapping review may not replace broad report artifacts: "
            + ", ".join(sorted(violations))
        )


def validate_schema(
    document: Mapping[str, Any], schema_path: Path, label: str
) -> None:
    schema = load_json_file(schema_path)
    failures = validate_json_schema_instance(document, schema)
    if failures:
        raise ValueError(
            f"{label} failed schema validation: " + "; ".join(failures)
        )


def initial_decision_document(source_commit: str) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "generator": "test-catalog-mapping-review",
        "generator_version": "1",
        "source_commit": source_commit,
        "proposal_snapshots": [],
        "decisions": [],
    }


def validate_source_commit(
    proposal_document: Mapping[str, Any], source_commit: str
) -> None:
    proposal_commit = proposal_document.get("source_commit")
    if (
        not isinstance(proposal_commit, str)
        or COMMIT_RE.fullmatch(proposal_commit) is None
    ):
        raise ValueError("proposal queue source_commit must be a Git commit")
    if proposal_commit != source_commit:
        raise ValueError(
            "proposal queue source_commit differs from current HEAD"
        )


def git_head(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    commit = completed.stdout.strip()
    if completed.returncode != 0 or COMMIT_RE.fullmatch(commit) is None:
        raise ValueError("focused mapping review requires a valid Git HEAD")
    return commit


def repository_file(root: Path, path: Path, label: str) -> Path:
    candidate = safe_repository_path(root, path, label)
    if not candidate.is_file() or candidate.is_symlink():
        raise ValueError(f"{label} must be a regular repository file")
    return candidate


def repository_optional_file(
    root: Path, path: Path, label: str
) -> Path | None:
    candidate = safe_repository_path(root, path, label)
    if not candidate.exists():
        return None
    if not candidate.is_file() or candidate.is_symlink():
        raise ValueError(f"{label} must be a regular repository file")
    return candidate


def repository_output(root: Path, path: Path, label: str) -> Path:
    return safe_repository_path(root, path, label)


def quarantine_file(root: Path, path: Path, label: str) -> Path:
    candidate = repository_file(root, path, label)
    relative = candidate.relative_to(root)
    if not relative.is_relative_to(QUARANTINE_ROOT):
        raise ValueError(f"{label} must live under {QUARANTINE_ROOT}")
    return candidate


def quarantine_output(root: Path, path: Path, label: str) -> Path:
    candidate = repository_output(root, path, label)
    relative = candidate.relative_to(root)
    if not relative.is_relative_to(QUARANTINE_ROOT):
        raise ValueError(f"{label} must live under {QUARANTINE_ROOT}")
    return candidate


def safe_repository_path(root: Path, path: Path, label: str) -> Path:
    candidate = path if path.is_absolute() else root / path
    candidate = candidate.absolute()
    try:
        relative = candidate.relative_to(root)
    except ValueError as error:
        raise ValueError(f"{label} escapes the repository") from error
    if any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError(f"{label} must use a normalized repository path")
    cursor = root
    for part in relative.parts:
        cursor = cursor / part
        if cursor.is_symlink():
            raise ValueError(f"{label} may not traverse a symlink")
    resolved_parent = candidate.parent.resolve(strict=True)
    if resolved_parent != root and root not in resolved_parent.parents:
        raise ValueError(f"{label} resolves outside the repository")
    return candidate


def load_toml_file(path: Path) -> dict[str, Any]:
    return load_toml_bytes(path.read_bytes(), str(path))


def load_toml_bytes(content: bytes, label: str) -> dict[str, Any]:
    document = tomllib.loads(content.decode())
    if not isinstance(document, dict):
        raise ValueError(f"{label} must contain a TOML object")
    return document


def load_json_file(path: Path) -> dict[str, Any]:
    return load_json_bytes(path.read_bytes(), str(path))


def load_json_bytes(content: bytes, label: str) -> dict[str, Any]:
    document = json.loads(content)
    if not isinstance(document, dict):
        raise ValueError(f"{label} must contain a JSON object")
    return document


def string_array(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or any(
        not isinstance(item, str) or not item for item in value
    ):
        raise ValueError(f"{label} must be a string array")
    return list(value)


def required_string(
    record: Mapping[str, Any], field: str, label: str
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def digest(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()
