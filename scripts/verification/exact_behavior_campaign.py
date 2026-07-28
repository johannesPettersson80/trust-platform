"""Build grouped exact-nextest campaigns from reviewed assignments."""

from __future__ import annotations

import re
import shlex
import tomllib
from collections import defaultdict
from collections.abc import Callable, Mapping
from pathlib import Path
from typing import Any

from .coverage_trace_filter import exact_name_matcher


CATALOG_PATH = Path("verification/test-catalog.toml")
NewTestIdentity = Callable[[Mapping[str, Any]], dict[str, Any]]


def build_exact_campaign_plan(
    *,
    root: Path,
    assignment: Mapping[str, Any],
    basis: str,
    group_prefix: str,
    full_name_overrides: Mapping[str, str] | None = None,
    new_test_identity: NewTestIdentity | None = None,
) -> dict[str, Any]:
    """Group each selected identity into one exact nextest invocation."""

    overrides = full_name_overrides or {}
    catalog = tomllib.loads((root / CATALOG_PATH).read_text())
    catalog_tests = catalog.get("tests")
    if not isinstance(catalog_tests, list):
        raise ValueError("test catalog lacks test records")
    catalog_by_id = {
        str(test["id"]): test
        for test in catalog_tests
        if isinstance(test, Mapping) and isinstance(test.get("id"), str)
    }

    grouped: dict[
        tuple[str, str, str | None],
        list[dict[str, Any]],
    ] = defaultdict(list)
    assignments = assignment.get("assignments")
    if not isinstance(assignments, list):
        raise ValueError("assignment plan lacks assignments")
    for selected in assignments:
        if not isinstance(selected, Mapping):
            raise ValueError("assignment plan contains a non-object")
        test_id = (
            str(selected["catalog_test_id"])
            if selected["catalog_test_id"] is not None
            else str(selected["discovery_id"])
        )
        if selected["catalog_test_id"] is None:
            if new_test_identity is None:
                raise ValueError(
                    f"{test_id} is uncataloged without an exact target resolver"
                )
            selected_identity = new_test_identity(selected)
        else:
            catalog_test = catalog_by_id.get(str(selected["catalog_test_id"]))
            if catalog_test is None:
                raise ValueError(f"catalog lacks {selected['catalog_test_id']}")
            selected_identity = catalog_test_identity(
                test_id,
                catalog_test,
                full_name_overrides=overrides,
            )
        grouped[
            (
                selected_identity["package"],
                selected_identity["target_kind"],
                selected_identity["target_name"],
            )
        ].append(
            {
                "test_id": test_id,
                "discovery_id": selected["discovery_id"],
                "path": selected["path"],
                **selected_identity,
            }
        )

    groups = []
    seen_test_ids: set[str] = set()
    for index, (target, selected) in enumerate(sorted(grouped.items())):
        package, target_kind, target_name = target
        selected.sort(key=lambda row: row["full_name"])
        full_names = [str(row["full_name"]) for row in selected]
        if len(full_names) != len(set(full_names)):
            raise ValueError(
                f"{target} contains duplicate exact test names"
            )
        test_ids = [str(row["test_id"]) for row in selected]
        overlap = seen_test_ids.intersection(test_ids)
        if overlap:
            raise ValueError(
                "campaign assigns tests to multiple targets: "
                + ", ".join(sorted(overlap))
            )
        seen_test_ids.update(test_ids)
        binary_ids = {str(row["binary_id"]) for row in selected}
        prefixes = {str(row["encoded_test_name_prefix"]) for row in selected}
        if len(binary_ids) != 1 or len(prefixes) != 1:
            raise ValueError(f"{target} binary identity drifted")
        binary_id = next(iter(binary_ids))
        encoded_prefix = next(iter(prefixes))
        exact_terms = [
            f"(binary_id({exact_name_matcher(binary_id)}) & "
            f"test({exact_name_matcher(name)}))"
            for name in full_names
        ]
        target_args = {
            "lib": ["--lib"],
            "bin": ["--bin", str(target_name)],
            "test": ["--test", str(target_name)],
        }.get(target_kind)
        if target_args is None:
            raise ValueError(f"unsupported target kind {target_kind}")
        command = [
            "cargo",
            "nextest",
            "run",
            "-p",
            package,
            *target_args,
            "--no-fail-fast",
            "--message-format",
            "libtest-json-plus",
            "--message-format-version",
            "0.1",
            "--user-config-file",
            "none",
            "-E",
            " | ".join(exact_terms),
        ]
        groups.append(
            {
                "id": f"{group_prefix}_{index + 1:02d}",
                "package": package,
                "target_kind": target_kind,
                "target_name": target_name,
                "binary_id": binary_id,
                "encoded_test_name_prefix": encoded_prefix,
                "test_count": len(selected),
                "test_ids": test_ids,
                "full_names": full_names,
                "tests_by_full_name": {
                    str(row["full_name"]): str(row["test_id"])
                    for row in selected
                },
                "paths": sorted({str(row["path"]) for row in selected}),
                "exact_terms": exact_terms,
                "command": command,
            }
        )
    expected = assignment.get("assigned_test_count")
    if not isinstance(expected, int) or len(seen_test_ids) != expected:
        raise ValueError("campaign omitted assigned test identities")
    behavior_count = assignment.get("demand_count")
    if not isinstance(behavior_count, int):
        raise ValueError("assignment plan lacks demand count")
    return {
        "schema_version": 1,
        "basis": basis,
        "behavior_count": behavior_count,
        "test_count": expected,
        "group_count": len(groups),
        "groups": groups,
    }


