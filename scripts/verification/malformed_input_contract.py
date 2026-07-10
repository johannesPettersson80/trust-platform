"""Reviewed malformed-input taxonomy and catalog binding contracts."""

from __future__ import annotations

import json
import re
import tomllib
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_validation import check_supported_schema_keywords, is_safe_relative_path
from .metadata_validator.integrity import OPEN_GAP_RESOLUTIONS


TAXONOMY_PATH = "verification/malformed-input-taxonomy.toml"
TAXONOMY_SCHEMA_PATH = "verification/schemas/malformed-input-taxonomy.schema.json"
ALLOWED_DISPOSITIONS = {"required", "spec_gap", "blocked", "deferred", "not_applicable"}
ALLOWED_MALFORMED_TEST_CLASSES = {"negative_malformed_input", "fuzz"}
CLASS_ID_RE = re.compile(r"^[a-z][a-z0-9_]*$")
DOC_ROW_RE = re.compile(
    r"^\| `(?P<id>[a-z][a-z0-9_]*)` \| (?P<title>[^|]+?) \| "
    r"`(?P<disposition>[a-z_]+)` \| `(?P<authority>[^`|]+)` \|$"
)
ROOT_FIELDS = {
    "schema_version",
    "id",
    "title",
    "area",
    "surface_id",
    "review_doc",
    "last_reviewed",
    "classes",
}
CLASS_COMMON_FIELDS = {"id", "title", "disposition", "rationale"}


def load_malformed_input_taxonomy(root: Path) -> dict[str, Any]:
    """Load the committed reviewed taxonomy."""

    return tomllib.loads((root / TAXONOMY_PATH).read_text())


def validate_malformed_input_contract(root: Path, taxonomy: Mapping[str, Any]) -> list[str]:
    """Validate schema, semantics, references, and review-document drift."""

    failures: list[str] = []
    schema_path = root / TAXONOMY_SCHEMA_PATH
    try:
        schema = json.loads(schema_path.read_text())
    except Exception as exc:
        return [f"malformed-input taxonomy schema cannot be read: {exc}"]
    failures.extend(validate_taxonomy_schema_contract(schema))
    failures.extend(validate_json_schema_instance(dict(taxonomy), schema))
    if set(taxonomy) != ROOT_FIELDS:
        failures.append("malformed-input taxonomy root fields drift from contract")
    if taxonomy.get("area") != "bytecode_vm":
        failures.append("malformed-input taxonomy v1 must remain bytecode_vm-only")
    if taxonomy.get("surface_id") != "bytecode_container_instruction_stream":
        failures.append("malformed-input taxonomy v1 uses an unknown surface")

    classes = taxonomy.get("classes")
    if not isinstance(classes, list) or not classes:
        return [*failures, "malformed-input taxonomy requires classes"]
    class_ids = [item.get("id") for item in classes if isinstance(item, Mapping)]
    if len(class_ids) != len(classes) or any(not isinstance(item, str) for item in class_ids):
        failures.append("malformed-input taxonomy class IDs must be strings")
    elif len(class_ids) != len(set(class_ids)):
        failures.append("malformed-input taxonomy duplicates class IDs")
    elif class_ids != sorted(class_ids):
        failures.append("malformed-input taxonomy classes must use canonical ID ordering")

    spec_sources = _load_records(root / "verification/spec-sources.toml", "spec_sources")
    spec_gaps = _load_records(root / "verification/spec-gaps.toml", "spec_gaps")
    for index, item in enumerate(classes):
        if not isinstance(item, Mapping):
            failures.append(f"malformed-input taxonomy class {index} must be a table")
            continue
        class_id = item.get("id")
        label = class_id if isinstance(class_id, str) else f"classes[{index}]"
        if not isinstance(class_id, str) or not CLASS_ID_RE.fullmatch(class_id):
            failures.append(f"malformed-input class {label} has invalid ID")
        disposition = item.get("disposition")
        if disposition not in ALLOWED_DISPOSITIONS:
            failures.append(f"malformed-input class {label} has unknown disposition {disposition!r}")
            continue
        expected_fields = set(CLASS_COMMON_FIELDS)
        if disposition == "required":
            expected_fields.add("oracle_ref")
            reference = item.get("oracle_ref")
            source = spec_sources.get(_base_ref(reference))
            if (
                source is None
                or source.get("area") != taxonomy.get("area")
                or source.get("source_status") != "active"
                or source.get("authority") == "public_claim"
            ):
                failures.append(f"malformed-input class {label} references unknown oracle {reference!r}")
        elif disposition == "spec_gap":
            expected_fields.add("spec_gap_ref")
            reference = item.get("spec_gap_ref")
            gap = spec_gaps.get(_base_ref(reference))
            if (
                gap is None
                or gap.get("area") != taxonomy.get("area")
                or gap.get("status") != "spec_gap"
                or gap.get("resolution_status") not in OPEN_GAP_RESOLUTIONS
            ):
                failures.append(
                    f"malformed-input class {label} requires an open/actionable same-area spec gap: {reference!r}"
                )
        elif disposition == "blocked":
            expected_fields.add("blocker_ref")
            if not isinstance(item.get("blocker_ref"), str) or not item["blocker_ref"].strip():
                failures.append(f"malformed-input class {label} blocked disposition requires blocker_ref")
        elif disposition == "not_applicable":
            expected_fields.add("decision_ref")
            decision = spec_sources.get(_base_ref(item.get("decision_ref")))
            if (
                decision is None
                or decision.get("area") != taxonomy.get("area")
                or decision.get("source_status") != "active"
                or decision.get("authority") not in {"reviewed_decision", "reviewed_deviation"}
            ):
                failures.append(
                    f"malformed-input class {label} not_applicable requires active same-area reviewed decision/deviation"
                )
        if set(item) != expected_fields:
            failures.append(
                f"malformed-input class {label} fields do not match {disposition} disposition"
            )

    failures.extend(_validate_review_doc(root, taxonomy))
    return failures


