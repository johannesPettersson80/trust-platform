"""Frozen-denominator inputs for focused mapping candidate generation."""

from __future__ import annotations

import hashlib
import json
import re
import tomllib
import tempfile
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, replace
from pathlib import Path
from typing import Any

from .code_spec_authority_selection import (
    AuthoritySelectionApplication,
    build_authority_selection_application,
)
from .code_spec_live import CodeSpecMetadata
from .coverage_trace import validate_trace_document
from .coverage_trace_inputs import trace_input_digest
from .coverage_trace_resolver import (
    mechanical_resolved_rust_command,
    mechanical_rust_target,
)
from .focused_test_facts import resolve_focused_rust_facts
from .focused_test_facts import focused_rust_input_digest
from .test_catalog_denominator import (
    DENOMINATOR_SCHEMA_PATH,
    analyze_test_catalog_denominator,
    validate_test_catalog_denominator_document,
)
from .test_catalog_models import InferredTestFact


DISCOVERY_ID_RE = re.compile(r"^DISC_[A-F0-9]{20}$")
QUARANTINE_ROOT = "target/gate-artifacts"
CODE_MAP_PATH = "verification/code-spec-map.toml"


@dataclass(frozen=True)
class FocusedMappingContext:
    """Validated selected facts plus the complete frozen denominator."""

    denominator: Mapping[str, Any]
    reviews_by_id: Mapping[str, Mapping[str, Any]]
    selected_facts: Mapping[str, Mapping[str, Any]]
    selected_live_facts: tuple[InferredTestFact, ...]
    frozen_facts: tuple[InferredTestFact, ...]
    source_input_digest: str | None = None


@dataclass(frozen=True)
class AuthenticatedAuthorityProjection:
    """Rebuilt applicator output plus immutable input preconditions."""

    metadata: CodeSpecMetadata
    application: AuthoritySelectionApplication
    expected_originals: Mapping[str, bytes | None]


def build_focused_mapping_context(
    *,
    root: Path,
    discovery_ids: Sequence[str],
    tests: Mapping[str, Mapping[str, Any]],
    denominator_document: Mapping[str, Any] | None = None,
    denominator_schema_document: Mapping[str, Any] | None = None,
    ignored_document: Mapping[str, Any] | None = None,
    source_input_preconditions: Mapping[str, bytes] | None = None,
    source_root: Path | None = None,
) -> FocusedMappingContext:
    """Validate the frozen census and rescan only requested Rust test files."""

    root = root.resolve()
    requested = sorted(discovery_ids)
    if (
        not requested
        or len(requested) != len(set(requested))
        or any(DISCOVERY_ID_RE.fullmatch(value) is None for value in requested)
    ):
        raise ValueError(
            "focused mapping discovery IDs must be unique reviewed DISC IDs"
        )
    denominator = (
        dict(denominator_document)
        if denominator_document is not None
        else tomllib.loads(
            canonical_input_file(
                root,
                root / "verification/test-catalog-denominator.toml",
                "test-catalog denominator",
            ).read_text()
        )
    )
    schema = (
        dict(denominator_schema_document)
        if denominator_schema_document is not None
        else _load_json(
            canonical_input_file(
                root,
                root / DENOMINATOR_SCHEMA_PATH,
                "test-catalog denominator schema",
            )
        )
    )
    failures = validate_test_catalog_denominator_document(
        denominator,
        schema=schema,
    )
    if failures:
        raise ValueError("; ".join(failures))
    frozen_facts = _facts_from_denominator(denominator)
    if source_input_preconditions is not None and source_root is not None:
        raise ValueError("focused mapping accepts one frozen source input route")
    if source_root is not None:
        selected_live = resolve_focused_rust_facts(
            root=source_root,
            discovery_ids=requested,
            denominator=denominator,
        )
        selected_source_digest = focused_rust_input_digest(
            root=source_root,
            facts=selected_live,
            denominator=denominator,
        )
    elif source_input_preconditions is None:
        selected_live = resolve_focused_rust_facts(
            root=root,
            discovery_ids=requested,
            denominator=denominator,
        )
        selected_source_digest = None
    else:
        with tempfile.TemporaryDirectory() as temporary:
            snapshot_root = Path(temporary)
            for relative, content in source_input_preconditions.items():
                target = snapshot_root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(content)
            selected_live = resolve_focused_rust_facts(
                root=snapshot_root,
                discovery_ids=requested,
                denominator=denominator,
            )
            selected_source_digest = focused_rust_input_digest(
                root=snapshot_root,
                facts=selected_live,
                denominator=denominator,
            )
    selected_by_id = {fact.stable_id: fact for fact in selected_live}
    if set(selected_by_id) != set(requested):
        raise ValueError("focused Rust resolver did not return the exact request")
    validated_facts = tuple(
        selected_by_id.get(fact.stable_id, fact) for fact in frozen_facts
    )
    ignored_document = (
        dict(ignored_document)
        if ignored_document is not None
        else tomllib.loads(
            canonical_input_file(
                root,
                root / "verification/ignored-tests.toml",
                "ignored-test registry",
            ).read_text()
        )
    )
    ignored_tests = ignored_document.get("ignored_tests")
    if (
        not isinstance(ignored_tests, list)
        or any(not isinstance(record, Mapping) for record in ignored_tests)
    ):
        raise ValueError("ignored-test registry must contain object rows")
    analyze_test_catalog_denominator(
        facts=validated_facts,
        tests=list(tests.values()),
        reviews=denominator["reviews"],
        ignored_tests=ignored_tests,
    )
    reviews_by_id = {
        str(review["discovery_id"]): review
        for review in denominator["reviews"]
    }
    return FocusedMappingContext(
        denominator=denominator,
        reviews_by_id=reviews_by_id,
        selected_facts={
            discovery_id: _proposal_fact(selected_by_id[discovery_id])
            for discovery_id in requested
        },
        selected_live_facts=tuple(
            selected_by_id[discovery_id] for discovery_id in requested
        ),
        frozen_facts=validated_facts,
        source_input_digest=selected_source_digest,
    )