def catalog_test_identity(
    test_id: str,
    test: Mapping[str, Any],
    *,
    full_name_overrides: Mapping[str, str],
) -> dict[str, Any]:
    """Resolve the exact binary and full test name from a catalog command."""

    command = test.get("command")
    if not isinstance(command, str) or not command:
        raise ValueError(f"{test_id} lacks a command")
    name = test.get("name")
    if not isinstance(name, str) or not name:
        raise ValueError(f"{test_id} lacks a test name")
    tokens = shlex.split(command)
    if tokens[:2] == ["cargo", "test"]:
        return _cargo_test_identity(
            test_id,
            tokens,
            expected_name=name,
            full_name_overrides=full_name_overrides,
        )
    if tokens[:3] == ["cargo", "nextest", "run"]:
        return _nextest_identity(
            test_id,
            tokens,
            full_name_overrides=full_name_overrides,
        )
    raise ValueError(f"{test_id} has unsupported command {command!r}")


def exact_identity(
    *,
    package: str,
    target_kind: str,
    target_name: str | None,
    full_name: str,
) -> dict[str, Any]:
    """Return the canonical nextest target identity."""

    if target_kind == "lib":
        binary_id = package
        encoded_prefix = f"{package}::{package.replace('-', '_')}$"
    else:
        if not target_name:
            raise ValueError(f"{package} {target_kind} target lacks a name")
        binary_id = (
            f"{package}::bin/{target_name}"
            if target_kind == "bin"
            else f"{package}::{target_name}"
        )
        encoded_prefix = (
            f"{package}::{target_name}$"
            if target_kind == "bin"
            else f"{binary_id}$"
        )
    return {
        "package": package,
        "target_kind": target_kind,
        "target_name": target_name,
        "binary_id": binary_id,
        "encoded_test_name_prefix": encoded_prefix,
        "full_name": full_name,
    }


def _cargo_test_identity(
    test_id: str,
    tokens: list[str],
    *,
    expected_name: str,
    full_name_overrides: Mapping[str, str],
) -> dict[str, Any]:
    try:
        package = tokens[tokens.index("-p") + 1]
        separator = tokens.index("--")
    except (ValueError, IndexError) as error:
        raise ValueError(f"{test_id} lacks exact cargo test fields") from error
    if separator < 1:
        raise ValueError(f"{test_id} cargo command lacks a test name")
    full_name = tokens[separator - 1]
    if (
        full_name != expected_name
        and not full_name.endswith(f"::{expected_name}")
    ):
        raise ValueError(
            f"{test_id} cargo filter {full_name!r} does not identify "
            f"{expected_name!r}"
        )
    target_kind, target_name = _target(tokens[:separator], package)
    return exact_identity(
        package=package,
        target_kind=target_kind,
        target_name=target_name,
        full_name=full_name_overrides.get(test_id, full_name),
    )


def _nextest_identity(
    test_id: str,
    tokens: list[str],
    *,
    full_name_overrides: Mapping[str, str],
) -> dict[str, Any]:
    try:
        package = tokens[tokens.index("-p") + 1]
        expression = tokens[tokens.index("-E") + 1]
    except (ValueError, IndexError) as error:
        raise ValueError(f"{test_id} lacks exact nextest fields") from error
    target_kind, target_name = _target(tokens, package)
    binary_match = re.search(r"binary_id\(=([^)]*)\)", expression)
    test_match = re.search(r"test\(([=~])([^)]*)\)", expression)
    if binary_match is None or test_match is None:
        raise ValueError(f"{test_id} nextest expression is unsupported")
    result = exact_identity(
        package=package,
        target_kind=target_kind,
        target_name=target_name,
        full_name=full_name_overrides.get(test_id, test_match.group(2)),
    )
    if result["binary_id"] != binary_match.group(1):
        raise ValueError(f"{test_id} nextest binary identity drifted")
    return result


def _target(
    tokens: list[str],
    package: str,
) -> tuple[str, str | None]:
    if "--test" in tokens:
        index = tokens.index("--test")
        return "test", tokens[index + 1]
    if "--bin" in tokens:
        index = tokens.index("--bin")
        return "bin", tokens[index + 1]
    if "--lib" in tokens:
        return "lib", None
    return "lib", None
