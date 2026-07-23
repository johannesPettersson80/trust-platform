"""Run VS Code tests once with isolated per-test V8 coverage snapshots."""

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
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .code_spec_live import load_code_spec_metadata
from .code_spec_typescript import invoke_typescript_extractor
from .coverage_trace_inputs import trace_input_digest
from .coverage_trace_inventory import code_inventory_digest
from .coverage_trace_istanbul import merge_istanbul_coverage
from .coverage_trace_istanbul_document import build_istanbul_trace_document
from .test_catalog_json_schema import validate_json_schema_instance
from .test_catalog_scanner import scan_repository


def build_vscode_selection(
    facts: Sequence[Mapping[str, Any]],
    *,
    root: Path,
) -> dict[str, Any]:
    tests: list[dict[str, str]] = []
    seen: set[str] = set()
    for fact in sorted(facts, key=lambda row: str(row.get("discovery_id", ""))):
        discovery_id = _required_string(fact, "discovery_id", "VS Code fact")
        if discovery_id in seen:
            raise ValueError(f"VS Code selection duplicates {discovery_id}")
        seen.add(discovery_id)
        if fact.get("discovery_source_kind") != "vscode_test":
            raise ValueError(f"{discovery_id} is not a VS Code test")
        if fact.get("command_hint") != "cd editors/vscode && npm test":
            raise ValueError(f"{discovery_id} VS Code command hint drifted")
        source_path = _required_string(fact, "path", discovery_id)
        prefix = "editors/vscode/src/"
        if not source_path.startswith(prefix) or "/test/" not in source_path:
            raise ValueError(f"{discovery_id} source path is outside VS Code tests")
        relative = Path(source_path[len(prefix) :])
        compiled = (
            Path("editors/vscode/out")
            / relative.with_suffix(".js")
        ).as_posix()
        if not (root / compiled).is_file():
            raise ValueError(
                f"{discovery_id} compiled VS Code test is missing: {compiled}"
            )
        tests.append(
            {
                "discovery_id": discovery_id,
                "source_path": source_path,
                "compiled_file": compiled,
                "title": _required_string(fact, "name", discovery_id),
            }
        )
    if not tests:
        raise ValueError("VS Code trace selection is empty")
    return {"schema_version": 1, "tests": tests}


def validate_vscode_attempts(
    document: Mapping[str, Any],
    *,
    requested_ids: set[str],
    coverage_root: Path,
) -> dict[str, Mapping[str, Any]]:
    if document.get("schema_version") != 1:
        raise ValueError("VS Code attempts schema_version must equal 1")
    requested = document.get("requested_discovery_ids")
    attempts = document.get("attempts")
    if (
        not isinstance(requested, list)
        or any(not isinstance(item, str) or not item for item in requested)
        or set(requested) != requested_ids
        or len(requested) != len(requested_ids)
    ):
        raise ValueError("VS Code attempts requested denominator drifted")
    if not isinstance(attempts, list) or any(
        not isinstance(attempt, Mapping) for attempt in attempts
    ):
        raise ValueError("VS Code attempts must be an object array")
    indexed: dict[str, Mapping[str, Any]] = {}
    resolved_coverage_root = coverage_root.resolve()
    for attempt in attempts:
        discovery_id = _required_string(
            attempt, "discovery_id", "VS Code attempt"
        )
        if discovery_id in indexed:
            raise ValueError(f"VS Code attempts duplicate {discovery_id}")
        if attempt.get("exit_code") != 0:
            raise ValueError(f"{discovery_id} VS Code test did not pass")
        duration = attempt.get("duration_ms")
        if (
            not isinstance(duration, int)
            or isinstance(duration, bool)
            or duration < 0
        ):
            raise ValueError(f"{discovery_id} duration is invalid")
        raw_files = attempt.get("raw_coverage_files")
        if not isinstance(raw_files, list) or not raw_files:
            raise ValueError(f"{discovery_id} has no raw V8 coverage")
        for raw_file in raw_files:
            if not isinstance(raw_file, str) or not raw_file:
                raise ValueError(f"{discovery_id} coverage path is invalid")
            resolved = Path(raw_file).resolve()
            try:
                resolved.relative_to(resolved_coverage_root)
            except ValueError as error:
                raise ValueError(
                    f"{discovery_id} coverage path escapes trace root"
                ) from error
            if not resolved.is_file():
                raise ValueError(f"{discovery_id} coverage file is missing")
        indexed[discovery_id] = attempt
    if set(indexed) != requested_ids:
        raise ValueError(
            "VS Code attempts do not exactly cover selection: "
            f"missing={sorted(requested_ids - set(indexed))}, "
            f"extra={sorted(set(indexed) - requested_ids)}"
        )
    return dict(sorted(indexed.items()))


