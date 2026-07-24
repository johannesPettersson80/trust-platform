"""Focused Rust mapping review over the frozen test denominator.

This module owns no repository-wide scanner, denominator reconciliation, broad
metadata validation, or Cargo invocation. It refreshes only selected Rust
assertion inventories and treats unselected denominator rows as frozen.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import tomllib
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

from .code_spec_live import load_code_spec_metadata
from .code_spec_authority_contract import (
    catalog_tests_by_discovery,
    validate_discoveries_and_tests,
)
from .code_spec_authority_selection import (
    AuthoritySelectionApplication,
    build_authority_selection_application,
)
from .focused_mapping_context import (
    FocusedMappingContext,
    build_focused_mapping_context,
)
from .focused_mapping_review_support import (
    CATALOG_PATH,
    CODE_MAP_PATH,
    DECISION_PATH,
    DECISION_SCHEMA_PATH,
    DEFAULT_RECEIPT_PATH,
    DENOMINATOR_PATH,
    IGNORED_PATH,
    PROPOSAL_PATH,
    PROPOSAL_SCHEMA_PATH,
    REVIEW_SCHEMA_PATH,
    SELECTION_SCHEMA_PATH,
    build_receipt,
    capture_initial_originals,
    capture_selected_source_originals,
    captured_bytes,
    expected_authority_replacement_paths,
    git_head,
    index_records,
    index_code_mappings,
    initial_decision_document,
    load_json_bytes,
    load_json_file,
    load_toml_bytes,
    projected_invariants,
    quarantine_file,
    quarantine_output,
    require_exact_replacement_paths,
    require_unchanged_originals,
    repository_file,
    repository_optional_file,
    repository_output,
    required_string,
    string_array,
    selected_invariant_sources_from_snapshot,
    selected_proposals_from_numbered_selection,
    validate_authority_batch_scope,
    validate_projected_authority_review_support,
    validate_exact_replacements,
    validate_focused_post_state,
    validate_read_only_inputs,
    validate_schema,
    validate_source_commit,
)
from .mapping_assertions import (
    validate_assertion_batch,
    validate_proposal_assertion_inventories,
)
from .mapping_review_batch import MappingReviewBatch, build_review_batch
from .mapping_review_selection import build_review_input_from_selection
from .mapping_transaction import apply_mapping_transaction
from .test_catalog_denominator import DENOMINATOR_SCHEMA_PATH


DATE_RE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$")
AssertionInventoryLoader = Callable[
    [Path, Sequence[Mapping[str, Any]]],
    dict[str, dict[str, Any]],
]


@dataclass(frozen=True)
class AuthorityClosureRequest:
    """Fully reviewed overlay handed to the authoritative code-map owner."""

    root: Path
    reviewed_date: str
    authority_selection_path: Path
    proposal_document: Mapping[str, Any]
    review_inputs: tuple[Mapping[str, Any], ...]
    selected_proposals: tuple[Mapping[str, Any], ...]
    canonical_mappings: Mapping[str, Mapping[str, Any]]
    review_batch: MappingReviewBatch
    projected_catalog: Mapping[str, Any]
    projected_denominator: Mapping[str, Any]
    projected_invariants: Mapping[str, Mapping[str, Any]]


@dataclass(frozen=True)
class AuthorityClosurePlan:
    """Already-validated replacements returned by the authority owner."""

    replacements: Mapping[str, bytes]
    expected_originals: Mapping[str, bytes | None]
    validate_after_replace: Callable[[Path], Sequence[str]]
    receipt_fields: Mapping[str, Any]
    application_manifest: Mapping[str, Any]


class AuthorityClosureHook(Protocol):
    def __call__(
        self, request: AuthorityClosureRequest
    ) -> AuthorityClosurePlan: ...


@dataclass(frozen=True)
class FocusedMappingReviewResult:
    """One committed focused semantic review transaction."""

    batch: MappingReviewBatch
    receipt: Mapping[str, Any]
    replacements: Mapping[str, bytes]


def execute_focused_review_batch(
    *,
    root: Path,
    review_selection_path: Path,
    reviewed_date: str,
    receipt_path: Path = DEFAULT_RECEIPT_PATH,
    authority_selection_path: Path | None = None,
    authority_closure_hook: AuthorityClosureHook | None = None,
    assertion_inventory_loader: AssertionInventoryLoader | None = None,
) -> FocusedMappingReviewResult:
    """Review one selected Rust batch without a full repository test scan."""

    root = root.resolve()
    if DATE_RE.fullmatch(reviewed_date) is None:
        raise ValueError("focused mapping review date must be YYYY-MM-DD")
    selection_file = quarantine_file(
        root, review_selection_path, "mapping review selection"
    )
    receipt_file = quarantine_output(
        root, receipt_path, "focused mapping review receipt"
    )
    canonical_files = {
        path: repository_file(root, root / path, path.as_posix())
        for path in (
            PROPOSAL_PATH,
            CATALOG_PATH,
            CODE_MAP_PATH,
            DENOMINATOR_PATH,
            IGNORED_PATH,
            SELECTION_SCHEMA_PATH,
            REVIEW_SCHEMA_PATH,
            PROPOSAL_SCHEMA_PATH,
            DECISION_SCHEMA_PATH,
            Path(DENOMINATOR_SCHEMA_PATH),
        )
    }
    decision_file = repository_optional_file(
        root, root / DECISION_PATH, DECISION_PATH.as_posix()
    )
    authority_file = (
        quarantine_file(root, authority_selection_path, "authority selection")
        if authority_selection_path is not None
        else None
    )
    original_bytes = capture_initial_originals(
        root=root,
        canonical_files=canonical_files,
        decision_file=decision_file,
        selection_file=selection_file,
        receipt_file=receipt_file,
        authority_file=authority_file,
    )
    source_commit = git_head(root)
    proposal_bytes = captured_bytes(
        root, original_bytes, canonical_files[PROPOSAL_PATH]
    )
    proposal_document = load_toml_bytes(
        proposal_bytes, PROPOSAL_PATH.as_posix()
    )
    validate_source_commit(proposal_document, source_commit)
    validate_schema(
        proposal_document,
        canonical_files[PROPOSAL_SCHEMA_PATH],
        "mapping proposal queue",
    )
    decision_bytes = (
        captured_bytes(root, original_bytes, decision_file)
        if decision_file is not None
        else None
    )
    decision_document = (
        load_toml_bytes(decision_bytes, DECISION_PATH.as_posix())
        if decision_bytes is not None
        else initial_decision_document(source_commit)
    )
    validate_schema(
        decision_document,
        canonical_files[DECISION_SCHEMA_PATH],
        "mapping decision ledger",
    )
    selection_bytes = captured_bytes(root, original_bytes, selection_file)
    selection_document = load_json_bytes(
        selection_bytes, "mapping review selection"
    )
    validate_schema(
        selection_document,
        canonical_files[SELECTION_SCHEMA_PATH],
        "mapping review selection",
    )
    mechanically_selected_proposals = (
        selected_proposals_from_numbered_selection(
            proposal_document=proposal_document,
            selection_document=selection_document,
        )
    )
    selected_source_originals = capture_selected_source_originals(
        root, mechanically_selected_proposals
    )
    _merge_preconditions(original_bytes, selected_source_originals)

    metadata = load_code_spec_metadata(root)
    require_unchanged_originals(root, original_bytes)
    canonical_mappings = index_code_mappings(
        load_toml_bytes(
            captured_bytes(
                root, original_bytes, canonical_files[CODE_MAP_PATH]
            ),
            CODE_MAP_PATH.as_posix(),
        ),
        "frozen code-spec map",
    )
    review_document = build_review_input_from_selection(
        proposal_document=proposal_document,
        selection_document=selection_document,
        invariants=metadata.invariants,
    )
    validate_schema(
        review_document,
        canonical_files[REVIEW_SCHEMA_PATH],
        "generated mapping review input",
    )
    review_inputs = _review_inputs(review_document)
    proposals = index_records(proposal_document.get("proposals"), "proposal")
    selected_proposals = _selected_proposals(review_inputs, proposals)
    if {
        required_string(proposal, "id", "selected proposal")
        for proposal in selected_proposals
    } != {
        required_string(proposal, "id", "numbered selected proposal")
        for proposal in mechanically_selected_proposals
    }:
        raise ValueError(
            "generated review inputs differ from the numbered source snapshot"
        )
    selected_discovery_ids = [
        required_string(proposal, "discovery_id", "selected proposal")
        for proposal in selected_proposals
    ]
    context = build_focused_mapping_context(
        root=root,
        discovery_ids=selected_discovery_ids,
        tests=metadata.tests,
    )
    frozen_denominator = load_toml_bytes(
        captured_bytes(
            root, original_bytes, canonical_files[DENOMINATOR_PATH]
        ),
        DENOMINATOR_PATH.as_posix(),
    )
    if context.denominator != frozen_denominator:
        raise ValueError(
            "focused denominator differs from the initial byte snapshot"
        )
    _validate_selected_proposal_facts(selected_proposals, context)

    loader = assertion_inventory_loader or _prebuilt_rust_assertion_inventories
    fresh_inventories = loader(
        root,
        [
            context.selected_facts[discovery_id]
            for discovery_id in selected_discovery_ids
        ],
    )
    assertion_failures = validate_proposal_assertion_inventories(
        proposals=selected_proposals,
        inventories=fresh_inventories,
    )
    if assertion_failures:
        raise ValueError("; ".join(assertion_failures))

    invariant_sources = selected_invariant_sources_from_snapshot(
        originals=original_bytes,
        invariant_ids=_accepted_invariant_ids(review_inputs),
    )
    catalog_bytes = captured_bytes(
        root, original_bytes, canonical_files[CATALOG_PATH]
    )
    ignored_bytes = captured_bytes(
        root, original_bytes, canonical_files[IGNORED_PATH]
    )
    ignored_document = load_toml_bytes(
        ignored_bytes, IGNORED_PATH.as_posix()
    )
    ignored_tests = ignored_document.get("ignored_tests")
    if (
        not isinstance(ignored_tests, list)
        or any(not isinstance(record, Mapping) for record in ignored_tests)
    ):
        raise ValueError("ignored-test registry must contain object rows")

    batch = build_review_batch(
        proposal_document=proposal_document,
        decision_document=decision_document,
        review_inputs=review_inputs,
        catalog_source=catalog_bytes.decode(),
        denominator_document=context.denominator,
        invariant_sources=invariant_sources,
        invariants=metadata.invariants,
        reviewed_date=reviewed_date,
        source_commit=source_commit,
    )
    replacements = dict(batch.replacements)
    base_allowed_paths = {
        CATALOG_PATH.as_posix(),
        DENOMINATOR_PATH.as_posix(),
        PROPOSAL_PATH.as_posix(),
        DECISION_PATH.as_posix(),
        *(path for path, _ in invariant_sources.values()),
    }
    require_exact_replacement_paths(
        replacements, allowed=base_allowed_paths, label="focused review batch"
    )
    projected_catalog = load_toml_bytes(
        replacements[CATALOG_PATH.as_posix()], "projected test catalog"
    )
    projected_denominator = load_toml_bytes(
        replacements[DENOMINATOR_PATH.as_posix()], "projected denominator"
    )
    projected_invariant_documents = projected_invariants(
        metadata=metadata,
        replacements=replacements,
        invariant_sources=invariant_sources,
    )

    authority_plan, authority_file = _build_authority_plan(
        root=root,
        authority_file=authority_file,
        authority_closure_hook=authority_closure_hook,
        reviewed_date=reviewed_date,
        proposal_document=proposal_document,
        review_inputs=review_inputs,
        selected_proposals=selected_proposals,
        canonical_mappings=canonical_mappings,
        batch=batch,
        projected_catalog=projected_catalog,
        projected_denominator=projected_denominator,
        projected_invariants=projected_invariant_documents,
    )
    if authority_plan is not None:
        _validate_authority_plan_paths(root, authority_plan)
        validate_authority_batch_scope(
            application_manifest=authority_plan.application_manifest,
            review_inputs=review_inputs,
            selected_proposals=selected_proposals,
            canonical_mappings=canonical_mappings,
        )
        expected_authority_paths = expected_authority_replacement_paths(
            root=root,
            authority_file=authority_file,
        )
        require_exact_replacement_paths(
            authority_plan.replacements,
            allowed=expected_authority_paths,
            label="authority closure",
        )
        _merge_replacements(replacements, authority_plan.replacements)
    else:
        expected_authority_paths = set()
    receipt_relative = receipt_file.relative_to(root).as_posix()
    require_exact_replacement_paths(
        replacements,
        allowed=base_allowed_paths | expected_authority_paths,
        label="combined focused review",
    )

    expected_originals = dict(original_bytes)
    if authority_plan is not None:
        _merge_preconditions(
            expected_originals, authority_plan.expected_originals
        )

    receipt = build_receipt(
        source_commit=source_commit,
        reviewed_date=reviewed_date,
        proposal_bytes=proposal_bytes,
        selection_bytes=selection_bytes,
        context=context,
        batch=batch,
        projected_denominator=projected_denominator,
        replacements=replacements,
        authority_fields=(
            authority_plan.receipt_fields
            if authority_plan is not None
            else {}
        ),
    )
    if receipt_relative in replacements:
        raise ValueError("focused mapping receipt collides with a batch output")
    replacements[receipt_relative] = (
        json.dumps(receipt, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode()
    require_exact_replacement_paths(
        replacements,
        allowed=base_allowed_paths
        | expected_authority_paths
        | {receipt_relative},
        label="focused review transaction",
    )
    expected_read_only = {
        path: content
        for path, content in expected_originals.items()
        if path not in replacements
    }

    def validate_after_replace() -> list[str]:
        failures = validate_exact_replacements(root, replacements)
        failures.extend(validate_read_only_inputs(root, expected_read_only))
        failures.extend(
            validate_focused_post_state(
                root=root,
                context=context,
                selected_proposals=selected_proposals,
                review_inputs=review_inputs,
                batch=batch,
                ignored_tests=ignored_tests,
                denominator_schema=load_json_bytes(
                    captured_bytes(
                        root,
                        original_bytes,
                        canonical_files[Path(DENOMINATOR_SCHEMA_PATH)],
                    ),
                    DENOMINATOR_SCHEMA_PATH,
                ),
            )
        )
        if authority_plan is not None:
            failures.extend(authority_plan.validate_after_replace(root))
        return sorted(set(failures))

    apply_mapping_transaction(
        root=root,
        replacements=replacements,
        expected_originals=expected_originals,
        validate=validate_after_replace,
    )
    return FocusedMappingReviewResult(
        batch=batch,
        receipt=receipt,
        replacements=dict(sorted(replacements.items())),
    )


def _build_authority_plan(
    *,
    root: Path,
    authority_file: Path | None,
    authority_closure_hook: AuthorityClosureHook | None,
    reviewed_date: str,
    proposal_document: Mapping[str, Any],
    review_inputs: Sequence[Mapping[str, Any]],
    selected_proposals: Sequence[Mapping[str, Any]],
    canonical_mappings: Mapping[str, Mapping[str, Any]],
    batch: MappingReviewBatch,
    projected_catalog: Mapping[str, Any],
    projected_denominator: Mapping[str, Any],
    projected_invariants: Mapping[str, Mapping[str, Any]],
) -> tuple[AuthorityClosurePlan | None, Path | None]:
    if authority_file is None:
        if authority_closure_hook is not None:
            raise ValueError(
                "authority closure hook requires an authority selection path"
            )
        return None, None
    if authority_closure_hook is None:
        authority_closure_hook = build_authority_closure_plan
    return (
        authority_closure_hook(
            AuthorityClosureRequest(
                root=root,
                reviewed_date=reviewed_date,
                authority_selection_path=authority_file,
                proposal_document=proposal_document,
                review_inputs=tuple(review_inputs),
                selected_proposals=tuple(selected_proposals),
                canonical_mappings=canonical_mappings,
                review_batch=batch,
                projected_catalog=projected_catalog,
                projected_denominator=projected_denominator,
                projected_invariants=projected_invariants,
            )
        ),
        authority_file,
    )


def build_authority_closure_plan(
    request: AuthorityClosureRequest,
) -> AuthorityClosurePlan:
    """Build and close one existing-authority applicator projection."""

    output_path = request.authority_selection_path.with_name(
        request.authority_selection_path.stem + ".application.json"
    )
    application = build_authority_selection_application(
        root=request.root,
        selection_path=request.authority_selection_path,
        output_path=output_path,
    )
    if application.manifest.get("reviewed_date") != request.reviewed_date:
        raise ValueError(
            "authority selection reviewed_date differs from mapping review"
        )
    validate_authority_batch_scope(
        application_manifest=application.manifest,
        review_inputs=request.review_inputs,
        selected_proposals=request.selected_proposals,
        canonical_mappings=request.canonical_mappings,
    )
    _validate_authority_projected_closure(
        application=application,
        catalog=request.projected_catalog,
        denominator=request.projected_denominator,
        invariants=request.projected_invariants,
    )
    expected_originals = dict(application.expected_originals)

    def validate_after_replace(root: Path) -> list[str]:
        try:
            catalog = tomllib.loads(
                (root / CATALOG_PATH).read_text(encoding="utf-8")
            )
            denominator = tomllib.loads(
                (root / DENOMINATOR_PATH).read_text(encoding="utf-8")
            )
            invariants = {
                str(record["id"]): record
                for path in sorted(
                    (root / "verification/invariants").rglob("*.toml")
                )
                for record in [
                    tomllib.loads(path.read_text(encoding="utf-8"))
                ]
            }
            _validate_authority_projected_closure(
                application=application,
                catalog=catalog,
                denominator=denominator,
                invariants=invariants,
            )
            projected_mappings = index_code_mappings(
                tomllib.loads(
                    (root / CODE_MAP_PATH).read_text(encoding="utf-8")
                ),
                "projected code-spec map",
            )
            validate_projected_authority_review_support(
                application_manifest=application.manifest,
                review_inputs=request.review_inputs,
                selected_proposals=request.selected_proposals,
                projected_mappings=projected_mappings,
            )
        except (KeyError, OSError, ValueError, tomllib.TOMLDecodeError) as error:
            return [f"authority closure post-state failed: {error}"]
        return []

    return AuthorityClosurePlan(
        replacements=application.replacements,
        expected_originals=expected_originals,
        validate_after_replace=validate_after_replace,
        receipt_fields={
            "kind": application.manifest["kind"],
            "row_count": application.manifest["row_count"],
            "selection_sha256": application.manifest["selection_sha256"],
            "code_map_before_sha256": application.manifest[
                "code_map_before_sha256"
            ],
            "code_map_after_sha256": application.manifest[
                "code_map_after_sha256"
            ],
            "application_manifest_path": output_path.relative_to(
                request.root
            ).as_posix(),
            "projected_code_map_path": application.manifest[
                "projected_code_map_path"
            ],
            "standalone_apply_ready_before_batch": application.manifest[
                "canonical_apply_ready"
            ],
            "combined_closure_ready": True,
        },
        application_manifest=application.manifest,
    )


def _validate_authority_projected_closure(
    *,
    application: AuthoritySelectionApplication,
    catalog: Mapping[str, Any],
    denominator: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
) -> None:
    tests = index_records(catalog.get("tests"), "projected catalog test")
    reviews = _index_by_field(
        denominator.get("reviews"),
        field="discovery_id",
        label="projected denominator review",
    )
    tests_by_discovery = catalog_tests_by_discovery(tests.values())
    failures = []
    for row in application.manifest["rows"]:
        invariant_ids = string_array(
            row.get("invariant_ids"), "authority invariant IDs"
        )
        discovery_ids = string_array(
            row.get("discovery_ids"), "authority discovery IDs"
        )
        test_ids = string_array(row.get("test_ids"), "authority test IDs")
        selected_invariants = {
            invariant_id: invariants[invariant_id]
            for invariant_id in invariant_ids
            if invariant_id in invariants
        }
        if len(selected_invariants) != len(invariant_ids):
            failures.append(
                f"{row['code_id']} projected invariants are incomplete"
            )
            continue
        missing_tests, missing_backlinks = validate_discoveries_and_tests(
            code_id=str(row["code_id"]),
            discovery_ids=discovery_ids,
            test_ids=test_ids,
            denominator_reviews=reviews,
            tests=tests,
            tests_by_discovery=tests_by_discovery,
            invariant_ids=invariant_ids,
            invariants=selected_invariants,
            area=str(
                next(iter(selected_invariants.values())).get("area", "")
            ),
            spec_ref=str(row["spec_ref"]),
        )
        failures.extend(
            f"{row['code_id']} missing projected catalog test {test_id}"
            for test_id in missing_tests
        )
        failures.extend(
            f"{row['code_id']} missing projected invariant backlink {backlink}"
            for backlink in missing_backlinks
        )
    if failures:
        raise ValueError("; ".join(sorted(set(failures))))


def _index_by_field(
    value: Any, *, field: str, label: str
) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, list):
        raise ValueError(f"{label} rows must be an array")
    result = {}
    for row in value:
        if not isinstance(row, Mapping):
            raise ValueError(f"{label} rows must be objects")
        key = required_string(row, field, label)
        if key in result:
            raise ValueError(f"{label} duplicates {key}")
        result[key] = row
    return result


def _prebuilt_rust_assertion_inventories(
    root: Path,
    facts: Sequence[Mapping[str, Any]],
) -> dict[str, dict[str, Any]]:
    """Run the already-built focused Rust extractor without invoking Cargo."""

    configured = os.environ.get("TRUST_XTASK_BIN")
    binary = (
        Path(configured) if configured else root / "target/debug/xtask"
    )
    binary = repository_file(root, binary, "prebuilt xtask")
    if not os.access(binary, os.X_OK):
        raise ValueError(f"prebuilt xtask is not executable: {binary}")
    requested = [
        {
            "discovery_id": required_string(
                fact, "discovery_id", "selected Rust fact"
            ),
            "path": required_string(fact, "path", "selected Rust fact"),
            "name": required_string(fact, "name", "selected Rust fact"),
        }
        for fact in facts
    ]
    request = {
        "schema_version": 1,
        "tests": [
            {"path": row["path"], "name": row["name"]}
            for row in sorted(
                requested,
                key=lambda row: (
                    row["path"],
                    row["name"],
                    row["discovery_id"],
                ),
            )
        ],
    }
    completed = subprocess.run(
        [str(binary), "test-assertions-batch"],
        cwd=root,
        input=json.dumps(request, sort_keys=True),
        capture_output=True,
        text=True,
        check=False,
        env=dict(os.environ),
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise ValueError(f"prebuilt Rust assertion extraction failed: {detail}")
    try:
        document = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise ValueError(
            "prebuilt Rust assertion extractor emitted invalid JSON"
        ) from error
    return validate_assertion_batch(root, document, requested)


def _accepted_invariant_ids(
    reviews: Sequence[Mapping[str, Any]],
) -> list[str]:
    return sorted(
        {
            invariant_id
            for review in reviews
            if review.get("outcome") == "accepted"
            for invariant_id in string_array(
                review.get("selected_invariant_ids"),
                "selected invariant IDs",
            )
        }
    )


def _selected_proposals(
    review_inputs: Sequence[Mapping[str, Any]],
    proposals: Mapping[str, Mapping[str, Any]],
) -> list[Mapping[str, Any]]:
    selected = []
    seen: set[str] = set()
    for review in review_inputs:
        proposal_id = required_string(
            review, "proposal_id", "mapping review input"
        )
        if proposal_id in seen:
            raise ValueError(f"mapping review duplicates {proposal_id}")
        seen.add(proposal_id)
        proposal = proposals.get(proposal_id)
        if proposal is None:
            raise ValueError(
                f"generated review input names unknown pending {proposal_id}"
            )
        selected.append(proposal)
    return selected


def _validate_selected_proposal_facts(
    proposals: Sequence[Mapping[str, Any]],
    context: FocusedMappingContext,
) -> None:
    for proposal in proposals:
        discovery_id = required_string(
            proposal, "discovery_id", "selected proposal"
        )
        fact = context.selected_facts.get(discovery_id)
        if fact is None:
            raise ValueError(
                "selected proposal is absent from focused live facts: "
                f"{discovery_id}"
            )
        for field in (
            "discovery_id",
            "discovery_source_kind",
            "path",
            "name",
            "ignore_state",
        ):
            if proposal.get(field) != fact.get(field):
                raise ValueError(f"{discovery_id} proposal {field} is stale")


def _review_inputs(
    document: Mapping[str, Any],
) -> list[Mapping[str, Any]]:
    if set(document) != {"schema_version", "reviews"}:
        raise ValueError("review input root fields drift")
    if document.get("schema_version") != 1:
        raise ValueError("review input must use schema_version 1")
    reviews = document.get("reviews")
    if (
        not isinstance(reviews, Sequence)
        or isinstance(reviews, (str, bytes))
        or not reviews
        or any(not isinstance(review, Mapping) for review in reviews)
    ):
        raise ValueError("review input requires non-empty review object array")
    return list(reviews)


def _validate_authority_plan_paths(
    root: Path, plan: AuthorityClosurePlan
) -> None:
    for relative in set(plan.replacements) | set(plan.expected_originals):
        repository_output(root, root / relative, f"authority path {relative}")


def _merge_replacements(
    target: dict[str, bytes], additions: Mapping[str, bytes]
) -> None:
    for path, content in additions.items():
        previous = target.get(path)
        if previous is not None and previous != content:
            raise ValueError(f"authority replacement collides at {path}")
        target[path] = content


def _merge_preconditions(
    target: dict[str, bytes | None],
    additions: Mapping[str, bytes | None],
) -> None:
    for path, content in additions.items():
        if path in target and target[path] != content:
            raise ValueError(f"transaction precondition collides at {path}")
        target[path] = content
