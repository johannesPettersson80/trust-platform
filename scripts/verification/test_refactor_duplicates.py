"""Content-based duplicate and reviewed malformed-class overlap analysis."""

from __future__ import annotations

import hashlib
import json
import tomllib
from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .test_refactor_file_metrics import read_workspace_bytes


def analyze_duplicate_fixtures(
    *,
    root: Path,
    paths: Sequence[str],
    catalog_records: Sequence[Mapping[str, Any]],
) -> dict[str, list[dict[str, Any]]]:
    """Find whole-file duplicates and explicit malformed-class overlaps."""

    exact: dict[str, list[str]] = defaultdict(list)
    normalized: dict[str, list[str]] = defaultdict(list)
    seen_paths: set[str] = set()
    for path in sorted(paths):
        if path in seen_paths:
            raise ValueError(f"duplicate source path in assessment input: {path}")
        seen_paths.add(path)
        raw = read_workspace_bytes(root, path)
        exact[_sha256(raw)].append(path)
        try:
            text = raw.decode("utf-8")
        except UnicodeError as exc:
            raise ValueError(f"test source is not UTF-8 text: {path}: {exc}") from exc
        normalized[_sha256(" ".join(text.split()).encode())].append(path)

    case_analysis = _case_input_analysis(root, catalog_records)
    return {
        "case_file_paths": case_analysis["case_file_paths"],
        "exact_groups": _content_groups(exact, "content_sha256"),
        "exact_case_input_groups": case_analysis["exact_case_input_groups"],
        "free_form_body_similarity": "not_assessed",
        "malformed_class_overlaps": _malformed_class_overlaps(catalog_records),
        "same_table_structural_shape_groups": case_analysis[
            "same_table_structural_shape_groups"
        ],
        "shared_case_file_reference_groups": case_analysis[
            "shared_case_file_reference_groups"
        ],
        "whitespace_normalized_groups": _content_groups(
            normalized, "normalized_content_sha256"
        ),
    }


def _content_groups(
    by_digest: Mapping[str, list[str]], digest_field: str
) -> list[dict[str, Any]]:
    return [
        {digest_field: digest, "paths": sorted(paths)}
        for digest, paths in sorted(by_digest.items())
        if len(paths) > 1
    ]


def _malformed_class_overlaps(
    catalog_records: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    by_class: dict[str, list[tuple[str, str]]] = defaultdict(list)
    seen_test_ids: set[str] = set()
    for record in catalog_records:
        test_id = _string(record.get("id"), "catalog test id")
        if test_id in seen_test_ids:
            raise ValueError(f"catalog duplicates test id {test_id}")
        seen_test_ids.add(test_id)
        class_ids = record.get("malformed_input_class_ids", [])
        if not isinstance(class_ids, list):
            raise ValueError(f"catalog test {test_id} malformed_input_class_ids must be a list")
        if len(class_ids) != len(set(class_ids)):
            raise ValueError(f"catalog test {test_id} duplicates malformed_input_class_ids")
        if not class_ids:
            continue
        path = _string(record.get("path"), f"catalog test {test_id} path")
        for class_id in class_ids:
            by_class[_string(class_id, "malformed input class id")].append((test_id, path))

    overlaps: list[dict[str, Any]] = []
    for class_id, owners in sorted(by_class.items()):
        if len(owners) < 2:
            continue
        overlaps.append(
            {
                "malformed_input_class_id": class_id,
                "paths": sorted({path for _, path in owners}),
                "test_ids": sorted(test_id for test_id, _ in owners),
            }
        )
    return overlaps


def _case_input_analysis(
    root: Path,
    catalog_records: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    references: dict[str, list[tuple[str, str]]] = defaultdict(list)
    for record in catalog_records:
        case_file = record.get("case_file")
        if case_file is None:
            continue
        case_file = _string(case_file, "catalog case_file")
        references[case_file].append(
            (
                _string(record.get("id"), "catalog test id"),
                _string(record.get("path"), "catalog record path"),
            )
        )

    exact_inputs: dict[str, list[tuple[str, str]]] = defaultdict(list)
    structural_groups: list[dict[str, Any]] = []
    for case_file in sorted(references):
        try:
            payload = tomllib.loads(read_workspace_bytes(root, case_file).decode("utf-8"))
        except (UnicodeError, tomllib.TOMLDecodeError) as exc:
            raise ValueError(f"case file is not valid UTF-8 TOML: {case_file}: {exc}") from exc
        cases = payload.get("case")
        if not isinstance(cases, list):
            raise ValueError(f"case file must contain [[case]] records: {case_file}")
        shapes: dict[str, list[str]] = defaultdict(list)
        case_ids: set[str] = set()
        for case in cases:
            if not isinstance(case, Mapping):
                raise ValueError(f"case file contains a non-object case: {case_file}")
            case_id = _string(case.get("id"), f"case id in {case_file}")
            if case_id in case_ids:
                raise ValueError(f"case file duplicates case id {case_id}: {case_file}")
            case_ids.add(case_id)
            case_input = case.get("input")
            if not isinstance(case_input, Mapping):
                raise ValueError(f"case {case_id} input must be an object")
            input_bytes = _canonical_json(case_input)
            exact_inputs[_sha256(input_bytes)].append((case_id, case_file))
            shape_bytes = _canonical_json(_value_shape(case_input))
            shapes[_sha256(shape_bytes)].append(case_id)
        for digest, peers in sorted(shapes.items()):
            if len(peers) > 1:
                structural_groups.append(
                    {
                        "case_file": case_file,
                        "case_ids": sorted(peers),
                        "shape_digest": f"sha256:{digest}",
                    }
                )

    exact_groups = []
    for digest, owners in sorted(exact_inputs.items()):
        if len(owners) > 1:
            exact_groups.append(
                {
                    "case_files": sorted({case_file for _, case_file in owners}),
                    "case_ids": sorted(case_id for case_id, _ in owners),
                    "input_digest": f"sha256:{digest}",
                }
            )
    shared = [
        {
            "case_file": case_file,
            "record_paths": sorted({path for _, path in owners}),
            "test_ids": sorted(test_id for test_id, _ in owners),
        }
        for case_file, owners in sorted(references.items())
        if len(owners) > 1
    ]
    return {
        "case_file_paths": sorted(references),
        "exact_case_input_groups": exact_groups,
        "same_table_structural_shape_groups": sorted(
            structural_groups, key=lambda row: (row["case_file"], row["case_ids"])
        ),
        "shared_case_file_reference_groups": shared,
    }


def _value_shape(value: Any) -> Any:
    if isinstance(value, Mapping):
        return {
            "kind": "table",
            "fields": [[str(key), _value_shape(child)] for key, child in sorted(value.items())],
        }
    if isinstance(value, list):
        return {"kind": "array", "items": [_value_shape(child) for child in value]}
    if isinstance(value, bool):
        return {"kind": "bool"}
    if isinstance(value, int):
        return {"kind": "int"}
    if isinstance(value, float):
        return {"kind": "float"}
    if isinstance(value, str):
        return {"kind": "string"}
    return {"kind": type(value).__name__}


def _canonical_json(value: Any) -> bytes:
    try:
        return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    except (TypeError, ValueError) as exc:
        raise ValueError(f"case input is not canonically JSON-serializable: {exc}") from exc


def _sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} must be a non-empty string")
    return value
