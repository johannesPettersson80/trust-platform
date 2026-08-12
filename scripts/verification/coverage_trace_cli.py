"""Focused Rust per-test coverage trace orchestration."""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import shlex
import subprocess
import sys
import tomllib
from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .code_spec_live import load_code_spec_metadata
from .coverage_trace import validate_trace_document
from .coverage_trace_config import render_nextest_tool_config
from .coverage_trace_document import build_rust_trace_document
from .coverage_trace_attribution import executed_repository_paths
from .coverage_trace_evidence import capture_raw_trace_evidence
from .coverage_trace_direct import parse_llvm_cov_environment
from .coverage_trace_filter import exact_name_matcher
from .coverage_trace_inputs import (
    focused_rust_trace_input_provenance,
    trace_input_digest,
)
from .coverage_trace_resolver import (
    mechanical_resolved_rust_command,
    resolve_rust_tests,
)
from .coverage_trace_resume import reusable_completed_attempt_ids
from .coverage_trace_shards import (
    MAX_TARGETS_PER_NEXTEST_LIST_COMMAND,
    build_targeted_nextest_list_shards,
    cargo_target_args as _cargo_target_args,
)
from .coverage_trace_tools import export_attempt_coverage
from .focused_test_facts import (
    focused_rust_input_digest,
    resolve_focused_rust_facts,
)
from .focused_trace_receipt import write_authenticated_trace_receipt
from .mapping_live_code_facts import (
    focused_rust_path_inventory,
    focused_rust_observed_source_projection,
)
from .test_catalog_denominator import load_test_catalog_denominator
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_scanner import scan_repository


MAX_TESTS_PER_COVERAGE_COMMAND = 64
LIBRARY_TARGET_KINDS = {
    "lib",
    "rlib",
    "dylib",
    "cdylib",
    "staticlib",
    "proc-macro",
}