def validate_focused_rust_trace(
    *,
    root: Path,
    trace: Mapping[str, Any],
    context: FocusedMappingContext,
    metadata: CodeSpecMetadata,
) -> None:
    """Reject semantic or input drift in one focused Rust execution trace."""

    failures = validate_trace_document(
        trace,
        live_facts=context.selected_facts,
    )
    scope = trace.get("scope")
    expected_ids = sorted(context.selected_facts)
    expected_kinds = sorted(
        {
            str(fact["discovery_source_kind"])
            for fact in context.selected_facts.values()
        }
    )
    if not isinstance(scope, Mapping) or scope.get("mode") != "focused_shard":
        failures.append("focused Rust mapping requires focused_shard trace scope")
    else:
        if scope.get("requested_discovery_ids") != expected_ids:
            failures.append("focused Rust trace scope discovery IDs drift")
        if scope.get("source_kinds") != expected_kinds:
            failures.append("focused Rust trace scope source kinds drift")
    if any(
        kind not in {"rust_unit_test", "rust_integration_test"}
        for kind in expected_kinds
    ):
        failures.append("focused Rust mapping forbids non-Rust source kinds")
    records = trace.get("tests")
    if isinstance(records, list):
        for record in records:
            if (
                isinstance(record, Mapping)
                and record.get("trace_status") != "observed"
            ):
                failures.append(
                    f"{record.get('discovery_id')} lacks observed execution proof"
                )
    provenance = trace.get("input_provenance")
    if not isinstance(provenance, Mapping):
        failures.append("focused Rust trace lacks reproducible input provenance")
    else:
        failures.extend(
            _focused_trace_provenance_failures(
                root=root,
                trace=trace,
                provenance=provenance,
                context=context,
                metadata=metadata,
            )
        )
    if failures:
        raise ValueError("; ".join(sorted(set(failures))))