def validate_taxonomy_schema_contract(schema: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    check_supported_schema_keywords(dict(schema), "$", failures)
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        failures.append("malformed-input taxonomy schema root must be a closed object")
    if set(schema.get("required", [])) != ROOT_FIELDS:
        failures.append("malformed-input taxonomy schema required fields drift")
    properties = schema.get("properties", {})
    expected_consts = {
        "schema_version": 1,
        "id": "MALFORMED_INPUT_BYTECODE_VM_V1",
        "area": "bytecode_vm",
        "surface_id": "bytecode_container_instruction_stream",
        "review_doc": "verification/malformed-input-taxonomy.md",
    }
    for field, expected in expected_consts.items():
        if not isinstance(properties, Mapping) or properties.get(field, {}).get("const") != expected:
            failures.append(f"malformed-input taxonomy schema const for {field} drifts")
    definitions = schema.get("$defs", {})
    class_schema = definitions.get("class", {}) if isinstance(definitions, Mapping) else {}
    if class_schema.get("type") != "object" or class_schema.get("additionalProperties") is not False:
        failures.append("malformed-input taxonomy class schema must be a closed object")
    if set(class_schema.get("required", [])) != CLASS_COMMON_FIELDS:
        failures.append("malformed-input taxonomy class schema required fields drift")
    dispositions = class_schema.get("properties", {}).get("disposition", {}).get("enum")
    if set(dispositions or []) != ALLOWED_DISPOSITIONS:
        failures.append("malformed-input taxonomy disposition enum drifts")
    return failures


def validate_catalog_malformed_bindings(
    *,
    tests: Mapping[str, Mapping[str, Any]],
    taxonomy: Mapping[str, Any],
) -> list[str]:
    """Validate explicit test-to-malformed-class bindings without inference."""

    failures: list[str] = []
    classes = {
        item.get("id"): item
        for item in taxonomy.get("classes", [])
        if isinstance(item, Mapping) and isinstance(item.get("id"), str)
    }
    taxonomy_area = taxonomy.get("area")
    for test_id in sorted(tests):
        record = tests[test_id]
        class_ids = record.get("malformed_input_class_ids")
        subject_kind = record.get("subject_kind")
        test_class = record.get("test_class")
        if test_class == "negative_malformed_input" and subject_kind == "generated_test" and class_ids is None:
            failures.append(
                f"{test_id} negative_malformed_input requires malformed_input_class_ids"
            )
            continue
        if class_ids is None:
            continue
        if subject_kind != "generated_test":
            failures.append(f"{test_id} {subject_kind} forbids malformed_input_class_ids")
            continue
        if test_class not in ALLOWED_MALFORMED_TEST_CLASSES:
            failures.append(
                f"{test_id} test_class {test_class!r} forbids malformed_input_class_ids"
            )
        if not isinstance(class_ids, list) or not class_ids or not all(
            isinstance(item, str) and item for item in class_ids
        ):
            failures.append(f"{test_id} malformed_input_class_ids must be a non-empty string array")
            continue
        if len(class_ids) != len(set(class_ids)):
            failures.append(f"{test_id} duplicates malformed_input_class_ids")
        for class_id in sorted(set(class_ids)):
            malformed_class = classes.get(class_id)
            if malformed_class is None:
                failures.append(f"{test_id} references unknown malformed-input class {class_id}")
                continue
            if record.get("area") != taxonomy_area:
                failures.append(
                    f"{test_id} malformed-input class {class_id} area {taxonomy_area} "
                    f"does not match {record.get('area')}"
                )
            disposition = malformed_class.get("disposition")
            if disposition == "required":
                expected = _base_ref(malformed_class.get("oracle_ref"))
                if _base_ref(record.get("oracle_ref")) != expected:
                    failures.append(
                        f"{test_id} malformed-input class {class_id} requires oracle_ref {expected}"
                    )
            elif disposition == "spec_gap":
                expected = _base_ref(malformed_class.get("spec_gap_ref"))
                if _base_ref(record.get("spec_gap_ref")) != expected:
                    failures.append(
                        f"{test_id} malformed-input class {class_id} requires spec_gap_ref {expected}"
                    )
    return failures


def _validate_review_doc(root: Path, taxonomy: Mapping[str, Any]) -> list[str]:
    review_doc = taxonomy.get("review_doc")
    if not isinstance(review_doc, str) or not is_safe_relative_path(review_doc):
        return ["malformed-input taxonomy review_doc must be workspace-relative"]
    try:
        lines = (root / review_doc).read_text().splitlines()
    except Exception as exc:
        return [f"malformed-input taxonomy review_doc cannot be read: {exc}"]
    actual = []
    for line in lines:
        match = DOC_ROW_RE.fullmatch(line)
        if match:
            actual.append(
                (
                    match.group("id"),
                    match.group("title").strip(),
                    match.group("disposition"),
                    match.group("authority"),
                )
            )
    expected = []
    for item in taxonomy.get("classes", []):
        if not isinstance(item, Mapping):
            continue
        authority = (
            item.get("oracle_ref")
            or item.get("spec_gap_ref")
            or item.get("blocker_ref")
            or item.get("decision_ref")
        )
        expected.append((item.get("id"), item.get("title"), item.get("disposition"), authority))
    if actual != expected:
        return ["malformed-input taxonomy review document class table drifts from machine contract"]
    return []


def _load_records(path: Path, key: str) -> dict[str, Mapping[str, Any]]:
    try:
        data = tomllib.loads(path.read_text())
    except Exception:
        return {}
    return {
        item["id"]: item
        for item in data.get(key, [])
        if isinstance(item, Mapping) and isinstance(item.get("id"), str)
    }


def _base_ref(value: Any) -> str | None:
    return value.split("#", 1)[0] if isinstance(value, str) else None