def rust_package_target_inventory(
    document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    """Extract exact test-capable library and binary targets by package."""

    packages = document.get("packages")
    if not isinstance(packages, Sequence) or isinstance(packages, (str, bytes)):
        raise ValueError("cargo metadata lacks packages array")
    inventory: dict[str, dict[str, Any]] = {}
    for package in packages:
        if not isinstance(package, Mapping):
            raise ValueError("cargo metadata package must be an object")
        name = _required_string(package, "name", "cargo metadata package")
        if name in inventory:
            raise ValueError(f"cargo metadata duplicates package name {name}")
        targets = package.get("targets")
        if not isinstance(targets, Sequence) or isinstance(
            targets, (str, bytes)
        ):
            raise ValueError(f"cargo metadata package {name} lacks targets")
        has_library = False
        binary_names: set[str] = set()
        for target in targets:
            if not isinstance(target, Mapping):
                raise ValueError(f"cargo metadata package {name} target is invalid")
            kinds = target.get("kind")
            if not isinstance(kinds, Sequence) or isinstance(
                kinds, (str, bytes)
            ) or any(not isinstance(kind, str) for kind in kinds):
                raise ValueError(
                    f"cargo metadata package {name} target kinds are invalid"
                )
            if target.get("test") is False:
                continue
            if any(kind in LIBRARY_TARGET_KINDS for kind in kinds):
                has_library = True
            if "bin" in kinds:
                binary_names.add(
                    _required_string(target, "name", f"cargo target in {name}")
                )
        inventory[name] = {
            "package_has_library": has_library,
            "package_binary_names": sorted(binary_names),
        }
    return dict(sorted(inventory.items()))


def targeted_nextest_list_shards(
    facts: Sequence[Mapping[str, Any]],
    cargo_features: Sequence[str] = (),
    *,
    root: Path | None = None,
) -> list[dict[str, Any]]:
    return build_targeted_nextest_list_shards(
        facts,
        cargo_features,
        root=root,
    )


def resolve_nextest_list_shard(
    *,
    shard: Mapping[str, Any],
    nextest_document: Mapping[str, Any],
    cargo_features: Sequence[str] = (),
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Resolve one batched list document exactly once."""

    targets = shard.get("targets")
    if not isinstance(targets, Sequence) or isinstance(targets, (str, bytes)):
        raise ValueError("nextest list shard targets must be an array")
    facts: list[Mapping[str, Any]] = []
    for target in targets:
        if not isinstance(target, Mapping):
            raise ValueError("nextest list shard target must be an object")
        target_facts = target.get("facts")
        if not isinstance(target_facts, Sequence) or isinstance(
            target_facts,
            (str, bytes),
        ):
            raise ValueError("nextest list shard target facts must be an array")
        if any(not isinstance(fact, Mapping) for fact in target_facts):
            raise ValueError("nextest list shard facts must be objects")
        facts.extend(target_facts)
    resolved, unsupported = resolve_rust_tests(
        facts=facts,
        nextest_document=nextest_document,
    )
    for record in resolved:
        record["command"] = mechanical_resolved_rust_command(
            record,
            cargo_features,
        )
    return resolved, unsupported


def coverage_run_commands(
    *,
    resolved: Sequence[Mapping[str, Any]],
    tool_config: str,
    cargo_features: Sequence[str] = (),
) -> list[list[str]]:
    if not tool_config:
        raise ValueError("coverage run commands require tool config")
    groups: dict[tuple[str, str, str | None, bool], list[Mapping[str, Any]]] = (
        defaultdict(list)
    )
    for record in resolved:
        package = _required_string(record, "package", "resolved test")
        kind = _required_string(record, "cargo_target_kind", "resolved test")
        name = record.get("cargo_target_name")
        if name is not None and (not isinstance(name, str) or not name):
            raise ValueError("resolved test cargo target name must be null or non-empty")
        _cargo_target_args(kind=kind, name=name)
        no_default_features = record.get("no_default_features", False)
        if not isinstance(no_default_features, bool):
            raise ValueError(
                "resolved test no_default_features must be a boolean"
            )
        groups[(package, kind, name, no_default_features)].append(record)
    commands: list[list[str]] = []
    for (package, kind, name, no_default_features), records in sorted(
        groups.items(),
        key=lambda item: (
            item[0][0],
            item[0][1],
            item[0][2] or "",
            item[0][3],
        ),
    ):
        target_args = _cargo_target_args(kind=kind, name=name)
        ordered = sorted(
            records,
            key=lambda row: (
                str(row.get("nextest_binary_id", "")),
                str(row.get("nextest_test_name", "")),
            ),
        )
        for offset in range(0, len(ordered), MAX_TESTS_PER_COVERAGE_COMMAND):
            chunk = ordered[offset : offset + MAX_TESTS_PER_COVERAGE_COMMAND]
            feature_args = (
                ["--features", ",".join(cargo_features)]
                if cargo_features
                else []
            )
            commands.append(
                [
                    "cargo",
                    "llvm-cov",
                    "nextest",
                    "--no-report",
                    "-p",
                    package,
                    *target_args,
                    *(["--no-default-features"] if no_default_features else []),
                    *feature_args,
                    "--tool-config-file",
                    f"trust-catalog:{tool_config}",
                    "-P",
                    "catalog-trace",
                    "-E",
                    nextest_filter_expression(chunk),
                ]
            )
    return commands


def trace_environment(
    *,
    base_environment: Mapping[str, str],
    cargo_target_dir: Path,
    temporary_dir: Path,
) -> dict[str, str]:
    environment = dict(base_environment)
    resolved_target = cargo_target_dir.resolve()
    coverage_target = (resolved_target / "llvm-cov-target").resolve()
    environment["CARGO_TARGET_DIR"] = str(resolved_target)
    environment["CARGO_LLVM_COV_TARGET_DIR"] = str(coverage_target)
    environment["CARGO_LLVM_COV_BUILD_DIR"] = str(coverage_target)
    resolved_temporary = temporary_dir.resolve()
    resolved_temporary.mkdir(parents=True, exist_ok=True)
    environment["TMPDIR"] = str(resolved_temporary)
    return environment


def instrumented_nextest_list_environment(
    *,
    base_environment: Mapping[str, str],
    llvm_cov_environment: Mapping[str, str],
    cargo_target_dir: Path,
) -> dict[str, str]:
    """Build list identities with the same coverage executable as trace runs."""

    environment = dict(base_environment)
    environment.update(llvm_cov_environment)
    coverage_target = (cargo_target_dir.resolve() / "llvm-cov-target").resolve()
    coverage_target.mkdir(parents=True, exist_ok=True)
    environment["CARGO_TARGET_DIR"] = str(coverage_target)
    environment["CARGO_LLVM_COV_TARGET_DIR"] = str(coverage_target)
    environment["CARGO_LLVM_COV_BUILD_DIR"] = str(coverage_target)
    return environment


def nextest_filter_expression(
    resolved: Sequence[Mapping[str, Any]],
) -> str:
    identities: set[tuple[str, str]] = set()
    expressions: list[str] = []
    for record in sorted(
        resolved,
        key=lambda row: (
            str(row.get("nextest_binary_id", "")),
            str(row.get("nextest_test_name", "")),
        ),
    ):
        binary_id = _required_string(record, "nextest_binary_id", "resolved test")
        test_name = _required_string(record, "nextest_test_name", "resolved test")
        identity = (binary_id, test_name)
        if identity in identities:
            raise ValueError(f"resolved tests duplicate runtime identity {identity}")
        identities.add(identity)
        expressions.append(
            f"(binary_id({exact_name_matcher(binary_id)}) & "
            f"test({exact_name_matcher(test_name)}))"
        )
    if not expressions:
        raise ValueError("nextest filter requires at least one resolved test")
    return " | ".join(expressions)


def requested_tests_document(
    resolved: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    tests = [
        {
            "discovery_id": _required_string(record, "discovery_id", "resolved test"),
            "nextest_binary_id": _required_string(
                record, "nextest_binary_id", "resolved test"
            ),
            "nextest_binary_path": _required_string(
                record, "nextest_binary_path", "resolved test"
            ),
            "nextest_test_name": _required_string(
                record, "nextest_test_name", "resolved test"
            ),
        }
        for record in sorted(
            resolved, key=lambda row: str(row.get("discovery_id", ""))
        )
    ]
    return {"schema_version": 2, "tests": tests}


def legacy_requested_tests_document(
    resolved: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Exact v1 projection accepted only for one-way resume migration."""

    tests = [
        {
            "discovery_id": _required_string(
                record, "discovery_id", "resolved test"
            ),
            "nextest_binary_id": _required_string(
                record, "nextest_binary_id", "resolved test"
            ),
            "nextest_test_name": _required_string(
                record, "nextest_test_name", "resolved test"
            ),
        }
        for record in sorted(
            resolved, key=lambda row: str(row.get("discovery_id", ""))
        )
    ]
    return {"schema_version": 1, "tests": tests}


def validate_resume_request(
    *,
    existing: Mapping[str, Any],
    current: Mapping[str, Any],
    legacy: Mapping[str, Any],
) -> None:
    """Validate stored request shape before reconciling it with live tests.

    Resume reconciliation is performed per attempt by
    ``reusable_completed_attempt_ids``.  The live denominator may legitimately
    grow, shrink, or change identity between commits; requiring whole-document
    equality here would prevent that exact reconciliation from running.
    """

    if existing in (current, legacy):
        return
    if set(existing) != {"schema_version", "tests"}:
        raise ValueError("existing Rust resume request root fields drift")
    schema_version = existing.get("schema_version")
    if schema_version not in {1, 2}:
        raise ValueError("existing Rust resume request schema_version is unsupported")
    tests = existing.get("tests")
    if not isinstance(tests, list):
        raise ValueError("existing Rust resume request tests must be an array")
    required_fields = {
        "discovery_id",
        "nextest_binary_id",
        "nextest_test_name",
    }
    if schema_version == 2:
        required_fields.add("nextest_binary_path")
    discovery_ids: set[str] = set()
    runtime_identities: set[tuple[str, ...]] = set()
    for index, record in enumerate(tests):
        if not isinstance(record, Mapping) or set(record) != required_fields:
            raise ValueError(
                f"existing Rust resume request test[{index}] fields drift"
            )
        for field in sorted(required_fields):
            value = record.get(field)
            if not isinstance(value, str) or not value:
                raise ValueError(
                    f"existing Rust resume request test[{index}] "
                    f"requires non-empty {field}"
                )
        discovery_id = str(record["discovery_id"])
        if discovery_id in discovery_ids:
            raise ValueError(
                f"existing Rust resume request duplicates {discovery_id}"
            )
        discovery_ids.add(discovery_id)
        runtime_identity = tuple(
            str(record[field])
            for field in (
                "nextest_binary_id",
                "nextest_test_name",
                *(("nextest_binary_path",) if schema_version == 2 else ()),
            )
        )
        if runtime_identity in runtime_identities:
            raise ValueError(
                "existing Rust resume request duplicates runtime identity "
                f"{runtime_identity}"
            )
        runtime_identities.add(runtime_identity)


def select_ground_truth_pilot_ids(
    *,
    tests: Mapping[str, Mapping[str, Any]],
    live_discovery_ids: set[str],
    count: int,
) -> list[str]:
    if count < 1:
        raise ValueError("ground-truth pilot count must be positive")
    by_area: dict[str, list[tuple[str, str]]] = defaultdict(list)
    for test_id, record in tests.items():
        discovery_id = record.get("discovery_id")
        if (
            record.get("subject_kind") == "generated_test"
            and record.get("discovery_source_kind")
            in {"rust_unit_test", "rust_integration_test"}
            and isinstance(discovery_id, str)
            and discovery_id in live_discovery_ids
            and isinstance(record.get("invariants"), list)
            and record["invariants"]
        ):
            by_area[str(record.get("area", "unmapped"))].append(
                (test_id, discovery_id)
            )
    for rows in by_area.values():
        rows.sort()
    selected: list[str] = []
    while len(selected) < count and any(by_area.values()):
        for area in sorted(by_area):
            if by_area[area] and len(selected) < count:
                _, discovery_id = by_area[area].pop(0)
                selected.append(discovery_id)
    if len(selected) != count:
        raise ValueError(
            f"requested {count} ground-truth tests but only {len(selected)} are eligible"
        )
    return sorted(selected)


def resolve_requested_trace_facts(
    *,
    root: Path,
    discovery_ids: Sequence[str],
) -> list[Any]:
    """Resolve a focused Rust shard without scanning unrelated test files."""

    return resolve_focused_rust_facts(
        root=root,
        discovery_ids=discovery_ids,
        denominator=load_test_catalog_denominator(root),
    )


def run_focused_rust_trace(
    *,
    root: Path,
    discovery_ids: Sequence[str],
    trace_root: Path,
    output_path: Path,
    cargo_target_dir: Path,
    llvm_tools_dir: Path,
    temporary_dir: Path,
    resume: bool = False,
    source_facts: Sequence[Any] | None = None,
    source_input_digest: str | None = None,
    cargo_features: Sequence[str] = (),
    authentication_receipt_path: Path | None = None,
) -> dict[str, Any]:
    root = root.resolve()
    cargo_features = _normalized_cargo_features(cargo_features)
    trace_root = trace_root.resolve()
    output_path = output_path.resolve()
    if trace_root.exists() and any(trace_root.iterdir()) and not resume:
        raise ValueError("trace root must be absent or empty for an exact focused run")
    trace_root.mkdir(parents=True, exist_ok=True)
    if source_facts is not None:
        live_facts = list(source_facts)
        if not isinstance(source_input_digest, str) or not re.fullmatch(
            r"sha256:[0-9a-f]{64}",
            source_input_digest,
        ):
            raise ValueError("supplied trace facts require an exact input digest")
    else:
        denominator = load_test_catalog_denominator(root)
        live_facts = resolve_focused_rust_facts(
            root=root,
            discovery_ids=discovery_ids,
            denominator=denominator,
        )
        source_input_digest = focused_rust_input_digest(
            root=root,
            facts=live_facts,
            denominator=denominator,
        )
    facts = {
        fact.stable_id: {
            "discovery_id": fact.stable_id,
            "discovery_source_kind": fact.source_kind,
            "path": fact.path,
            "name": fact.name,
            "ignore_state": fact.ignore_state,
            "package": fact.package,
        }
        for fact in live_facts
    }
    requested_ids = sorted(set(discovery_ids))
    if len(requested_ids) != len(discovery_ids):
        raise ValueError("focused trace discovery IDs must be unique")
    unknown = sorted(set(requested_ids) - set(facts))
    if unknown:
        raise ValueError(f"focused trace names unknown discovery IDs: {unknown}")
    requested_facts = [facts[discovery_id] for discovery_id in requested_ids]
    rust_facts = [
        fact
        for fact in requested_facts
        if fact["discovery_source_kind"]
        in {"rust_unit_test", "rust_integration_test"}
    ]

    environment = trace_environment(
        base_environment=os.environ,
        cargo_target_dir=cargo_target_dir,
        temporary_dir=temporary_dir,
    )
    resolved: list[dict[str, Any]] = []
    unsupported: list[dict[str, Any]] = []
    if rust_facts:
        metadata_result = _run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            root=root,
            environment=environment,
        )
        try:
            cargo_metadata = json.loads(metadata_result.stdout)
        except json.JSONDecodeError as error:
            raise ValueError("cargo metadata emitted invalid JSON") from error
        if not isinstance(cargo_metadata, Mapping):
            raise ValueError("cargo metadata root must be an object")
        llvm_cov_environment = parse_llvm_cov_environment(
            _run(
                ["cargo", "llvm-cov", "show-env"],
                root=root,
                environment=environment,
            ).stdout
        )
        list_environment = instrumented_nextest_list_environment(
            base_environment=environment,
            llvm_cov_environment=llvm_cov_environment,
            cargo_target_dir=cargo_target_dir,
        )
        package_targets = rust_package_target_inventory(cargo_metadata)
        for fact in rust_facts:
            package = str(fact["package"])
            target_inventory = package_targets.get(package)
            if target_inventory is None:
                raise ValueError(
                    f"cargo metadata lacks scanner package {package}"
                )
            fact.update(target_inventory)
        for shard in targeted_nextest_list_shards(
            rust_facts,
            cargo_features,
            root=root,
        ):
            listed = _run(
                list(shard["command"]),
                root=root,
                environment=list_environment,
            )
            try:
                nextest_document = json.loads(listed.stdout)
            except json.JSONDecodeError as error:
                raise ValueError("cargo nextest list emitted invalid JSON") from error
            if not isinstance(nextest_document, Mapping):
                raise ValueError("cargo nextest list root must be an object")
            shard_resolved, shard_unsupported = resolve_nextest_list_shard(
                shard=shard,
                nextest_document=nextest_document,
                cargo_features=cargo_features,
            )
            resolved.extend(shard_resolved)
            unsupported.extend(shard_unsupported)
        resolved.sort(key=lambda row: str(row["discovery_id"]))
        unsupported.sort(key=lambda row: str(row["discovery_id"]))
    for fact in requested_facts:
        if fact["discovery_source_kind"] not in {
            "rust_unit_test",
            "rust_integration_test",
        }:
            unsupported.append(
                {
                    **fact,
                    "reason": "source_kind_not_rust",
                    "candidate_test_names": [],
                }
            )

    if resolved:
        request_document = requested_tests_document(resolved)
        request_path = trace_root / "requested-tests.json"
        if resume and request_path.is_file():
            existing_request = json.loads(request_path.read_text())
            if not isinstance(existing_request, Mapping):
                raise ValueError("resume request root must be an object")
            validate_resume_request(
                existing=existing_request,
                current=request_document,
                legacy=legacy_requested_tests_document(resolved),
            )
        _write_json(request_path, request_document)
        tool_config = trace_root / "nextest.toml"
        rendered_tool_config = render_nextest_tool_config(
            (root / "scripts/verification/coverage_trace_wrapper.py").resolve()
        )
        if resume and tool_config.is_file() and tool_config.read_text() != rendered_tool_config:
            raise ValueError("resume nextest tool configuration drifted")
        tool_config.write_text(rendered_tool_config)
        environment["TRUST_CATALOG_TRACE_ROOT"] = str(trace_root)
        completed_ids = (
            reusable_completed_attempt_ids(
                trace_root=trace_root,
                resolved=resolved,
                purge_incomplete=resume,
            )
            if resume
            else set()
        )
        pending = [
            record
            for record in resolved
            if record["discovery_id"] not in completed_ids
        ]
        run_commands = (
            coverage_run_commands(
                resolved=pending,
                tool_config=str(tool_config),
                cargo_features=cargo_features,
            )
            if pending
            else []
        )
        for run_command in run_commands:
            _run(run_command, root=root, environment=environment)

        metadata = load_code_spec_metadata(root)
        mapped_rust_paths = sorted(
            {
                str(mapping["path"])
                for mapping in metadata.mappings.values()
                if mapping.get("source_kind") == "rust"
                and isinstance(mapping.get("path"), str)
                and (root / str(mapping["path"])).is_file()
            }
        )
        sources, _unfiltered_projection_digest = (
            focused_rust_observed_source_projection(
                root,
                mapped_rust_paths,
                metadata.mappings,
            )
        )
        inventory = focused_rust_path_inventory(
            root,
            sorted(sources),
            metadata.mappings,
        )
        exports = export_attempt_coverage(
            root=root,
            trace_root=trace_root,
            llvm_tools_dir=llvm_tools_dir.resolve(),
            source_paths=sorted(sources),
        )
        observed_paths: set[str] = set()

        def observe_export(document: Mapping[str, Any]) -> None:
            observed_paths.update(
                executed_repository_paths(document, root=root)
            )

        raw_evidence = capture_raw_trace_evidence(
            root=root,
            trace_root=trace_root,
            llvm_tools_dir=llvm_tools_dir,
        )
        input_provenance = focused_rust_trace_input_provenance(
            source_input_digest=source_input_digest,
            resolved=resolved,
            code_map=metadata.code_map,
            trace_root=trace_root.relative_to(root).as_posix(),
            raw_evidence_digest=raw_evidence.digest,
            llvm_tool_digests=raw_evidence.tool_digests,
            llvm_tool_paths=raw_evidence.tool_paths,
        )
        document = build_rust_trace_document(
            root=root,
            trace_root=trace_root,
            requested=resolved,
            coverage_exports=exports,
            sources=sources,
            spans=inventory.spans,
            source_commit=_git_head(root),
            platform=f"{platform.system().lower()}-{platform.machine().lower()}",
            input_digest=input_provenance["input_digest"],
            code_inventory_digests={
                "rust_observed_paths": "sha256:" + "0" * 64,
            },
            mode="focused_shard",
            input_provenance=input_provenance,
            coverage_export_observer=observe_export,
        )
        observed_sources, source_projection_digest = (
            focused_rust_observed_source_projection(
                root,
                sorted(observed_paths),
                metadata.mappings,
            )
        )
        if any(
            sources[path] != content
            for path, content in observed_sources.items()
        ):
            raise ValueError("covered Rust source changed during trace assembly")
        document["code_inventory_digests"] = {
            "rust_observed_paths": source_projection_digest,
        }
    else:
        document = {
            "schema_version": 1,
            "generator": "test-coverage-trace",
            "generator_version": "1",
            "source_commit": _git_head(root),
            "platform": f"{platform.system().lower()}-{platform.machine().lower()}",
            "scope": {
                "mode": "focused_shard",
                "requested_discovery_ids": requested_ids,
                "source_kinds": sorted(
                    {str(fact["discovery_source_kind"]) for fact in requested_facts}
                ),
            },
            "input_digest": trace_input_digest(source_input_digest, [], {}),
            "code_inventory_digests": {},
            "tests": [],
        }
    if unsupported:
        document["tests"].extend(
            {
                "discovery_id": row["discovery_id"],
                "discovery_source_kind": row["discovery_source_kind"],
                "path": row["path"],
                "name": row["name"],
                "command": "unavailable",
                "command_authority": "unavailable",
                "trace_status": "unsupported",
                "attribution_kind": "unavailable",
                "covered_code_ids": [],
                "limitations": [str(row["reason"])],
            }
            for row in unsupported
        )
        document["tests"] = sorted(
            document["tests"], key=lambda row: row["discovery_id"]
        )
        document["scope"]["requested_discovery_ids"] = requested_ids
        document["scope"]["source_kinds"] = sorted(
            {str(fact["discovery_source_kind"]) for fact in requested_facts}
        )
    failures = validate_trace_document(document, live_facts=facts)
    schema = json.loads(
        (root / "verification/schemas/test-coverage-trace.schema.json").read_text()
    )
    failures.extend(validate_json_schema_instance(document, schema))
    if failures:
        raise ValueError("; ".join(sorted(set(failures))))
    _write_json(output_path, document)
    if authentication_receipt_path is not None:
        if not rust_facts:
            raise ValueError(
                "authentication receipts require focused Rust trace evidence"
            )
        write_authenticated_trace_receipt(
            root=root,
            receipt_path=authentication_receipt_path.resolve(),
            trace=document,
            requested=requested_ids,
            evidence=raw_evidence,
            source_originals=observed_sources,
        )
    return document


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument("--discovery-id", action="append")
    selection.add_argument("--ground-truth-count", type=int)
    selection.add_argument("--all-rust", action="store_true")
    parser.add_argument(
        "--trace-root",
        type=Path,
        default=Path("target/gate-artifacts/verification/catalog-trace"),
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(
            "target/gate-artifacts/verification/test-coverage-trace.json"
        ),
    )
    parser.add_argument("--cargo-target-dir", type=Path, required=True)
    parser.add_argument("--llvm-tools-dir", type=Path, required=True)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument(
        "--cargo-feature",
        action="append",
        default=[],
        help=(
            "enable one Cargo feature while listing and tracing the selected "
            "Rust tests; repeat for multiple features"
        ),
    )
    parser.add_argument(
        "--temporary-dir",
        type=Path,
        default=Path(
            "target/gate-artifacts/verification/catalog-trace-temporary"
        ),
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        discovery_ids = args.discovery_id
        source_facts = None
        source_input_digest = None
        if args.all_rust:
            scan = scan_repository(args.root.resolve())
            source_facts = list(scan.inferred_facts)
            source_input_digest = scan.input_digest
            discovery_ids = sorted(
                fact.stable_id
                for fact in scan.inferred_facts
                if fact.source_kind
                in {"rust_unit_test", "rust_integration_test"}
                and fact.ignore_state != "ignored"
            )
        elif args.ground_truth_count is not None:
            metadata = load_code_spec_metadata(args.root.resolve())
            denominator = load_test_catalog_denominator(args.root.resolve())
            reviews = denominator.get("reviews", [])
            if not isinstance(reviews, list):
                raise ValueError("test-catalog denominator reviews are malformed")
            discovery_ids = select_ground_truth_pilot_ids(
                tests=metadata.tests,
                live_discovery_ids={
                    str(review["discovery_id"])
                    for review in reviews
                    if isinstance(review, Mapping)
                    and isinstance(review.get("discovery_id"), str)
                },
                count=args.ground_truth_count,
            )
        document = run_focused_rust_trace(
            root=args.root,
            discovery_ids=discovery_ids,
            trace_root=args.trace_root,
            output_path=args.out,
            cargo_target_dir=args.cargo_target_dir,
            llvm_tools_dir=args.llvm_tools_dir,
            temporary_dir=args.temporary_dir,
            resume=args.resume,
            source_facts=source_facts,
            source_input_digest=source_input_digest,
            cargo_features=args.cargo_feature,
        )
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"focused coverage trace failed: {error}", file=sys.stderr)
        return 2
    observed = sum(
        record["trace_status"] == "observed" for record in document["tests"]
    )
    print(
        f"focused coverage trace complete: {observed}/{len(document['tests'])} "
        "tests observed"
    )
    return 0


def _run(
    command: list[str],
    *,
    root: Path,
    environment: Mapping[str, str],
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=root,
        env=dict(environment),
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise ValueError(f"command failed ({completed.returncode}): {detail}")
    return completed


def _git_head(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    head = completed.stdout.strip()
    if completed.returncode != 0 or not re.fullmatch(r"[0-9a-f]{40}", head):
        raise ValueError("focused trace cannot resolve source commit")
    return head


def _write_json(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    rendered = json.dumps(document, indent=2, sort_keys=True) + "\n"
    path.write_text(rendered)


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _normalized_cargo_features(features: Sequence[str]) -> tuple[str, ...]:
    normalized = []
    for feature in features:
        if (
            not isinstance(feature, str)
            or not feature
            or any(character.isspace() or character == "," for character in feature)
        ):
            raise ValueError(
                "focused trace Cargo features must be non-empty names "
                "without whitespace or commas"
            )
        normalized.append(feature)
    return tuple(sorted(set(normalized)))
