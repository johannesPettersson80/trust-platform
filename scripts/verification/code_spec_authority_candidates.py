"""Compile authenticated coverage facts into an ID-free authority review."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .code_spec_authority_contract import oracle_eligible
from .code_spec_live import CodeSpecMetadata, load_code_spec_metadata
from .focused_mapping_context import (
    build_focused_mapping_context,
    canonical_input_file,
    validate_focused_rust_trace,
)
from .mapping_transaction import apply_mapping_transaction
from .metadata_validator.constants import HIGH_RISKS
from .quarantine_paths import quarantine_path


CODE_MAP_PATH = "verification/code-spec-map.toml"
SPEC_SOURCES_PATH = "verification/spec-sources.toml"
INVARIANT_ROOT = "verification/invariants"
CARD_FIELDS = {
    "schema_version",
    "kind",
    "trace_path",
    "trace_digest",
    "source_commit",
    "reviewed_date",
    "source_bindings",
    "code_facts",
    "discoveries",
    "invariants",
    "authority_groups",
    "card_digest",
}
TEMPLATE_FIELDS = {
    "schema_version",
    "kind",
    "generated_binding",
    "reviewer_ids",
    "selections",
}
BINDING_FIELDS = {
    "card_path",
    "card_digest",
    "trace_path",
    "trace_digest",
    "source_commit",
}
SELECTION_FIELDS = {
    "code_indices",
    "discovery_indices",
    "invariant_indices",
}
SPEC_REF_RE = re.compile(r"^([A-Z0-9_]+)#[A-Za-z0-9._:-]+$")


@dataclass(frozen=True)
class _AuthenticatedInputs:
    trace_relative: str
    trace_bytes: bytes
    trace: Mapping[str, Any]
    reviewed_date: str
    metadata: CodeSpecMetadata
    source_bindings: tuple[dict[str, str], ...]
    source_originals: Mapping[str, bytes]


def write_authority_candidate_card(
    *,
    root: Path,
    trace_path: Path,
    output_path: Path,
) -> dict[str, Any]:
    """Render numbered mechanical choices from one authenticated focused trace."""

    root = root.resolve()
    output_relative, output_resolved = quarantine_path(
        root, output_path, label="authority candidate card output", must_exist=False
    )
    inputs = _authenticate_inputs(root, trace_path)
    trace_resolved = root / inputs.trace_relative
    if output_resolved == trace_resolved:
        raise ValueError("trace and authority candidate card paths must differ")
    card = _build_card(inputs)
    _write_quarantine(
        root=root,
        relative=output_relative,
        content=_json_bytes(card),
        label="authority candidate card",
        expected_originals={
            inputs.trace_relative: inputs.trace_bytes,
            **inputs.source_originals,
        },
    )
    return card


def write_authority_candidate_human_template(
    *,
    root: Path,
    card_path: Path,
    output_path: Path,
) -> dict[str, Any]:
    """Generate all binding data so reviewers enter only IDs and indices."""

    root = root.resolve()
    card_relative, card_resolved = quarantine_path(
        root, card_path, label="authority candidate card", must_exist=True
    )
    output_relative, output_resolved = quarantine_path(
        root,
        output_path,
        label="authority candidate human template output",
        must_exist=False,
    )
    if output_resolved == card_resolved:
        raise ValueError("candidate card and human template paths must differ")
    card_bytes = card_resolved.read_bytes()
    card = _load_object(card_bytes, "authority candidate card")
    rebuilt, inputs = _rebuild_card(root, card)
    if card != rebuilt or card_bytes != _json_bytes(rebuilt):
        raise ValueError("authority candidate card differs from rebuilt inputs")
    template = {
        "schema_version": 1,
        "kind": "generated_numbered_authority_candidate_selection",
        "generated_binding": {
            "card_path": card_relative,
            "card_digest": card["card_digest"],
            "trace_path": card["trace_path"],
            "trace_digest": card["trace_digest"],
            "source_commit": card["source_commit"],
        },
        "reviewer_ids": [],
        "selections": [],
    }
    _write_quarantine(
        root=root,
        relative=output_relative,
        content=_json_bytes(template),
        label="authority candidate human template",
        expected_originals={
            card_relative: card_bytes,
            inputs.trace_relative: inputs.trace_bytes,
            **inputs.source_originals,
        },
    )
    return template


def build_authority_candidate_proposal(
    *,
    root: Path,
    card_path: Path,
    human_selection_path: Path,
) -> dict[str, Any]:
    """Purely rebuild an authority proposal from numbered human choices."""

    root = root.resolve()
    card_relative, card_resolved = quarantine_path(
        root, card_path, label="authority candidate card", must_exist=True
    )
    human_relative, human_resolved = quarantine_path(
        root,
        human_selection_path,
        label="authority candidate human selection",
        must_exist=True,
    )
    if card_resolved == human_resolved:
        raise ValueError("candidate card and human selection paths must differ")
    card_bytes = card_resolved.read_bytes()
    human_bytes = human_resolved.read_bytes()
    card = _load_object(card_bytes, "authority candidate card")
    rebuilt, _ = _rebuild_card(root, card)
    if card != rebuilt or card_bytes != _json_bytes(rebuilt):
        raise ValueError("authority candidate card differs from rebuilt inputs")
    human = _load_object(human_bytes, "authority candidate human selection")
    _validate_human_selection(
        human,
        expected_binding={
            "card_path": card_relative,
            "card_digest": card["card_digest"],
            "trace_path": card["trace_path"],
            "trace_digest": card["trace_digest"],
            "source_commit": card["source_commit"],
        },
    )
    rows = _compile_rows(card, human)
    risk_classes = _selected_risks(card, human)
    return {
        "schema_version": 1,
        "reviewed_date": card["reviewed_date"],
        "candidate_provenance": {
            "compiler": "numbered_code_spec_authority_candidates_v1",
            "candidate_card_path": card_relative,
            "candidate_card_digest": card["card_digest"],
            "candidate_human_selection_path": human_relative,
            "candidate_human_selection_digest": _digest(human_bytes),
            "trace_path": card["trace_path"],
            "trace_digest": card["trace_digest"],
            "proposer_ids": sorted(human["reviewer_ids"]),
            "risk_classes": risk_classes,
            "independent_authority_review_required": any(
                risk in HIGH_RISKS for risk in risk_classes
            ),
        },
        "rows": rows,
    }


def compile_authority_candidate_proposal(
    *,
    root: Path,
    card_path: Path,
    human_selection_path: Path,
    output_path: Path,
) -> dict[str, Any]:
    """Atomically write the existing authority-proposal JSON contract."""

    root = root.resolve()
    card_relative, card_resolved = quarantine_path(
        root, card_path, label="authority candidate card", must_exist=True
    )
    human_relative, human_resolved = quarantine_path(
        root,
        human_selection_path,
        label="authority candidate human selection",
        must_exist=True,
    )
    output_relative, output_resolved = quarantine_path(
        root,
        output_path,
        label="authority proposal output",
        must_exist=False,
    )
    if output_resolved in {card_resolved, human_resolved}:
        raise ValueError("authority proposal output must differ from review inputs")
    proposal = build_authority_candidate_proposal(
        root=root,
        card_path=card_resolved,
        human_selection_path=human_resolved,
    )
    card = _load_object(card_resolved.read_bytes(), "authority candidate card")
    _, inputs = _rebuild_card(root, card)
    _write_quarantine(
        root=root,
        relative=output_relative,
        content=_json_bytes(proposal),
        label="authority proposal",
        expected_originals={
            card_relative: card_resolved.read_bytes(),
            human_relative: human_resolved.read_bytes(),
            inputs.trace_relative: inputs.trace_bytes,
            **inputs.source_originals,
        },
    )
    return proposal


def _authenticate_inputs(root: Path, trace_path: Path) -> _AuthenticatedInputs:
    trace_relative, trace_resolved = quarantine_path(
        root, trace_path, label="focused trace", must_exist=True
    )
    trace_bytes = trace_resolved.read_bytes()
    trace = _load_object(trace_bytes, "focused trace")
    scope = trace.get("scope")
    discovery_ids = (
        scope.get("requested_discovery_ids")
        if isinstance(scope, Mapping)
        else None
    )
    if (
        not isinstance(discovery_ids, list)
        or not discovery_ids
        or any(not isinstance(value, str) for value in discovery_ids)
        or len(discovery_ids) != len(set(discovery_ids))
    ):
        raise ValueError("focused trace requested discovery IDs are malformed")
    if trace.get("source_commit") != _git_head(root):
        raise ValueError("focused trace source commit is stale")
    source_originals, source_bindings = _source_snapshot(root)
    metadata = load_code_spec_metadata(root)
    context = build_focused_mapping_context(
        root=root,
        discovery_ids=discovery_ids,
        tests=metadata.tests,
    )
    validate_focused_rust_trace(
        root=root,
        trace=trace,
        context=context,
        metadata=metadata,
    )
    return _AuthenticatedInputs(
        trace_relative=trace_relative,
        trace_bytes=trace_bytes,
        trace=trace,
        reviewed_date=_commit_date(root, str(trace["source_commit"])),
        metadata=metadata,
        source_bindings=source_bindings,
        source_originals=source_originals,
    )


def _source_snapshot(
    root: Path,
) -> tuple[dict[str, bytes], tuple[dict[str, str], ...]]:
    relative_paths = [CODE_MAP_PATH, SPEC_SOURCES_PATH]
    invariant_root = _canonical_directory(
        root, root / INVARIANT_ROOT, "invariant root"
    )
    relative_paths.extend(
        path.relative_to(root).as_posix()
        for path in sorted(invariant_root.rglob("*.toml"))
    )
    originals: dict[str, bytes] = {}
    bindings = []
    for relative in relative_paths:
        resolved = canonical_input_file(
            root, root / relative, f"authority source {relative}"
        )
        if not resolved.is_file():
            raise ValueError(f"authority source {relative} must be a file")
        content = resolved.read_bytes()
        originals[relative] = content
        bindings.append({"path": relative, "digest": _digest(content)})
    return originals, tuple(bindings)


def _build_card(inputs: _AuthenticatedInputs) -> dict[str, Any]:
    trace_records = inputs.trace.get("tests")
    if not isinstance(trace_records, list) or any(
        not isinstance(record, Mapping) for record in trace_records
    ):
        raise ValueError("focused trace tests must be an object array")
    selected_records = sorted(
        trace_records, key=lambda record: str(record.get("discovery_id"))
    )
    code_ids = sorted(
        {
            str(code_id)
            for record in selected_records
            for code_id in _string_array(
                record.get("covered_code_ids"), "covered code IDs", allow_empty=True
            )
            if _is_spec_gap(inputs.metadata.mappings.get(str(code_id)))
        }
    )
    code_index = {code_id: index for index, code_id in enumerate(code_ids, 1)}
    discovery_index = {
        str(record["discovery_id"]): index
        for index, record in enumerate(selected_records, 1)
    }
    discoveries = []
    for record in selected_records:
        if record.get("trace_status") != "observed":
            raise ValueError("authority candidates require observed trace records")
        covered = sorted(
            code_index[code_id]
            for code_id in _string_array(
                record.get("covered_code_ids"),
                "covered code IDs",
                allow_empty=True,
            )
            if code_id in code_index
        )
        discoveries.append(
            {
                "discovery_index": discovery_index[str(record["discovery_id"])],
                "discovery_id": record["discovery_id"],
                "path": record["path"],
                "name": record["name"],
                "covered_code_indices": covered,
            }
        )
    code_facts = []
    for code_id in code_ids:
        mapping = inputs.metadata.mappings[code_id]
        code_facts.append(
            {
                "code_index": code_index[code_id],
                "code_id": code_id,
                "area": _required_string(mapping, "area", code_id),
                "source_kind": _required_string(
                    mapping, "source_kind", code_id
                ),
                "path": _required_string(mapping, "path", code_id),
                "qualified_symbol": _required_string(
                    mapping, "qualified_symbol", code_id
                ),
                "item_kind": _required_string(
                    mapping, "item_kind", code_id
                ),
                "owner": _required_string(mapping, "owner", code_id),
                "risk_signals": sorted(
                    _string_array(
                        mapping.get("risk_signals"),
                        f"{code_id} risk signals",
                        allow_empty=True,
                    )
                ),
                "covered_by_discovery_indices": [
                    row["discovery_index"]
                    for row in discoveries
                    if code_index[code_id] in row["covered_code_indices"]
                ],
            }
        )
    invariants = _compatible_invariants(
        inputs.metadata,
        eligible_areas={str(row["area"]) for row in code_facts},
    )
    authority_groups = _authority_groups(invariants)
    card_without_digest = {
        "schema_version": 1,
        "kind": "numbered_code_spec_authority_candidates",
        "trace_path": inputs.trace_relative,
        "trace_digest": _digest(inputs.trace_bytes),
        "source_commit": inputs.trace["source_commit"],
        "reviewed_date": inputs.reviewed_date,
        "source_bindings": list(inputs.source_bindings),
        "code_facts": code_facts,
        "discoveries": discoveries,
        "invariants": invariants,
        "authority_groups": authority_groups,
    }
    return {
        **card_without_digest,
        "card_digest": _canonical_digest(card_without_digest),
    }


def _compatible_invariants(
    metadata: CodeSpecMetadata, *, eligible_areas: set[str]
) -> list[dict[str, Any]]:
    rows = []
    for invariant_id, invariant in sorted(metadata.invariants.items()):
        area = invariant.get("area")
        spec = invariant.get("spec")
        oracle = invariant.get("oracle")
        spec_ref = oracle.get("ref") if isinstance(oracle, Mapping) else None
        match = SPEC_REF_RE.fullmatch(spec_ref) if isinstance(spec_ref, str) else None
        source_id = match.group(1) if match is not None else None
        source_refs = spec.get("source_refs") if isinstance(spec, Mapping) else None
        if (
            not isinstance(area, str)
            or not area
            or area not in eligible_areas
            or not isinstance(spec, Mapping)
            or spec.get("status") != "specified"
            or not isinstance(source_refs, list)
            or source_id not in source_refs
            or not oracle_eligible(metadata.spec_sources.get(str(source_id)))
        ):
            continue
        rows.append(
            {
                "invariant_index": len(rows) + 1,
                "invariant_id": invariant_id,
                "title": _required_string(
                    invariant, "title", invariant_id
                ),
                "claim": _required_string(
                    invariant, "claim", invariant_id
                ),
                "risk": _required_string(invariant, "risk", invariant_id),
                "area": area,
                "spec_ref": spec_ref,
                "oracle": {
                    "kind": _required_string(
                        oracle, "kind", f"{invariant_id} oracle"
                    ),
                    "ref": spec_ref,
                },
                "behavior_summaries": _behavior_summaries(
                    invariant, invariant_id
                ),
            }
        )
    return rows


def _authority_groups(
    invariants: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str], list[int]] = {}
    for invariant in invariants:
        key = (str(invariant["area"]), str(invariant["spec_ref"]))
        grouped.setdefault(key, []).append(int(invariant["invariant_index"]))
    return [
        {
            "group_index": index,
            "area": area,
            "spec_ref": spec_ref,
            "invariant_indices": indices,
        }
        for index, ((area, spec_ref), indices) in enumerate(
            sorted(grouped.items()), 1
        )
    ]


def _rebuild_card(
    root: Path, card: Mapping[str, Any]
) -> tuple[dict[str, Any], _AuthenticatedInputs]:
    if set(card) != CARD_FIELDS:
        raise ValueError("authority candidate card fields drift")
    trace_path = card.get("trace_path")
    if not isinstance(trace_path, str):
        raise ValueError("authority candidate card trace path is malformed")
    inputs = _authenticate_inputs(root, root / trace_path)
    return _build_card(inputs), inputs


def _validate_human_selection(
    human: Mapping[str, Any], *, expected_binding: Mapping[str, Any]
) -> None:
    if set(human) != TEMPLATE_FIELDS:
        raise ValueError("authority candidate human selection fields drift")
    if (
        human.get("schema_version") != 1
        or human.get("kind")
        != "generated_numbered_authority_candidate_selection"
    ):
        raise ValueError("authority candidate human template identity drift")
    binding = human.get("generated_binding")
    if (
        not isinstance(binding, Mapping)
        or set(binding) != BINDING_FIELDS
        or dict(binding) != dict(expected_binding)
    ):
        raise ValueError("authority candidate generated binding is stale")
    _string_array(human.get("reviewer_ids"), "reviewer IDs")
    selections = human.get("selections")
    if (
        not isinstance(selections, list)
        or not selections
        or any(not isinstance(selection, Mapping) for selection in selections)
    ):
        raise ValueError("authority candidate selections must be non-empty objects")
    for index, selection in enumerate(selections, 1):
        if set(selection) != SELECTION_FIELDS:
            raise ValueError(f"authority candidate selection {index} fields drift")


def _compile_rows(
    card: Mapping[str, Any], human: Mapping[str, Any]
) -> list[dict[str, Any]]:
    code_by_index = _index_rows(card["code_facts"], "code_index", "code facts")
    discovery_by_index = _index_rows(
        card["discoveries"], "discovery_index", "discoveries"
    )
    invariant_by_index = _index_rows(
        card["invariants"], "invariant_index", "invariants"
    )
    used_code_indices: set[int] = set()
    rows = []
    for selection_number, selection in enumerate(human["selections"], 1):
        code_indices = _indices(
            selection["code_indices"], code_by_index, "code indices"
        )
        discovery_indices = _indices(
            selection["discovery_indices"],
            discovery_by_index,
            "discovery indices",
        )
        invariant_indices = _indices(
            selection["invariant_indices"],
            invariant_by_index,
            "invariant indices",
        )
        duplicates = used_code_indices.intersection(code_indices)
        if duplicates:
            raise ValueError("code indices may appear in only one selection")
        used_code_indices.update(code_indices)
        invariants = [invariant_by_index[index] for index in invariant_indices]
        authority_keys = {
            (str(row["area"]), str(row["spec_ref"])) for row in invariants
        }
        if len(authority_keys) != 1:
            raise ValueError(
                f"selection {selection_number} invariants cross area or oracle"
            )
        area, spec_ref = next(iter(authority_keys))
        codes = [code_by_index[index] for index in code_indices]
        if any(code["area"] != area for code in codes):
            raise ValueError(
                f"selection {selection_number} code and invariant areas differ"
            )
        discoveries = [
            discovery_by_index[index] for index in discovery_indices
        ]
        for code in codes:
            if any(
                int(discovery["discovery_index"])
                not in code["covered_by_discovery_indices"]
                for discovery in discoveries
            ):
                raise ValueError(
                    f"selection {selection_number} contains an uncovered "
                    "code/discovery pair"
                )
            rows.append(
                {
                    "code_id": code["code_id"],
                    "spec_ref": spec_ref,
                    "invariant_ids": sorted(
                        str(invariant["invariant_id"])
                        for invariant in invariants
                    ),
                    "discovery_ids": sorted(
                        str(discovery["discovery_id"])
                        for discovery in discoveries
                    ),
                }
            )
    return sorted(rows, key=lambda row: str(row["code_id"]))


def _selected_risks(
    card: Mapping[str, Any], human: Mapping[str, Any]
) -> list[str]:
    invariant_by_index = _index_rows(
        card["invariants"], "invariant_index", "invariants"
    )
    return sorted(
        {
            _required_string(
                invariant_by_index[index],
                "risk",
                f"invariant index {index}",
            )
            for selection in human["selections"]
            for index in _indices(
                selection["invariant_indices"],
                invariant_by_index,
                "invariant indices",
            )
        }
    )


def _behavior_summaries(
    invariant: Mapping[str, Any], invariant_id: str
) -> list[dict[str, Any]]:
    behaviors = invariant.get("behavior", [])
    if not isinstance(behaviors, list) or any(
        not isinstance(behavior, Mapping) for behavior in behaviors
    ):
        raise ValueError(f"{invariant_id} behaviors are malformed")
    fields = (
        "partition",
        "case_family",
        "oracle_ref",
        "outcome",
        "error_code",
        "no_partial_apply",
        "delta",
        "rationale",
    )
    return [
        {
            field: behavior[field]
            for field in fields
            if field in behavior
        }
        for behavior in behaviors
    ]


def _index_rows(
    value: Any, key: str, label: str
) -> dict[int, Mapping[str, Any]]:
    if not isinstance(value, list) or any(
        not isinstance(row, Mapping) for row in value
    ):
        raise ValueError(f"authority candidate card {label} are malformed")
    indexed: dict[int, Mapping[str, Any]] = {}
    for row in value:
        index = row.get(key)
        if not isinstance(index, int) or isinstance(index, bool) or index < 1:
            raise ValueError(f"authority candidate card {label} index is malformed")
        if index in indexed:
            raise ValueError(f"authority candidate card {label} duplicates index")
        indexed[index] = row
    if sorted(indexed) != list(range(1, len(indexed) + 1)):
        raise ValueError(f"authority candidate card {label} indices are not dense")
    return indexed


def _indices(
    value: Any, available: Mapping[int, Any], label: str
) -> list[int]:
    if (
        not isinstance(value, list)
        or not value
        or any(
            not isinstance(index, int)
            or isinstance(index, bool)
            or index not in available
            for index in value
        )
        or len(value) != len(set(value))
    ):
        raise ValueError(f"{label} must be unique in-range numeric indices")
    return sorted(value)


def _is_spec_gap(mapping: Mapping[str, Any] | None) -> bool:
    return bool(mapping and mapping.get("disposition") == "spec_gap")


def _required_string(
    record: Mapping[str, Any], field: str, label: str
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _string_array(value: Any, label: str, *, allow_empty: bool = False) -> list[str]:
    if (
        not isinstance(value, list)
        or (not value and not allow_empty)
        or any(not isinstance(item, str) or not item for item in value)
        or len(value) != len(set(value))
    ):
        raise ValueError(f"{label} must be a unique string array")
    return list(value)


def _commit_date(root: Path, source_commit: str) -> str:
    completed = subprocess.run(
        ["git", "show", "-s", "--format=%cs", source_commit],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    reviewed_date = completed.stdout.strip()
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", reviewed_date):
        raise ValueError("source commit date is malformed")
    return reviewed_date


def _canonical_directory(root: Path, path: Path, label: str) -> Path:
    root = root.resolve()
    try:
        relative = path.relative_to(root)
    except ValueError as error:
        raise ValueError(f"{label} escapes repository") from error
    cursor = root
    for part in relative.parts:
        cursor = cursor / part
        if cursor.is_symlink():
            raise ValueError(f"{label} must not traverse symlinks")
    try:
        resolved = path.resolve(strict=True)
        resolved.relative_to(root)
    except (OSError, ValueError) as error:
        raise ValueError(f"{label} is missing or escapes repository") from error
    if not resolved.is_dir():
        raise ValueError(f"{label} must be a directory")
    return resolved


def _git_head(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def _write_quarantine(
    *,
    root: Path,
    relative: str,
    content: bytes,
    label: str,
    expected_originals: Mapping[str, bytes],
) -> None:
    output_path = root / relative
    original = output_path.read_bytes() if output_path.is_file() else None
    apply_mapping_transaction(
        root=root,
        replacements={relative: content},
        expected_originals={
            **expected_originals,
            relative: original,
        },
        validate=lambda: _validate_exact_output(output_path, content, label),
    )


def _validate_exact_output(
    path: Path, expected: bytes, label: str
) -> list[str]:
    if not path.is_file() or path.read_bytes() != expected:
        return [f"{label} output differs from canonical bytes"]
    return []


def _load_object(content: bytes, label: str) -> dict[str, Any]:
    value = json.loads(content)
    if not isinstance(value, dict):
        raise ValueError(f"{label} root must be an object")
    return value


def _json_bytes(value: Mapping[str, Any]) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def _digest(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _canonical_digest(value: Mapping[str, Any]) -> str:
    return _digest(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Compile numbered authority candidates without manual IDs"
    )
    parser.add_argument("--root", type=Path, default=Path.cwd())
    subparsers = parser.add_subparsers(dest="command", required=True)
    card = subparsers.add_parser("card")
    card.add_argument("--trace", type=Path, required=True)
    card.add_argument("--output", type=Path, required=True)
    template = subparsers.add_parser("template")
    template.add_argument("--card", type=Path, required=True)
    template.add_argument("--output", type=Path, required=True)
    compile_parser = subparsers.add_parser("compile")
    compile_parser.add_argument("--card", type=Path, required=True)
    compile_parser.add_argument("--human-selection", type=Path, required=True)
    compile_parser.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "card":
            document = write_authority_candidate_card(
                root=args.root,
                trace_path=args.trace,
                output_path=args.output,
            )
        elif args.command == "template":
            document = write_authority_candidate_human_template(
                root=args.root,
                card_path=args.card,
                output_path=args.output,
            )
        else:
            document = compile_authority_candidate_proposal(
                root=args.root,
                card_path=args.card,
                human_selection_path=args.human_selection,
                output_path=args.output,
            )
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"code-spec authority candidate error: {error}", file=sys.stderr)
        return 1
    print(
        f"code-spec authority candidate {args.command}: "
        f"rows={len(document.get('rows', []))}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
