"""Exact Cargo target and feature shards for focused Rust coverage traces."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .coverage_trace_resolver import (
    mechanical_rust_target,
    no_default_feature_discovery_ids,
)


MAX_TARGETS_PER_NEXTEST_LIST_COMMAND = 16


def build_targeted_nextest_list_shards(
    facts: Sequence[Mapping[str, Any]],
    cargo_features: Sequence[str] = (),
    *,
    root: Path | None = None,
) -> list[dict[str, Any]]:
    """Group exact facts without mixing packages or feature configurations."""

    groups: dict[
        tuple[str, bool],
        dict[tuple[str, str | None], list[Mapping[str, Any]]],
    ] = defaultdict(lambda: defaultdict(list))
    classified_disabled = (
        no_default_feature_discovery_ids(root=root, facts=facts)
        if root is not None
        else set()
    )
    for fact in facts:
        package = _required_string(fact, "package", "Rust scanner fact")
        discovery_id = _required_string(
            fact, "discovery_id", "Rust scanner fact"
        )
        no_default_features = fact.get(
            "no_default_features",
            discovery_id in classified_disabled,
        )
        if not isinstance(no_default_features, bool):
            raise ValueError(
                "Rust scanner fact no_default_features must be a boolean"
            )
        if root is not None and no_default_features != (
            discovery_id in classified_disabled
        ):
            raise ValueError(
                f"Rust scanner fact feature shard drifted for {discovery_id}"
            )
        target = mechanical_rust_target(fact)
        kind = _required_string(target, "kind", "Rust cargo target")
        name = target.get("name")
        if name is not None and (not isinstance(name, str) or not name):
            raise ValueError("Rust cargo target name must be null or non-empty")
        exact_fact = dict(fact)
        exact_fact["cargo_target_kind"] = kind
        exact_fact["cargo_target_name"] = name
        exact_fact["no_default_features"] = no_default_features
        if "test_prefix" in target:
            exact_fact["nextest_test_prefix"] = target["test_prefix"]
        groups[(package, no_default_features)][(kind, name)].append(exact_fact)

    shards: list[dict[str, Any]] = []
    for (package, no_default_features), package_groups in sorted(groups.items()):
        targets: list[dict[str, Any]] = []
        for (kind, name), group_facts in sorted(
            package_groups.items(),
            key=lambda item: (item[0][0], item[0][1] or ""),
        ):
            targets.append(
                {
                    "cargo_target_kind": kind,
                    "cargo_target_name": name,
                    "facts": [
                        dict(fact)
                        for fact in sorted(
                            group_facts,
                            key=lambda row: str(row.get("discovery_id", "")),
                        )
                    ],
                }
            )
        for offset in range(
            0,
            len(targets),
            MAX_TARGETS_PER_NEXTEST_LIST_COMMAND,
        ):
            chunk = targets[
                offset : offset + MAX_TARGETS_PER_NEXTEST_LIST_COMMAND
            ]
            command = [
                "cargo",
                "nextest",
                "list",
                "--run-ignored",
                "all",
                "--message-format",
                "json",
                "-p",
                package,
            ]
            if cargo_features:
                command.extend(["--features", ",".join(cargo_features)])
            if no_default_features:
                command.append("--no-default-features")
            for target in chunk:
                command.extend(
                    cargo_target_args(
                        kind=str(target["cargo_target_kind"]),
                        name=target["cargo_target_name"],
                    )
                )
            shards.append(
                {
                    "package": package,
                    "no_default_features": no_default_features,
                    "targets": chunk,
                    "command": command,
                }
            )
    return shards


def cargo_target_args(*, kind: str, name: Any) -> list[str]:
    """Return the exact Cargo selector for one resolved target."""

    if kind == "lib":
        if name is not None:
            raise ValueError("library target must not carry a target name")
        return ["--lib"]
    if kind not in {"test", "bin", "example", "bench"}:
        raise ValueError(f"unsupported exact cargo target kind {kind!r}")
    if not isinstance(name, str) or not name:
        raise ValueError(f"{kind} target requires a target name")
    return [f"--{kind}", name]


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