def authenticate_projected_code_spec_authority(
    *,
    root: Path,
    metadata: CodeSpecMetadata,
    plan_path: Path,
) -> AuthenticatedAuthorityProjection:
    """Accept only an exact projection rebuilt by the authority applicator."""

    root = root.resolve()
    resolved = canonical_input_file(
        root,
        plan_path,
        "authority projection manifest",
        required_parent=QUARANTINE_ROOT,
    )
    relative = resolved.relative_to(root).as_posix()
    document = _load_json(resolved)
    if document.get("kind") != "reviewed_code_spec_authority_selection":
        raise ValueError("authority input is not an applicator projection manifest")
    selection_relative = document.get("selection_path")
    if not isinstance(selection_relative, str) or not selection_relative:
        raise ValueError("authority projection manifest lacks selection_path")
    selection_path = canonical_input_file(
        root,
        root / selection_relative,
        "compiled authority selection",
        required_parent=QUARANTINE_ROOT,
    )
    application = build_authority_selection_application(
        root=root,
        selection_path=selection_path,
        output_path=resolved,
    )
    if dict(document) != application.manifest:
        raise ValueError("authority projection manifest differs from rebuilt plan")
    expected_manifest = application.quarantine_replacements.get(relative)
    if expected_manifest is None or resolved.read_bytes() != expected_manifest:
        raise ValueError("authority projection manifest bytes are not canonical")
    projected_relative = application.manifest["projected_code_map_path"]
    projected_path = canonical_input_file(
        root,
        root / str(projected_relative),
        "projected code-spec map",
        required_parent=QUARANTINE_ROOT,
    )
    if projected_path.read_bytes() != application.projected_code_map_bytes:
        raise ValueError("projected code-spec map differs from rebuilt bytes")
    projected_document = tomllib.loads(
        application.projected_code_map_bytes.decode()
    )
    projected_rows = projected_document.get("mappings")
    if not isinstance(projected_rows, list) or any(
        not isinstance(row, Mapping) for row in projected_rows
    ):
        raise ValueError("projected code-spec map mappings are malformed")
    projected_mappings = {
        _required_string(row, "code_id", "projected mapping"): dict(row)
        for row in projected_rows
    }
    if len(projected_mappings) != len(projected_rows):
        raise ValueError("projected code-spec map duplicates code IDs")
    projected_metadata = replace(
        metadata,
        code_map=projected_document,
        mappings=dict(sorted(projected_mappings.items())),
    )
    expected_originals = dict(application.expected_originals)
    required_originals = {
        CODE_MAP_PATH: application.code_map_before_bytes,
        relative: resolved.read_bytes(),
        str(projected_relative): projected_path.read_bytes(),
        selection_path.relative_to(root).as_posix(): selection_path.read_bytes(),
    }
    for input_path, expected in required_originals.items():
        if expected_originals.get(input_path) != expected:
            raise ValueError(
                "authority application lacks exact authenticated input "
                f"precondition for {input_path}"
            )
    return AuthenticatedAuthorityProjection(
        metadata=projected_metadata,
        application=application,
        expected_originals=expected_originals,
    )


def _facts_from_denominator(
    denominator: Mapping[str, Any],
) -> tuple[InferredTestFact, ...]:
    reviews = denominator.get("reviews")
    if not isinstance(reviews, list):
        raise ValueError("test-catalog denominator reviews are malformed")
    facts: list[InferredTestFact] = []
    seen: set[str] = set()
    for review in reviews:
        if not isinstance(review, Mapping):
            raise ValueError("test-catalog denominator review is not a table")
        discovery_id = _required_string(
            review, "discovery_id", "denominator review"
        )
        if discovery_id in seen:
            raise ValueError(f"test-catalog denominator duplicates {discovery_id}")
        seen.add(discovery_id)
        facts.append(
            InferredTestFact(
                stable_id=discovery_id,
                native_id=f"frozen:{discovery_id}",
                source_kind=_required_string(
                    review, "discovery_source_kind", discovery_id
                ),
                name=_required_string(review, "name", discovery_id),
                path=_required_string(review, "path", discovery_id),
                line=0,
                package=None,
                command_hint="",
                command_hint_authority="frozen_denominator",
                discovery_confidence="reviewed",
                ignore_state=_required_string(
                    review, "ignore_state", discovery_id
                ),
                ignore_reason=None,
                reference_candidates=(),
                provenance="reviewed_denominator",
            )
        )
    return tuple(facts)


def _proposal_fact(fact: InferredTestFact) -> dict[str, Any]:
    return {
        "discovery_id": fact.stable_id,
        "discovery_source_kind": fact.source_kind,
        "path": fact.path,
        "name": fact.name,
        "ignore_state": fact.ignore_state,
    }


