"""Small deterministic evaluator for the generated-catalog JSON Schema subset."""

from __future__ import annotations

import json
import re
from typing import Any


def validate_json_schema_instance(instance: Any, schema: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    validate_node(instance, schema, schema, "$", failures)
    return failures


def validate_node(
    instance: Any,
    schema: dict[str, Any],
    root_schema: dict[str, Any],
    path: str,
    failures: list[str],
) -> None:
    one_of = schema.get("oneOf")
    if isinstance(one_of, list):
        matches = 0
        for branch in one_of:
            if not isinstance(branch, dict):
                continue
            branch_failures: list[str] = []
            validate_node(instance, branch, root_schema, path, branch_failures)
            if not branch_failures:
                matches += 1
        if matches != 1:
            failures.append(f"{path}: value must match exactly one schema branch")
            return

    reference = schema.get("$ref")
    if isinstance(reference, str):
        target = resolve_local_reference(root_schema, reference)
        if target is None:
            failures.append(f"{path}: unresolved schema reference {reference}")
            return
        validate_node(instance, target, root_schema, path, failures)

    expected_type = schema.get("type")
    if expected_type is not None and not matches_type(instance, expected_type):
        failures.append(f"{path}: expected JSON type {expected_type!r}")
        return
    if "const" in schema and not json_equal(instance, schema["const"]):
        failures.append(f"{path}: value does not match schema const")
    enum = schema.get("enum")
    if isinstance(enum, list) and not any(json_equal(instance, candidate) for candidate in enum):
        failures.append(f"{path}: value is outside the schema enum")

    if isinstance(instance, dict):
        required = schema.get("required", [])
        if isinstance(required, list):
            for key in required:
                if key not in instance:
                    failures.append(f"{path}: missing required property {key}")
        dependent_required = schema.get("dependentRequired", {})
        if isinstance(dependent_required, dict):
            for key, dependencies in dependent_required.items():
                if key not in instance or not isinstance(dependencies, list):
                    continue
                for dependency in dependencies:
                    if dependency not in instance:
                        failures.append(
                            f"{path}: property {key} requires property {dependency}"
                        )
        properties = schema.get("properties", {})
        if not isinstance(properties, dict):
            properties = {}
        if schema.get("additionalProperties") is False:
            for key in sorted(set(instance) - set(properties)):
                failures.append(f"{path}: additional property {key} is forbidden")
        for key, value in instance.items():
            child_schema = properties.get(key)
            if isinstance(child_schema, dict):
                validate_node(value, child_schema, root_schema, f"{path}.{key}", failures)
    elif isinstance(instance, list):
        minimum = schema.get("minItems")
        if isinstance(minimum, int) and len(instance) < minimum:
            failures.append(f"{path}: array has fewer than {minimum} items")
        maximum = schema.get("maxItems")
        if isinstance(maximum, int) and len(instance) > maximum:
            failures.append(f"{path}: array has more than {maximum} items")
        if schema.get("uniqueItems") is True:
            encoded = [json.dumps(item, sort_keys=True, separators=(",", ":")) for item in instance]
            if len(encoded) != len(set(encoded)):
                failures.append(f"{path}: array items must be unique")
        item_schema = schema.get("items")
        if isinstance(item_schema, dict):
            for index, item in enumerate(instance):
                validate_node(item, item_schema, root_schema, f"{path}[{index}]", failures)
    elif isinstance(instance, str):
        minimum = schema.get("minLength")
        if isinstance(minimum, int) and len(instance) < minimum:
            failures.append(f"{path}: string is shorter than {minimum}")
        pattern = schema.get("pattern")
        if isinstance(pattern, str) and re.search(pattern, instance) is None:
            failures.append(f"{path}: string does not match schema pattern {pattern}")
    elif is_json_integer(instance):
        minimum = schema.get("minimum")
        if isinstance(minimum, (int, float)) and instance < minimum:
            failures.append(f"{path}: integer is below schema minimum {minimum}")


def resolve_local_reference(root_schema: dict[str, Any], reference: str) -> dict[str, Any] | None:
    if not reference.startswith("#/"):
        return None
    current: Any = root_schema
    for part in reference[2:].split("/"):
        key = part.replace("~1", "/").replace("~0", "~")
        if not isinstance(current, dict) or key not in current:
            return None
        current = current[key]
    return current if isinstance(current, dict) else None


def matches_type(instance: Any, expected: Any) -> bool:
    types = expected if isinstance(expected, list) else [expected]
    return any(matches_single_type(instance, item) for item in types)


def matches_single_type(instance: Any, expected: Any) -> bool:
    if expected == "object":
        return isinstance(instance, dict)
    if expected == "array":
        return isinstance(instance, list)
    if expected == "string":
        return isinstance(instance, str)
    if expected == "integer":
        return is_json_integer(instance)
    if expected == "boolean":
        return isinstance(instance, bool)
    if expected == "null":
        return instance is None
    return False


def is_json_integer(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def json_equal(left: Any, right: Any) -> bool:
    if isinstance(left, bool) or isinstance(right, bool):
        return type(left) is type(right) and left == right
    return left == right
