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
) -> dict[str, Any]:
    _require_clean_checkout(root)
    inventory_document = _json(inventory_path)
    assignments_document = _json(assignments_path)
    selection = _selection(
        root=root,
        inventory_document=inventory_document,
        assignments_document=assignments_document,
    )
    target_args = selected_target_args(selection)
    output_dir.mkdir(parents=True, exist_ok=True)
    list_path = output_dir / "nextest-list.json"
    raw_path = output_dir / "runtime-protocol-io.jsonl"
    metadata_path = output_dir / "execution.json"
    plans_dir = output_dir / "plans"
    results_dir = output_dir / "results"
    if any(path.exists() for path in (list_path, raw_path, metadata_path)):
        raise ValueError("runtime protocol/I/O execution outputs already exist")

    environment = dict(os.environ)
    environment.update(EXPERIMENTAL_ENVIRONMENT)
    list_command = [
        "cargo",
        "nextest",
        "list",
        "-p",
        "trust-runtime",
        *target_args,
        "--message-format",
        "json",
    ]
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
            "runtime nextest list failed: "
            + (listed.stderr.strip() or f"exit {listed.returncode}")
        )
    _write_text(list_path, listed.stdout)
    resolved = resolve_nextest_listing(
        listing=json.loads(listed.stdout),
        selected_names={row["name"] for row in selection},
    )
    command = build_batch_command(resolved, target_args=target_args)
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
    )


def resolve_nextest_listing(
    *,
    listing: Mapping[str, Any],
    selected_names: set[str],
) -> dict[str, dict[str, str]]:
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
        testcases = raw_suite.get("testcases")
        if not isinstance(testcases, Mapping):
            raise ValueError(f"nextest suite {binary_id} lacks testcases")
        for full_name in testcases:
            if not isinstance(full_name, str) or not full_name:
                raise ValueError(f"nextest suite {binary_id} has invalid test name")
            bare = full_name.rsplit("::", 1)[-1]
            if bare in selected_names:
                candidates[bare].append(
                    {
                        "binary_id": binary_id,
                        "full_name": full_name,
                        "encoded_name": f"{binary_id}${full_name}",
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
        "-p",
        "trust-runtime",
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


def selected_target_args(
    selection: Sequence[Mapping[str, str]],
) -> list[str]:
    """Select only the library and named integration targets in the batch."""

    has_lib = False
    integration_targets: set[str] = set()
    prefix = "crates/trust-runtime/tests/"
    for row in selection:
        path = _string(row, "path", "selection")
        if path.startswith(prefix):
            relative = path.removeprefix(prefix)
            if "/" in relative or not relative.endswith(".rs"):
                raise ValueError(f"unsupported runtime integration path {path}")
            integration_targets.add(relative.removesuffix(".rs"))
        elif path.startswith("crates/trust-runtime/src/"):
            has_lib = True
        else:
            raise ValueError(f"unsupported runtime test path {path}")
    args = ["--lib"] if has_lib else []
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
        plan = {
            "schema_version": 1,
            "execution_kind": "runtime_rust_nextest_projection",
            "path": path,
            "package": "trust-runtime",
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
        }
        plan_path = plans_dir / f"{path_key(path)}-plan.json"
        inventory_path = plans_dir / f"{path_key(path)}-assertions.json"
        results_path = results_dir / f"{path_key(path)}-results.json"
        binary_ids = {resolved[name]["binary_id"] for name in names}
        if len(binary_ids) != 1:
            raise ValueError(f"{path} resolves to multiple nextest binaries")
        binary_id = next(iter(binary_ids))
        plan["catalog_command_template"] = (
            "cargo nextest run -p trust-runtime --no-fail-fast "
            "--user-config-file none -E "
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
            "package": "trust-runtime",
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
        "package": "trust-runtime",
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
    for line_number, line in enumerate(raw.splitlines(), start=1):
        if not line.strip():
            continue
        event = json.loads(line)
        if not isinstance(event, Mapping):
            raise ValueError(f"raw nextest line {line_number} is not an object")
        if event.get("type") != "test":
            continue
        encoded = event.get("name")
        if encoded not in encoded_to_name:
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


if __name__ == "__main__":
    raise SystemExit(main())