def _focused_trace_provenance_failures(
    *,
    root: Path,
    trace: Mapping[str, Any],
    provenance: Mapping[str, Any],
    context: FocusedMappingContext,
    metadata: CodeSpecMetadata,
) -> list[str]:
    failures: list[str] = []
    if provenance.get("kind") != "focused_rust_v1":
        failures.append("focused Rust trace input provenance kind drift")
    resolved = provenance.get("resolved")
    if not isinstance(resolved, list) or any(
        not isinstance(row, Mapping) for row in resolved
    ):
        return [*failures, "focused Rust trace resolved inputs are malformed"]
    resolved_by_id = {
        str(row.get("discovery_id")): row
        for row in resolved
        if isinstance(row.get("discovery_id"), str)
    }
    if len(resolved_by_id) != len(resolved) or sorted(resolved_by_id) != sorted(
        context.selected_facts
    ):
        failures.append("focused Rust trace resolved discovery set drift")
    records = trace.get("tests")
    records_by_id = {
        str(row.get("discovery_id")): row
        for row in records
        if isinstance(records, list)
        and isinstance(row, Mapping)
        and isinstance(row.get("discovery_id"), str)
    }
    live_by_id = {
        fact.stable_id: fact for fact in context.selected_live_facts
    }
    for discovery_id, fact in context.selected_facts.items():
        resolved_row = resolved_by_id.get(discovery_id)
        record = records_by_id.get(discovery_id)
        if resolved_row is None or record is None:
            continue
        for field in ("discovery_source_kind", "path", "name"):
            if resolved_row.get(field) != fact.get(field):
                failures.append(
                    f"{discovery_id} resolved {field} does not match live fact"
                )
        for field in (
            "discovery_source_kind",
            "path",
            "name",
            "command",
            "command_authority",
        ):
            if resolved_row.get(field) != record.get(field):
                failures.append(
                    f"{discovery_id} resolved {field} does not match trace record"
                )
        live = live_by_id.get(discovery_id)
        if live is None or not isinstance(live.package, str) or not live.package:
            failures.append(
                f"{discovery_id} lacks live Rust package authority"
            )
            continue
        if resolved_row.get("package") != live.package:
            failures.append(
                f"{discovery_id} resolved package does not match live fact"
            )
        mechanical_target = mechanical_rust_target(
            {
                **fact,
                "package": live.package,
            }
        )
        if resolved_row.get("cargo_target_kind") != mechanical_target["kind"]:
            failures.append(
                f"{discovery_id} resolved cargo target kind is not mechanical"
            )
        if resolved_row.get("cargo_target_name") != mechanical_target["name"]:
            failures.append(
                f"{discovery_id} resolved cargo target name is not mechanical"
            )
        test_name = resolved_row.get("nextest_test_name")
        if (
            not isinstance(test_name, str)
            or test_name.rsplit("::", 1)[-1] != live.name
        ):
            failures.append(
                f"{discovery_id} nextest test identity does not match live fact"
            )
        try:
            expected_command = mechanical_resolved_rust_command(resolved_row)
        except ValueError as error:
            failures.append(f"{discovery_id} resolved command is invalid: {error}")
        else:
            if resolved_row.get("command") != expected_command:
                failures.append(
                    f"{discovery_id} resolved command is not mechanically derived"
                )
    current_source_digest = context.source_input_digest or focused_rust_input_digest(
        root=root,
        facts=context.selected_live_facts,
        denominator=context.denominator,
    )
    if provenance.get("source_input_digest") != current_source_digest:
        failures.append("focused Rust trace source inputs are stale")
    resolution_digest = _canonical_digest(resolved)
    if provenance.get("resolution_digest") != resolution_digest:
        failures.append("focused Rust trace resolution digest is stale")
    code_map_digest = _canonical_digest(metadata.code_map)
    if provenance.get("code_map_digest") != code_map_digest:
        failures.append("focused Rust trace code-map digest is stale")
    recomputed = trace_input_digest(
        current_source_digest,
        resolved,
        metadata.code_map,
    )
    if provenance.get("input_digest") != recomputed:
        failures.append("focused Rust provenance input digest cannot be reproduced")
    if trace.get("input_digest") != recomputed:
        failures.append("focused Rust trace input digest cannot be reproduced")
    return failures


def canonical_input_file(
    root: Path,
    path: Path,
    label: str,
    *,
    required_parent: str | None = None,
    require_directory: bool = False,
) -> Path:
    """Resolve one regular input without accepting any symlink component."""

    root = root.resolve()
    candidate = path if path.is_absolute() else root / path
    try:
        relative = candidate.relative_to(root)
    except ValueError as error:
        raise ValueError(f"{label} escapes repository") from error
    cursor = root
    for part in relative.parts:
        cursor = cursor / part
        if cursor.is_symlink():
            raise ValueError(f"{label} must not traverse symlinks")
    try:
        resolved = candidate.resolve(strict=True)
        resolved_relative = resolved.relative_to(root)
    except (OSError, ValueError) as error:
        raise ValueError(f"{label} is missing or escapes repository") from error
    if required_parent is not None and not resolved_relative.is_relative_to(
        required_parent
    ):
        raise ValueError(f"{label} must stay under {required_parent}")
    if require_directory:
        if not resolved.is_dir():
            raise ValueError(f"{label} must be a regular directory")
    elif not resolved.is_file():
        raise ValueError(f"{label} must be a regular file")
    return resolved


def _load_json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} root must be an object")
    return document


def _canonical_digest(value: Any) -> str:
    content = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
