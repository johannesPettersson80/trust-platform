"""Run and project one frozen runtime protocol/I/O nextest batch."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shlex
import subprocess
import tempfile
import tomllib
from collections import defaultdict
from collections.abc import Mapping, Sequence
from decimal import Decimal, InvalidOperation, ROUND_CEILING
from pathlib import Path
from typing import Any

from .frozen_catalog_identity import digest_document, selection_digest
from .frozen_rust_nextest_target import (
    ADS_SERVER_CFG,
    ADS_WIRE_DISABLED_CFG,
    FrozenRustTargetProfile,
    get_target_profile,
    no_default_feature_test_names,
)

CATALOG_PATH = Path("verification/test-catalog.toml")
DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
RUNNER_PATH = Path("scripts/verification/frozen_runtime_protocol_io_batch.py")
MAPPING_DEBT_REASON = "no_reviewed_spec_or_invariant_binding"
EXPERIMENTAL_ENVIRONMENT = {"NEXTEST_EXPERIMENTAL_LIBTEST_JSON": "1"}
TERMINAL_EVENTS = {"ok", "failed", "ignored", "timeout"}


def run_batch(
    *,
    root: Path,
    inventory_path: Path,
    assignments_path: Path,
    output_dir: Path,
    reviewed_date: str,
    normalize_only: bool = False,
    mixed_debug_runtime_core: bool = False,
) -> dict[str, Any]:
    _require_clean_checkout(root)
    inventory_document = _json(inventory_path)
    assignments_document = _json(assignments_path)
    selection = _selection(
        root=root,
        inventory_document=inventory_document,
        assignments_document=assignments_document,
    )
    shards = (
        [("default", list(selection), False)]
        if mixed_debug_runtime_core
        else selected_feature_shards(root=root, selection=selection)
    )
    output_dir.mkdir(parents=True, exist_ok=True)
    list_path = output_dir / "nextest-list.json"
    raw_path = output_dir / "runtime-protocol-io.jsonl"
    metadata_path = output_dir / "execution.json"
    plans_dir = output_dir / "plans"
    results_dir = output_dir / "results"
    if normalize_only:
        if not list_path.is_file() or not raw_path.is_file():
            raise ValueError(
                "normalize-only requires existing nextest list and raw results"
            )
        if any(path.exists() for path in (metadata_path, plans_dir, results_dir)):
            raise ValueError(
                "normalize-only refuses partially generated projected outputs"
            )
    elif any(path.exists() for path in (list_path, raw_path, metadata_path)):
        raise ValueError("runtime protocol/I/O execution outputs already exist")

    environment = dict(os.environ)
    environment.update(EXPERIMENTAL_ENVIRONMENT)
    resolved: dict[str, dict[str, str]] = {}
    list_records = []
    commands = []
    recorded_shards: dict[str, Mapping[str, Any]] = {}
    if normalize_only:
        recorded_document = _json(list_path)
        raw_recorded_shards = recorded_document.get("shards")
        if (
            not isinstance(raw_recorded_shards, Sequence)
            or isinstance(raw_recorded_shards, (str, bytes))
        ):
            raise ValueError("recorded nextest list lacks feature shards")
        recorded_shards = {
            _string(row, "name", "recorded feature shard"): row
            for row in raw_recorded_shards
            if isinstance(row, Mapping)
        }
        if len(recorded_shards) != len(raw_recorded_shards):
            raise ValueError("recorded nextest feature shard names are invalid")
    for shard_name, shard_selection, no_default_features in shards:
        target_args = (
            [
                "-p",
                "trust-debug",
                "-p",
                "trust-runtime-core",
                "-p",
                "trust-hir",
                "-p",
                "trust-runtime",
                "--lib",
                "--test",
                "semantic_type_checking",
                "--test",
                "bounded_value_semantics",
                "--test",
                "stdlib_select",
            ]
            if mixed_debug_runtime_core
            else [
                "-p",
                "trust-runtime",
                *selected_target_args(shard_selection, root=root),
            ]
        )
        feature_args = ["--no-default-features"] if no_default_features else []
        list_command = [
            "cargo",
            "nextest",
            "list",
            *target_args,
            *feature_args,
            "--message-format",
            "json",
        ]
        if normalize_only:
            record = recorded_shards.get(shard_name)
            if not isinstance(record, Mapping):
                raise ValueError(
                    f"recorded nextest list lacks {shard_name} shard"
                )
            if (
                record.get("command") != list_command
                or record.get("no_default_features") is not no_default_features
                or record.get("selected_test_count") != len(shard_selection)
            ):
                raise ValueError(
                    f"recorded nextest {shard_name} shard selection drifted"
                )
            listing = record.get("listing")
            if not isinstance(listing, Mapping):
                raise ValueError(
                    f"recorded nextest {shard_name} listing is invalid"
                )
        else:
            listed = subprocess.run(
                list_command,
                cwd=root,
                check=False,
                capture_output=True,
                text=True,
                env=environment,
            )
            if listed.returncode != 0:
                raise ValueError(
                    f"runtime nextest list failed for {shard_name}: "
                    + (listed.stderr.strip() or f"exit {listed.returncode}")
                )
            listing = json.loads(listed.stdout)
        shard_resolved = resolve_nextest_listing(
            listing=listing,
            selected_names={row["name"] for row in shard_selection},
            expected_packages=(
                {
                    row["name"]: _mixed_target_profile(row["path"]).package
                    for row in shard_selection
                }
                if mixed_debug_runtime_core
                else None
            ),
        )
        overlap = set(resolved) & set(shard_resolved)
        if overlap:
            raise ValueError(
                "runtime feature shards overlap: " + ", ".join(sorted(overlap))
            )
        resolved.update(shard_resolved)
        if not normalize_only:
            list_records.append(
                {
                    "name": shard_name,
                    "command": list_command,
                    "no_default_features": no_default_features,
                    "selected_test_count": len(shard_selection),
                    "listing": listing,
                }
            )
        commands.append(
            build_batch_command(
                shard_resolved,
                target_args=(*target_args, *feature_args),
                include_package=False,
            )
        )
    if normalize_only:
        if set(recorded_shards) != {name for name, _rows, _flag in shards}:
            raise ValueError("recorded nextest feature shard set drifted")
    else:
        _write_json(
            list_path,
            {
                "schema_version": 1,
                "shards": list_records,
            },
        )
    command = combined_batch_command(commands)
    if not normalize_only:
        completed = subprocess.run(
            command,
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )
        _write_text(raw_path, completed.stdout)
        if completed.returncode != 0:
            raise ValueError(
                "runtime protocol/I/O nextest failed: "
                + (completed.stderr.strip() or f"exit {completed.returncode}")
            )
    version = subprocess.run(
        ["cargo", "nextest", "--version"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    return build_outputs(
        root=root,
        inventory_document=inventory_document,
        assignments_document=assignments_document,
        selection=selection,
        resolved=resolved,
        command=command,
        list_path=list_path,
        raw_path=raw_path,
        metadata_path=metadata_path,
        plans_dir=plans_dir,
        results_dir=results_dir,
        nextest_version=version,
        reviewed_date=reviewed_date,
        mixed_debug_runtime_core=mixed_debug_runtime_core,
        no_default_feature_names={
            row["name"]
            for _shard_name, rows, disabled in shards
            if disabled
            for row in rows
        },
    )


def resolve_nextest_listing(
    *,
    listing: Mapping[str, Any],
    selected_names: set[str],
    expected_packages: Mapping[str, str] | None = None,
) -> dict[str, dict[str, str]]:
    if expected_packages is not None and set(expected_packages) != selected_names:
        raise ValueError(
            "nextest expected-package keys do not equal selected names"
        )
    suites = listing.get("rust-suites")
    if not isinstance(suites, Mapping):
        raise ValueError("nextest listing lacks rust-suites")
    candidates: dict[str, list[dict[str, str]]] = defaultdict(list)
    for suite_key, raw_suite in suites.items():
        if not isinstance(suite_key, str) or not isinstance(raw_suite, Mapping):
            raise ValueError("nextest listing suite is invalid")
        binary_id = raw_suite.get("binary-id", suite_key)
        if not isinstance(binary_id, str) or not binary_id:
            raise ValueError("nextest listing binary ID is invalid")
        package_name = raw_suite.get("package-name")
        binary_name = raw_suite.get("binary-name")
        if (
            not isinstance(package_name, str)
            or not package_name
            or not isinstance(binary_name, str)
            or not binary_name
        ):
            raise ValueError(
                f"nextest suite {binary_id} lacks package or binary name"
            )
        event_suite_id = f"{package_name}::{binary_name}"
        testcases = raw_suite.get("testcases")
        if not isinstance(testcases, Mapping):
            raise ValueError(f"nextest suite {binary_id} lacks testcases")
        for full_name in testcases:
            if not isinstance(full_name, str) or not full_name:
                raise ValueError(f"nextest suite {binary_id} has invalid test name")
            bare = full_name.rsplit("::", 1)[-1]
            expected_package = (
                expected_packages.get(bare)
                if expected_packages is not None
                else None
            )
            if bare in selected_names and (
                expected_package is None or package_name == expected_package
            ):
                candidates[bare].append(
                    {
                        "binary_id": binary_id,
                        "full_name": full_name,
                        "encoded_name": f"{event_suite_id}${full_name}",
                        "package": package_name,
                    }
                )
    missing = sorted(selected_names - set(candidates))
    ambiguous = sorted(name for name, rows in candidates.items() if len(rows) != 1)
    if missing or ambiguous:
        raise ValueError(
            f"nextest listing resolution failed missing={missing} ambiguous={ambiguous}"
        )
    return {name: rows[0] for name, rows in candidates.items()}


def build_batch_command(
    resolved: Mapping[str, Mapping[str, str]],
    *,
    target_args: Sequence[str] = (),
    include_package: bool = True,
) -> list[str]:
    clauses = []
    for name, row in sorted(resolved.items()):
        binary_id = _string(row, "binary_id", name)
        full_name = _string(row, "full_name", name)
        clauses.append(
            f"(binary_id(={binary_id}) & test(={full_name}))"
        )
    if not clauses:
        raise ValueError("runtime protocol/I/O selection is empty")
    return [
        "cargo",
        "nextest",
        "run",
        *(("-p", "trust-runtime") if include_package else ()),
        *target_args,
        "--no-fail-fast",
        "--message-format",
        "libtest-json-plus",
        "--message-format-version",
        "0.1",
        "--user-config-file",
        "none",
        "-E",
        " | ".join(clauses),
    ]


def combined_batch_command(commands: Sequence[Sequence[str]]) -> list[str]:
    """Return the exact shell command used to execute all feature shards."""

    if not commands:
        raise ValueError("runtime protocol/I/O execution has no feature shards")
    if len(commands) == 1:
        return list(commands[0])
    return ["sh", "-c", " && ".join(shlex.join(command) for command in commands)]


def selected_feature_shards(
    *,
    root: Path,
    selection: Sequence[Mapping[str, str]],
) -> list[tuple[str, list[dict[str, str]], bool]]:
    """Split selected tests using their exact source cfg attributes."""

    disabled_names: set[str] = set()
    rows_by_path: dict[str, list[Mapping[str, str]]] = defaultdict(list)
    for row in selection:
        rows_by_path[_string(row, "path", "selection")].append(row)
    for path, rows in rows_by_path.items():
        profile = FrozenRustTargetProfile(
            key="runtime-protocol-io-source",
            package="trust-runtime",
            target_kind="test",
            target_name=None,
            binary_id="source-classification-only",
            test_prefix="",
            encoded_test_name_prefix="",
            discovery_source_kind="rust_integration_test",
            allowed_cfg_attributes=frozenset(
                {
                    ADS_WIRE_DISABLED_CFG,
                    ADS_SERVER_CFG,
                    '#[cfg(feature = "ads-wire")]',
                    '#[cfg(feature = "opcua-wire")]',
                    "#[cfg(unix)]",
                    '#[cfg(all(feature = "ethercat-wire", unix))]',
                }
            ),
            no_default_features_cfg=ADS_WIRE_DISABLED_CFG,
            no_default_features_shard_name="no-ads-wire",
        )
        disabled_names.update(
            no_default_feature_test_names(
                profile=profile,
                source=(root / path).read_text(),
                test_names=tuple(
                    sorted(_string(row, "name", path) for row in rows)
                ),
            )
        )
    default_rows = [
        dict(row) for row in selection if row["name"] not in disabled_names
    ]
    disabled_rows = [
        dict(row) for row in selection if row["name"] in disabled_names
    ]
    shards = []
    if disabled_rows:
        shards.append(("no-ads-wire", disabled_rows, True))
    if default_rows:
        shards.append(("default", default_rows, False))
    return shards


def selected_target_args(
    selection: Sequence[Mapping[str, str]],
    *,
    root: Path | None = None,
) -> list[str]:
    """Select only the library and named integration targets in the batch."""

    has_lib = False
    has_runtime_bin = False
    integration_targets: set[str] = set()
    prefix = "crates/trust-runtime/tests/"
    for row in selection:
        path = _string(row, "path", "selection")
        if path.startswith(prefix):
            relative = path.removeprefix(prefix)
            if "/" in relative:
                module_root = relative.split("/", 1)[0]
                target_source = (
                    root / prefix / f"{module_root}.rs"
                    if root is not None
                    else None
                )
                if target_source is None or not target_source.is_file():
                    raise ValueError(
                        f"unsupported runtime integration path {path}"
                    )
                integration_targets.add(module_root)
            elif relative.endswith(".rs"):
                integration_targets.add(relative.removesuffix(".rs"))
            else:
                raise ValueError(f"unsupported runtime integration path {path}")
        elif path.startswith(
            "crates/trust-runtime/src/bin/trust-runtime/"
        ):
            has_runtime_bin = True
        elif path.startswith("crates/trust-runtime/src/"):
            has_lib = True
        else:
            raise ValueError(f"unsupported runtime test path {path}")
    args = ["--lib"] if has_lib else []
    if has_runtime_bin:
        args.extend(["--bin", "trust-runtime"])
    for target in sorted(integration_targets):
        args.extend(["--test", target])
    if not args:
        raise ValueError("runtime target selection is empty")
    return args


def build_outputs(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    assignments_document: Mapping[str, Any],
    selection: Sequence[Mapping[str, str]],
    resolved: Mapping[str, Mapping[str, str]],
    command: Sequence[str],
    list_path: Path,
    raw_path: Path,
    metadata_path: Path,
    plans_dir: Path,
    results_dir: Path,
    nextest_version: str,
    reviewed_date: str,
    no_default_feature_names: set[str],
    mixed_debug_runtime_core: bool = False,
) -> dict[str, Any]:
    raw = raw_path.read_text()
    assignments = {
        (row["path"], row["name"]): row
        for row in assignments_document["assignments"]
    }
    inventories = inventory_document["inventories"]
    inventory_by_key = {
        (row["path"], row["test_name"]): (discovery_id, row)
        for discovery_id, row in inventories.items()
    }
    selected_by_path: dict[str, list[Mapping[str, str]]] = defaultdict(list)
    for row in selection:
        selected_by_path[row["path"]].append(row)
    projected = project_raw_results(
        raw=raw,
        resolved=resolved,
        path_by_name={row["name"]: row["path"] for row in selection},
    )
    invariants = _load_invariants(root)
    plans_dir.mkdir(parents=True, exist_ok=True)
    results_dir.mkdir(parents=True, exist_ok=True)
    path_records = []
    pending_results: dict[str, dict[str, Any]] = {}
    for path, rows in sorted(selected_by_path.items()):
        names = {row["name"] for row in rows}
        path_raw = plans_dir / f"{path_key(path)}.jsonl"
        _write_text(path_raw, projected[path])
        normalized = normalize_results(
            projected[path],
            expected={name: resolved[name]["encoded_name"] for name in names},
        )
        path_assignments = [assignments[(path, name)] for name in sorted(names)]
        setup_by_name = {
            row["name"]: set(row["setup_assertion_ids"]) for row in path_assignments
        }
        non_oracle_by_name = {
            row["name"]: set(row["non_oracle_assertion_ids"])
            for row in path_assignments
        }
        invariant_ids = sorted(
            {
                assertion["invariant_id"]
                for row in path_assignments
                for assertion in row["assertion_assignments"]
                if assertion["assertion_id"] not in setup_by_name[row["name"]]
                and assertion["assertion_id"] not in non_oracle_by_name[row["name"]]
            }
        )
        packages = {resolved[name]["package"] for name in names}
        if len(packages) != 1:
            raise ValueError(f"{path} resolves to multiple packages")
        package = next(iter(packages))
        profile = (
            _mixed_target_profile(path)
            if mixed_debug_runtime_core
            else None
        )
        plan = {
            "schema_version": 1,
            "execution_kind": "runtime_rust_nextest_projection",
            "path": path,
            "package": package,
            "command": shlex.join(command),
            "runner_path": RUNNER_PATH.as_posix(),
            "reviewed_date": reviewed_date,
            "timestamp": f"{reviewed_date}T00:00:00Z",
            "reviewer_ids": ["codex-main", "codex-assertion-review"],
            "expected_count": len(rows),
            "selection_digest": selection_digest(rows),
            "invariant_ids": invariant_ids,
            "invariant_behavior_digests": {
                invariant_id: digest_document(
                    {"behavior": invariants[invariant_id].get("behavior")}
                )
                for invariant_id in invariant_ids
            },
            "assertion_assignments": [
                {
                    "name": row["name"],
                    "assertions": row["assertion_assignments"],
                }
                for row in path_assignments
            ],
            "assertion_dispositions": [
                {
                    "name": row["name"],
                    "setup_assertion_ids": row["setup_assertion_ids"],
                    "non_oracle_assertion_ids": row["non_oracle_assertion_ids"],
                }
                for row in path_assignments
                if row["setup_assertion_ids"] or row["non_oracle_assertion_ids"]
            ],
            "resolved_tests": {
                name: dict(resolved[name]) for name in sorted(names)
            },
            "no_default_feature_names": sorted(
                names & no_default_feature_names
            ),
        }
        plan_path = plans_dir / f"{path_key(path)}-plan.json"
        inventory_path = plans_dir / f"{path_key(path)}-assertions.json"
        results_path = results_dir / f"{path_key(path)}-results.json"
        binary_ids = {resolved[name]["binary_id"] for name in names}
        if len(binary_ids) != 1:
            raise ValueError(f"{path} resolves to multiple nextest binaries")
        binary_id = next(iter(binary_ids))
        catalog_target_args = (
            list(profile.cargo_target_args)
            if profile is not None
            else selected_target_args(rows, root=root)
        )
        plan["catalog_command_template"] = (
            shlex.join(
                [
                    "cargo",
                    "nextest",
                    "run",
                    "-p",
                    package,
                    *catalog_target_args,
                ]
            )
            + " --no-fail-fast "
            "--user-config-file none -E "
            + shlex.quote(
                (
                    f"binary_id(={binary_id}) & "
                    f"test(={profile.test_prefix}{{name}})"
                    if profile is not None
                    else f"binary_id(={binary_id}) & test(~{{name}})"
                )
            )
        )
        if names & no_default_feature_names:
            plan["catalog_command_template_no_default_features"] = (
                shlex.join(
                    [
                        "cargo",
                        "nextest",
                        "run",
                        "-p",
                        package,
                        *catalog_target_args,
                        "--no-default-features",
                    ]
                )
                + " "
                "--no-fail-fast --user-config-file none -E "
                + shlex.quote(
                    f"binary_id(={binary_id}) & test(~{{name}})"
                )
            )
        _write_json(plan_path, plan)
        _write_json(
            inventory_path,
            {
                "schema_version": 2,
                "inventories": {
                    inventory_by_key[(path, name)][0]: inventory_by_key[(path, name)][1]
                    for name in sorted(names)
                },
            },
        )
        pending_results[path] = {
            "schema_version": 1,
            "tests": normalized,
            "failures": [],
            "pending": [],
            "stats": {
                "tests": len(normalized),
                "passes": len(normalized),
                "failures": 0,
                "pending": 0,
            },
        }
        path_records.append(
            {
                "path": path,
                "expected_count": len(rows),
                "plan_path": _relative(root, plan_path),
                "inventory_path": _relative(root, inventory_path),
                "results_path": _relative(root, results_path),
                "projected_raw_path": _relative(root, path_raw),
                "projected_raw_sha256": _digest_file(path_raw),
                "package": package,
            }
        )
    metadata = {
        "schema_version": 1,
        "kind": "runtime_rust_nextest_projection",
        "source_commit": _git_head(root),
        "command": shlex.join(command),
        "nextest_version": nextest_version,
        "selected_test_count": len(selection),
        "denominator_sha256": _digest_file(root / DENOMINATOR_PATH),
        "catalog_sha256": _digest_file(root / CATALOG_PATH),
        "runner_sha256": _digest_file(root / RUNNER_PATH),
        "list_path": _relative(root, list_path),
        "list_sha256": _digest_file(list_path),
        "raw_path": _relative(root, raw_path),
        "raw_sha256": _digest_file(raw_path),
        "paths": path_records,
    }
    _write_json(metadata_path, metadata)
    metadata_digest = _digest_file(metadata_path)
    for record in path_records:
        path = record["path"]
        result = pending_results[path]
        result["execution"] = {
            "kind": "runtime_rust_nextest_projection",
            "batch_command": metadata["command"],
            "runner_path": RUNNER_PATH.as_posix(),
            "runner_sha256": metadata["runner_sha256"],
            "source_path": path,
            "source_sha256": _digest_file(root / path),
            "raw_results_path": record["projected_raw_path"],
            "raw_results_sha256": record["projected_raw_sha256"],
            "combined_raw_results_path": metadata["raw_path"],
            "combined_raw_results_sha256": metadata["raw_sha256"],
            "execution_metadata_path": _relative(root, metadata_path),
            "execution_metadata_sha256": metadata_digest,
            "nextest_version": nextest_version,
            "package": record["package"],
        }
        _write_json(root / record["results_path"], result)
    index = {
        "schema_version": 1,
        "source_commit": metadata["source_commit"],
        "expected_count": len(selection),
        "paths": path_records,
    }
    _write_json(plans_dir / "index.json", index)
    return index


def validate_projection_execution(
    *,
    root: Path,
    plan: Mapping[str, Any],
    execution: Mapping[str, Any],
    results_document: Mapping[str, Any],
) -> None:
    expected_fields = {
        "kind",
        "batch_command",
        "runner_path",
        "runner_sha256",
        "source_path",
        "source_sha256",
        "raw_results_path",
        "raw_results_sha256",
        "combined_raw_results_path",
        "combined_raw_results_sha256",
        "execution_metadata_path",
        "execution_metadata_sha256",
        "nextest_version",
        "package",
    }
    if set(execution) != expected_fields:
        raise ValueError("runtime projection execution fields drift")
    expected = {
        "kind": "runtime_rust_nextest_projection",
        "batch_command": _string(plan, "command", "plan"),
        "runner_path": _string(plan, "runner_path", "plan"),
        "source_path": _string(plan, "path", "plan"),
        "package": _string(plan, "package", "plan"),
    }
    for field, value in expected.items():
        if execution.get(field) != value:
            raise ValueError(f"runtime projection {field} drifted")
    for path_field, digest_field in (
        ("runner_path", "runner_sha256"),
        ("source_path", "source_sha256"),
        ("raw_results_path", "raw_results_sha256"),
        ("combined_raw_results_path", "combined_raw_results_sha256"),
        ("execution_metadata_path", "execution_metadata_sha256"),
    ):
        path = root / str(execution[path_field])
        if not path.is_file() or path.is_symlink() or _digest_file(path) != execution[digest_field]:
            raise ValueError(f"runtime projection {digest_field} is stale")
    metadata = _json(root / str(execution["execution_metadata_path"]))
    if metadata.get("source_commit") != _git_head(root):
        raise ValueError("runtime projection source commit does not match HEAD")
    if metadata.get("command") != execution["batch_command"]:
        raise ValueError("runtime projection command drifted")
    resolved = plan.get("resolved_tests")
    if not isinstance(resolved, Mapping):
        raise ValueError("runtime projection plan lacks resolved tests")
    expected_names = {
        str(name): _string(row, "encoded_name", str(name))
        for name, row in resolved.items()
        if isinstance(row, Mapping)
    }
    normalized = normalize_results(
        (root / str(execution["raw_results_path"])).read_text(),
        expected=expected_names,
    )
    if normalized != results_document.get("tests"):
        raise ValueError("runtime projected normalized results drifted")


def project_raw_results(
    *,
    raw: str,
    resolved: Mapping[str, Mapping[str, str]],
    path_by_name: Mapping[str, str],
) -> dict[str, str]:
    encoded_to_name = {
        row["encoded_name"]: name for name, row in resolved.items()
    }
    lines_by_path: dict[str, list[str]] = defaultdict(list)
    seen: set[str] = set()
    events: list[tuple[int, str, Mapping[str, Any]]] = []
    for line_number, line in enumerate(raw.splitlines(), start=1):
        if not line.strip():
            continue
        event = json.loads(line)
        if not isinstance(event, Mapping):
            raise ValueError(f"raw nextest line {line_number} is not an object")
        events.append((line_number, line, event))
    unselected_terminal_events: dict[str, set[str]] = defaultdict(set)
    for _line_number, _line, event in events:
        encoded = event.get("name")
        kind = event.get("event")
        if (
            event.get("type") == "test"
            and encoded not in encoded_to_name
            and kind in TERMINAL_EVENTS
        ):
            unselected_terminal_events[str(encoded)].add(str(kind))
    unselected_ignored = {
        encoded
        for encoded, kinds in unselected_terminal_events.items()
        if kinds == {"ignored"}
    }
    for _line_number, line, event in events:
        if event.get("type") != "test":
            continue
        encoded = event.get("name")
        if encoded not in encoded_to_name:
            if encoded in unselected_ignored:
                continue
            raise ValueError(f"unexpected runtime test event {encoded!r}")
        name = encoded_to_name[str(encoded)]
        lines_by_path[path_by_name[name]].append(line)
        if event.get("event") in TERMINAL_EVENTS:
            if name in seen:
                raise ValueError(f"duplicate runtime terminal event {name}")
            seen.add(name)
    missing = sorted(set(resolved) - seen)
    if missing:
        raise ValueError("runtime execution lacks terminal tests: " + ", ".join(missing))
    return {
        path: "\n".join(lines) + "\n"
        for path, lines in lines_by_path.items()
    }


def normalize_results(
    raw: str,
    *,
    expected: Mapping[str, str],
) -> list[dict[str, Any]]:
    encoded_to_name = {encoded: name for name, encoded in expected.items()}
    started: set[str] = set()
    terminal: dict[str, dict[str, Any]] = {}
    for line in raw.splitlines():
        if not line.strip():
            continue
        event = json.loads(line)
        if not isinstance(event, Mapping) or event.get("type") != "test":
            continue
        encoded = event.get("name")
        name = encoded_to_name.get(str(encoded))
        if name is None:
            raise ValueError(f"unexpected projected runtime test {encoded!r}")
        kind = event.get("event")
        if kind == "started":
            if name in started:
                raise ValueError(f"duplicate runtime start {name}")
            started.add(name)
            continue
        if kind not in TERMINAL_EVENTS:
            raise ValueError(f"runtime test {name} has unknown event {kind!r}")
        if kind != "ok":
            raise ValueError(f"runtime test {name} did not pass: {kind}")
        if name in terminal:
            raise ValueError(f"duplicate runtime result {name}")
        terminal[name] = {
            "title": name,
            "duration": _duration_ms(event.get("exec_time"), name),
            "state": "passed",
        }
    if set(terminal) != set(expected):
        raise ValueError("runtime normalized names do not equal expected names")
    return [terminal[name] for name in sorted(terminal)]


def _selection(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    assignments_document: Mapping[str, Any],
) -> list[dict[str, str]]:
    assignments = assignments_document.get("assignments")
    inventories = inventory_document.get("inventories")
    if (
        not isinstance(assignments, Sequence)
        or isinstance(assignments, (str, bytes))
        or not isinstance(inventories, Mapping)
    ):
        raise ValueError("runtime assignment inputs are invalid")
    assigned = {
        (str(row.get("path")), str(row.get("name"))): row
        for row in assignments
        if isinstance(row, Mapping)
    }
    denominator = tomllib.loads((root / DENOMINATOR_PATH).read_text())
    rows = [
        dict(row)
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code") == MAPPING_DEBT_REASON
        and (str(row.get("path")), str(row.get("name"))) in assigned
    ]
    if len(rows) != len(assigned):
        raise ValueError("runtime assignments do not equal frozen mapping-debt rows")
    inventory_keys = {
        (str(row.get("path")), str(row.get("test_name")))
        for row in inventories.values()
        if isinstance(row, Mapping)
    }
    if set(assigned) - inventory_keys:
        raise ValueError("runtime assignments lack assertion inventories")
    return sorted(rows, key=lambda row: str(row["discovery_id"]))


def _load_invariants(root: Path) -> dict[str, Mapping[str, Any]]:
    result = {}
    for path in (root / "verification/invariants").rglob("*.toml"):
        row = tomllib.loads(path.read_text())
        if isinstance(row.get("id"), str):
            result[row["id"]] = row
    return result


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--inventory", type=Path, required=True)
    parser.add_argument("--assignments", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--reviewed-date", required=True)
    parser.add_argument(
        "--normalize-only",
        action="store_true",
        help="reuse existing list and raw evidence without executing tests",
    )
    parser.add_argument(
        "--mixed-debug-runtime-core",
        action="store_true",
        help=(
            "run one exact trust-debug, trust-runtime-core, and focused "
            "trust-hir projection batch"
        ),
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        index = run_batch(
            root=root,
            inventory_path=_root_path(root, args.inventory),
            assignments_path=_root_path(root, args.assignments),
            output_dir=_root_path(root, args.output_dir),
            reviewed_date=args.reviewed_date,
            normalize_only=args.normalize_only,
            mixed_debug_runtime_core=args.mixed_debug_runtime_core,
        )
        print(
            "frozen runtime protocol/I/O batch recorded: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
        return 0
    except (
        OSError,
        ValueError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"frozen runtime protocol/I/O batch failed: {error}")
        return 2


def path_key(path: str) -> str:
    return f"{Path(path).stem.replace('_', '-')}-{hashlib.sha256(path.encode()).hexdigest()[:12]}"


def _duration_ms(value: Any, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"runtime test {name} lacks numeric duration")
    if isinstance(value, float) and not math.isfinite(value):
        raise ValueError(f"runtime test {name} has non-finite duration")
    try:
        seconds = Decimal(str(value))
    except InvalidOperation as error:
        raise ValueError(f"runtime test {name} has invalid duration") from error
    return int((seconds * Decimal(1000)).to_integral_value(rounding=ROUND_CEILING))


def _require_clean_checkout(root: Path) -> None:
    status = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if status:
        raise ValueError("runtime execution requires a clean checkout")


def _git_head(root: Path) -> str:
    return subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def _json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a JSON object")
    return document


def _digest_file(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _relative(root: Path, path: Path) -> str:
    return path.resolve().relative_to(root.resolve()).as_posix()


def _root_path(root: Path, path: Path) -> Path:
    resolved = path if path.is_absolute() else root / path
    resolved = resolved.resolve()
    resolved.relative_to(root)
    return resolved


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _write_json(path: Path, document: Mapping[str, Any]) -> None:
    _write_text(path, json.dumps(document, indent=2, sort_keys=True) + "\n")


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        delete=False,
    ) as handle:
        handle.write(content)
        temporary = Path(handle.name)
    os.replace(temporary, path)


def _mixed_target_profile(path: str) -> FrozenRustTargetProfile:
    if path.startswith("crates/trust-debug/"):
        key = "debug-remaining"
    elif path.startswith("crates/trust-runtime-core/"):
        key = "runtime-core-remaining"
    elif path.startswith("crates/trust-hir/"):
        key = "hir-remaining"
    elif path.startswith("crates/trust-runtime/"):
        key = "runtime-remaining"
    else:
        raise ValueError(f"unsupported mixed Rust test path {path}")
    return get_target_profile(key, path=path)


if __name__ == "__main__":
    raise SystemExit(main())