def vscode_launcher_command(
    *,
    root: Path,
    selection_path: Path,
    attempts_path: Path,
    coverage_root: Path,
    cargo_target_dir: Path,
) -> list[str]:
    """Launch the real extension host on the required headless X display."""

    return [
        "xvfb-run",
        "-a",
        "-n",
        "120",
        "-s",
        "-screen 0 1920x1080x24",
        "node",
        str(root / "scripts/verification/vscode_trace_launcher.cjs"),
        str(root),
        str(root / "scripts/verification/vscode_trace_suite.cjs"),
        str(selection_path),
        str(attempts_path),
        str(coverage_root),
        str(cargo_target_dir.resolve()),
    ]


def run_vscode_trace(
    *,
    root: Path,
    discovery_ids: Sequence[str] | None,
    all_vscode: bool,
    trace_root: Path,
    output_path: Path,
    cargo_target_dir: Path,
    temporary_dir: Path,
    mode: str,
    resume: bool = False,
) -> dict[str, Any]:
    root = root.resolve()
    trace_root = trace_root.resolve()
    if trace_root.exists() and any(trace_root.iterdir()) and not resume:
        raise ValueError("VS Code trace root must be absent or empty")
    trace_root.mkdir(parents=True, exist_ok=True)
    scan = scan_repository(root)
    facts = {
        fact.stable_id: {
            "discovery_id": fact.stable_id,
            "discovery_source_kind": fact.source_kind,
            "path": fact.path,
            "name": fact.name,
            "package": fact.package,
            "command_hint": fact.command_hint,
            "ignore_state": fact.ignore_state,
        }
        for fact in scan.inferred_facts
    }
    if all_vscode:
        selected_ids = sorted(
            discovery_id
            for discovery_id, fact in facts.items()
            if fact["discovery_source_kind"] == "vscode_test"
            and fact["ignore_state"] != "ignored"
        )
    else:
        selected_ids = sorted(set(discovery_ids or []))
        if len(selected_ids) != len(discovery_ids or []):
            raise ValueError("VS Code trace discovery IDs must be unique")
    unknown = sorted(set(selected_ids) - set(facts))
    if unknown:
        raise ValueError(f"VS Code trace names unknown discovery IDs: {unknown}")
    selected = [facts[discovery_id] for discovery_id in selected_ids]
    unsupported = sorted(
        discovery_id
        for discovery_id, fact in zip(selected_ids, selected, strict=True)
        if fact["discovery_source_kind"] != "vscode_test"
    )
    if unsupported:
        raise ValueError(f"VS Code trace selection has non-VS Code facts: {unsupported}")
    ignored = sorted(
        fact["discovery_id"]
        for fact in selected
        if fact["ignore_state"] == "ignored"
    )
    if ignored:
        raise ValueError(
            f"VS Code trace selection contains structurally routed ignores: {ignored}"
        )
    environment = dict(os.environ)
    temporary_dir.mkdir(parents=True, exist_ok=True)
    environment["TMPDIR"] = str(temporary_dir.resolve())
    environment["CARGO_TARGET_DIR"] = str(cargo_target_dir.resolve())
    if not (root / "editors/vscode/node_modules").is_dir():
        _run(
            ["npm", "ci"],
            cwd=root / "editors/vscode",
            environment=environment,
            label="VS Code dependency install",
        )
    _run(
        ["npm", "run", "compile"],
        cwd=root / "editors/vscode",
        environment=environment,
        label="VS Code compile",
    )
    _run(
        [
            "cargo",
            "build",
            "-p",
            "trust-lsp",
            "-p",
            "trust-runtime",
            "-p",
            "trust-dev",
            "-p",
            "trust-debug",
        ],
        cwd=root,
        environment=environment,
        label="VS Code trace binary build",
    )
    selection = build_vscode_selection(selected, root=root)
    selection_path = trace_root / "selection.json"
    attempts_path = trace_root / "attempts.json"
    coverage_root = trace_root / "v8"
    coverage_root.mkdir(exist_ok=True)
    if resume and selection_path.is_file():
        if _load_json(selection_path) != selection:
            raise ValueError(
                "resume request does not match existing VS Code trace denominator"
            )
    selection_path.write_text(json.dumps(selection, indent=2, sort_keys=True) + "\n")
    launcher_command = vscode_launcher_command(
        root=root,
        selection_path=selection_path,
        attempts_path=attempts_path,
        coverage_root=coverage_root,
        cargo_target_dir=cargo_target_dir,
    )
    _run(
        launcher_command,
        cwd=root,
        environment=environment,
        label="VS Code selected test batch",
    )
    attempts_document = _load_json(attempts_path)
    attempts = validate_vscode_attempts(
        attempts_document,
        requested_ids=set(selected_ids),
        coverage_root=coverage_root,
    )
    coverage_documents: dict[str, Mapping[str, Any]] = {}
    observations: dict[str, Mapping[str, Any]] = {}
    converted_root = trace_root / "istanbul"
    converted_root.mkdir(exist_ok=True)
    converter = root / "scripts/verification/vscode_trace_convert.cjs"
    for discovery_id, attempt in attempts.items():
        converted: list[Mapping[str, Any]] = []
        for index, raw_file in enumerate(attempt["raw_coverage_files"]):
            converted_path = converted_root / f"{discovery_id}-{index}.json"
            _run(
                [
                    "node",
                    str(converter),
                    str(root),
                    str(raw_file),
                    str(converted_path),
                ],
                cwd=root,
                environment=environment,
                label=f"{discovery_id} V8 conversion",
            )
            converted.append(_load_json(converted_path))
        coverage_documents[discovery_id] = merge_istanbul_coverage(converted)
        observations[discovery_id] = {
            "attempt_id": attempt["attempt_id"],
            "exit_code": 0,
            "duration_ms": attempt["duration_ms"],
            "command": shlex.join(launcher_command),
        }
    metadata = load_code_spec_metadata(root)
    matrix = tomllib.loads((root / "verification/matrix.toml").read_text())
    inventory = invoke_typescript_extractor(
        root,
        matrix,
        area_bindings={},
    )
    sources = {
        path: (root / path).read_bytes() for path in inventory.source_paths
    }
    document = build_istanbul_trace_document(
        root=root,
        requested=selected,
        observations=observations,
        coverage_documents=coverage_documents,
        sources=sources,
        spans=inventory.spans,
        source_commit=_git_head(root),
        platform=f"{platform.system().lower()}-{platform.machine().lower()}",
        input_digest=trace_input_digest(
            scan.input_digest,
            selected,
            metadata.code_map,
        ),
        code_inventory_digest=code_inventory_digest(
            inventory.facts,
            inventory.spans,
        ),
        mode=mode,
    )
    schema = _load_json(
        root / "verification/schemas/test-coverage-trace.schema.json"
    )
    failures = validate_json_schema_instance(document, schema)
    if failures:
        raise ValueError("; ".join(failures))
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    return document


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument("--discovery-id", action="append")
    selection.add_argument("--all-vscode", action="store_true")
    parser.add_argument(
        "--trace-root",
        type=Path,
        default=Path("target/gate-artifacts/verification/vscode-catalog-trace"),
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(
            "target/gate-artifacts/verification/vscode-test-coverage-trace.json"
        ),
    )
    parser.add_argument("--cargo-target-dir", type=Path, required=True)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument(
        "--temporary-dir",
        type=Path,
        default=Path(
            "target/gate-artifacts/verification/vscode-trace-temporary"
        ),
    )
    parser.add_argument(
        "--mode",
        choices=("focused_shard", "full_campaign"),
        default="focused_shard",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        document = run_vscode_trace(
            root=root,
            discovery_ids=args.discovery_id,
            all_vscode=args.all_vscode,
            trace_root=_resolve(root, args.trace_root),
            output_path=_resolve(root, args.out),
            cargo_target_dir=_resolve(root, args.cargo_target_dir),
            temporary_dir=_resolve(root, args.temporary_dir),
            mode=args.mode,
            resume=args.resume,
        )
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"VS Code coverage trace failed: {error}", file=sys.stderr)
        return 2
    print(f"VS Code coverage trace complete: {len(document['tests'])} tests observed")
    return 0


def _run(
    command: list[str],
    *,
    cwd: Path,
    environment: Mapping[str, str],
    label: str,
) -> None:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=dict(environment),
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise ValueError(
            f"{label} failed ({completed.returncode}): "
            f"{completed.stderr.strip() or completed.stdout.strip()}"
        )


def _load_json(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} root must be an object")
    return document


def _resolve(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _git_head(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    head = completed.stdout.strip()
    if completed.returncode != 0 or re.fullmatch(r"[0-9a-f]{40}", head) is None:
        raise ValueError("VS Code trace cannot resolve source commit")
    return head


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


if __name__ == "__main__":
    raise SystemExit(main())
