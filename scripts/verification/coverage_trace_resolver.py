"""Mechanical scanner-to-nextest identity resolution for Rust tests."""

from __future__ import annotations

import shlex
from collections.abc import Mapping, Sequence
from pathlib import PurePosixPath
from typing import Any

from .coverage_trace_filter import exact_name_matcher


RUST_SOURCE_KINDS = {"rust_unit_test", "rust_integration_test"}


def mechanical_rust_target(fact: Mapping[str, Any]) -> dict[str, Any]:
    source_kind = _required_string(
        fact, "discovery_source_kind", "scanner fact"
    )
    package = _required_string(fact, "package", "scanner fact")
    raw_path = _required_string(fact, "path", "scanner fact")
    path = PurePosixPath(raw_path)
    if path.is_absolute() or "\\" in raw_path or ".." in path.parts:
        raise ValueError(f"Rust scanner fact has unsafe source path {raw_path!r}")
    parts = path.parts
    if source_kind == "rust_integration_test":
        try:
            tests_index = parts.index("tests")
            target_component = parts[tests_index + 1]
        except (ValueError, IndexError) as error:
            raise ValueError(
                f"Rust integration test is not below a tests target: {raw_path}"
            ) from error
        target_name = (
            PurePosixPath(target_component).stem
            if target_component.endswith(".rs")
            else target_component
        )
        if not target_name:
            raise ValueError(f"Rust integration test has no target name: {raw_path}")
        return {
            "kind": "test",
            "name": target_name,
            "cargo_args": ("--test", target_name),
        }
    if source_kind != "rust_unit_test":
        raise ValueError(f"unsupported Rust source kind {source_kind!r}")
    if "examples" in parts:
        example_index = parts.index("examples")
        try:
            component = parts[example_index + 1]
        except IndexError as error:
            raise ValueError(f"Rust example test has no target: {raw_path}") from error
        target_name = (
            PurePosixPath(component).stem
            if component.endswith(".rs")
            else component
        )
        return {
            "kind": "example",
            "name": target_name,
            "cargo_args": ("--example", target_name),
        }
    try:
        src_index = parts.index("src")
        source_tail = parts[src_index + 1 :]
    except ValueError as error:
        raise ValueError(
            f"Rust unit test is not below a source target: {raw_path}"
        ) from error
    if source_tail and source_tail[0] == "bin":
        if len(source_tail) < 2:
            raise ValueError(f"Rust binary test has no target: {raw_path}")
        component = source_tail[1]
        target_name = (
            PurePosixPath(component).stem
            if component.endswith(".rs")
            else component
        )
        return {
            "kind": "bin",
            "name": target_name,
            "cargo_args": ("--bin", target_name),
        }
    if source_tail and source_tail[0] == "main.rs":
        return {
            "kind": "bin",
            "name": package,
            "cargo_args": ("--bin", package),
        }
    return {"kind": "lib", "name": None, "cargo_args": ("--lib",)}


def resolve_rust_tests(
    *,
    facts: Sequence[Mapping[str, Any]],
    nextest_document: Mapping[str, Any],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Resolve only unique runtime identities; ambiguity remains explicit."""

    suites = nextest_document.get("rust-suites")
    if not isinstance(suites, Mapping):
        raise ValueError("nextest list lacks rust-suites object")
    runtime_tests: list[dict[str, str]] = []
    for suite_key, suite in suites.items():
        if not isinstance(suite_key, str) or not isinstance(suite, Mapping):
            raise ValueError("nextest rust-suites entries must be objects")
        package = _required_string(suite, "package-name", suite_key)
        binary_id = _required_string(suite, "binary-id", suite_key)
        testcases = suite.get("testcases")
        if not isinstance(testcases, Mapping):
            raise ValueError(f"nextest suite {suite_key} lacks testcases object")
        for test_name in testcases:
            if not isinstance(test_name, str) or not test_name:
                raise ValueError(f"nextest suite {suite_key} has invalid test name")
            runtime_tests.append(
                {
                    "package": package,
                    "binary_id": binary_id,
                    "test_name": test_name,
                }
            )

    resolved: list[dict[str, Any]] = []
    unsupported: list[dict[str, Any]] = []
    seen: set[str] = set()
    for fact in sorted(facts, key=lambda row: str(row.get("discovery_id", ""))):
        discovery_id = _required_string(fact, "discovery_id", "scanner fact")
        if discovery_id in seen:
            raise ValueError(f"scanner facts duplicate {discovery_id}")
        seen.add(discovery_id)
        source_kind = _required_string(
            fact, "discovery_source_kind", discovery_id
        )
        if source_kind not in RUST_SOURCE_KINDS:
            unsupported.append(
                _unsupported(fact, reason="source_kind_not_rust", candidates=[])
            )
            continue
        package = _required_string(fact, "package", discovery_id)
        name = _required_string(fact, "name", discovery_id)
        matches = sorted(
            (
                test
                for test in runtime_tests
                if test["package"] == package
                and test["test_name"].rsplit("::", 1)[-1] == name
            ),
            key=lambda row: (row["binary_id"], row["test_name"]),
        )
        if len(matches) != 1:
            unsupported.append(
                _unsupported(
                    fact,
                    reason=(
                        "no_nextest_identity"
                        if not matches
                        else "ambiguous_nextest_identity"
                    ),
                    candidates=[test["test_name"] for test in matches],
                )
            )
            continue
        match = matches[0]
        command = shlex.join(
            [
                "cargo",
                "nextest",
                "run",
                "-p",
                package,
                "-E",
                (
                    f"binary_id({exact_name_matcher(match['binary_id'])}) & "
                    f"test({exact_name_matcher(match['test_name'])})"
                ),
            ]
        )
        resolved.append(
            {
                "discovery_id": discovery_id,
                "discovery_source_kind": source_kind,
                "path": _required_string(fact, "path", discovery_id),
                "name": name,
                "ignore_state": _required_string(
                    fact, "ignore_state", discovery_id
                ),
                "package": package,
                "nextest_binary_id": match["binary_id"],
                "nextest_test_name": match["test_name"],
                "command": command,
                "command_authority": "verified_synthesized",
            }
        )
    return resolved, unsupported


def _unsupported(
    fact: Mapping[str, Any],
    *,
    reason: str,
    candidates: list[str],
) -> dict[str, Any]:
    return {
        "discovery_id": _required_string(fact, "discovery_id", "scanner fact"),
        "discovery_source_kind": _required_string(
            fact, "discovery_source_kind", "scanner fact"
        ),
        "path": _required_string(fact, "path", "scanner fact"),
        "name": _required_string(fact, "name", "scanner fact"),
        "reason": reason,
        "candidate_test_names": candidates,
    }


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
