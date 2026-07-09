"""Mechanical discovery for conformance, fuzz, gates, and workflows."""

from __future__ import annotations

import re
import tomllib
from collections import Counter
from pathlib import Path

from .test_catalog_common import (
    ScanBatch,
    diagnostic,
    make_fact,
    references_in,
    relative_path,
    source_files,
)


WORKFLOW_JOB_RE = re.compile(r"^  ([A-Za-z0-9_-]+):\s*(?:#.*)?$")
YAML_NAME_RE = re.compile(r"^name:\s*(.+?)\s*$")


def scan_conformance(root: Path) -> ScanBatch:
    batch = ScanBatch()
    cases_root = root / "conformance/cases"
    if not cases_root.is_dir():
        batch.diagnostics.append(
            diagnostic(
                "missing_scan_root",
                "conformance/cases",
                1,
                "required conformance case root is missing",
                severity="error",
            )
        )
        return batch
    ids: list[tuple[str, str, int]] = []
    for manifest in (
        path
        for path in source_files(root, "conformance/cases", (".toml",))
        if path.name == "manifest.toml"
    ):
        relative = relative_path(root, manifest)
        batch.input_paths.add(relative)
        try:
            text = manifest.read_text()
            data = tomllib.loads(text)
        except Exception as exc:
            batch.diagnostics.append(
                diagnostic("conformance_manifest_parse", relative, 1, str(exc), severity="error")
            )
            continue
        case_id = data.get("id")
        if not isinstance(case_id, str) or not case_id:
            batch.diagnostics.append(
                diagnostic(
                    "conformance_id_missing",
                    relative,
                    1,
                    "manifest id is missing",
                    severity="error",
                )
            )
            continue
        line = first_assignment_line(text, "id")
        ids.append((case_id, relative, line))
        batch.facts.append(
            make_fact(
                source_kind="conformance_case",
                name=case_id,
                path=relative,
                line=line,
                package="trust-runtime",
                command_hint=(
                    "cargo run -p trust-runtime --bin trust-runtime -- conformance --suite-root conformance "
                    f"--filter {case_id}"
                ),
                command_hint_authority="exact",
                discovery_confidence="parsed_manifest",
                native_id=case_id,
                reference_candidates=references_in(text),
            )
        )
    duplicates = {case_id for case_id, count in Counter(item[0] for item in ids).items() if count > 1}
    for case_id, path, line in ids:
        if case_id in duplicates:
            batch.diagnostics.append(
                diagnostic(
                    "duplicate_conformance_id",
                    path,
                    line,
                    f"duplicate conformance id {case_id}",
                    severity="error",
                )
            )
    return batch


def first_assignment_line(text: str, key: str) -> int:
    pattern = re.compile(rf"^\s*{re.escape(key)}\s*=", re.MULTILINE)
    match = pattern.search(text)
    return text.count("\n", 0, match.start()) + 1 if match else 1


def scan_fuzz_targets(root: Path) -> ScanBatch:
    batch = ScanBatch()
    manifest = root / "fuzz/Cargo.toml"
    if not manifest.is_file():
        batch.diagnostics.append(
            diagnostic(
                "missing_scan_root",
                "fuzz",
                1,
                "required fuzz manifest is missing",
                severity="error",
            )
        )
        return batch
    relative_manifest = "fuzz/Cargo.toml"
    batch.input_paths.add(relative_manifest)
    try:
        text = manifest.read_text()
        data = tomllib.loads(text)
    except Exception as exc:
        batch.diagnostics.append(
            diagnostic("fuzz_manifest_parse", relative_manifest, 1, str(exc), severity="error")
        )
        return batch
    package = data.get("package", {}).get("name")
    package = package if isinstance(package, str) and package else "cargo-fuzz"
    for item in data.get("bin", []):
        if not isinstance(item, dict):
            continue
        name = item.get("name")
        target_value = item.get("path")
        if not isinstance(name, str) or not isinstance(target_value, str):
            batch.diagnostics.append(
                diagnostic(
                    "fuzz_target_invalid",
                    relative_manifest,
                    1,
                    "bin needs name and path",
                    severity="error",
                )
            )
            continue
        target = (root / "fuzz" / target_value).resolve()
        fuzz_root = (root / "fuzz/fuzz_targets").resolve()
        try:
            target.relative_to(fuzz_root)
        except ValueError:
            batch.diagnostics.append(
                diagnostic(
                    "fuzz_target_path",
                    relative_manifest,
                    1,
                    f"target escapes fuzz_targets: {target_value}",
                    severity="error",
                )
            )
            continue
        relative = relative_path(root, target)
        if not target.is_file():
            batch.diagnostics.append(
                diagnostic(
                    "fuzz_target_missing",
                    relative,
                    1,
                    "target file is missing",
                    severity="error",
                )
            )
            continue
        batch.input_paths.add(relative)
        try:
            target_text = target.read_text()
        except (OSError, UnicodeError) as exc:
            batch.diagnostics.append(diagnostic("source_read", relative, 1, str(exc), severity="error"))
            continue
        batch.facts.append(
            make_fact(
                source_kind="fuzz_target",
                name=name,
                path=relative,
                line=1,
                package=package,
                command_hint=f"cd fuzz && cargo fuzz run {name}",
                command_hint_authority="exact",
                discovery_confidence="parsed_manifest",
                native_id=name,
                reference_candidates=references_in(text + "\n" + target_text),
            )
        )
    return batch


def scan_gate_scripts(root: Path) -> ScanBatch:
    batch = ScanBatch()
    scripts_root = root / "scripts"
    if not scripts_root.is_dir():
        batch.diagnostics.append(
            diagnostic(
                "missing_scan_root",
                "scripts",
                1,
                "required scripts root is missing",
                severity="error",
            )
        )
        return batch
    for path in (
        item
        for item in source_files(root, "scripts", (".py", ".sh"))
        if item.parent == scripts_root and "gate" in item.name
    ):
        relative = relative_path(root, path)
        batch.input_paths.add(relative)
        try:
            text = path.read_text()
        except (OSError, UnicodeError) as exc:
            batch.diagnostics.append(diagnostic("source_read", relative, 1, str(exc), severity="error"))
            continue
        if path.suffix == ".py":
            command = f"python3 {relative}"
        else:
            command = relative
        batch.facts.append(
            make_fact(
                source_kind="gate_script",
                name=path.stem,
                path=relative,
                line=1,
                package=None,
                command_hint=command,
                command_hint_authority="file_entrypoint",
                discovery_confidence="filename_pattern",
                native_id=relative,
                reference_candidates=references_in(text),
            )
        )
    return batch


def scan_workflow_jobs(root: Path) -> ScanBatch:
    batch = ScanBatch()
    workflows_root = root / ".github/workflows"
    if not workflows_root.is_dir():
        batch.diagnostics.append(
            diagnostic(
                "missing_scan_root",
                ".github/workflows",
                1,
                "required workflow root is missing",
                severity="error",
            )
        )
        return batch
    paths = [
        path
        for path in source_files(root, ".github/workflows", (".yml", ".yaml"))
        if path.parent == workflows_root
    ]
    for path in paths:
        relative = relative_path(root, path)
        batch.input_paths.add(relative)
        try:
            text = path.read_text()
        except (OSError, UnicodeError) as exc:
            batch.diagnostics.append(diagnostic("source_read", relative, 1, str(exc), severity="error"))
            continue
        lines = text.splitlines()
        workflow_name = path.stem
        for line in lines:
            match = YAML_NAME_RE.match(line)
            if match:
                workflow_name = unquote_yaml_scalar(match.group(1))
                break
        jobs_start = next((index for index, line in enumerate(lines) if line.strip() == "jobs:" and not line.startswith(" ")), None)
        if jobs_start is None:
            batch.diagnostics.append(
                diagnostic(
                    "workflow_jobs_missing",
                    relative,
                    1,
                    "top-level jobs mapping is missing",
                    severity="error",
                )
            )
            continue
        jobs: list[tuple[str, int, int]] = []
        current: tuple[str, int] | None = None
        for index in range(jobs_start + 1, len(lines)):
            line = lines[index]
            if line and not line.startswith((" ", "\t", "#")):
                break
            match = WORKFLOW_JOB_RE.match(line)
            if match:
                if current:
                    jobs.append((current[0], current[1], index))
                current = (match.group(1), index + 1)
        if current:
            jobs.append((current[0], current[1], len(lines)))
        for job_id, line_number, end_index in jobs:
            block = "\n".join(lines[line_number - 1 : end_index])
            batch.facts.append(
                make_fact(
                    source_kind="github_workflow_job",
                    name=f"{workflow_name} / {job_id}",
                    path=relative,
                    line=line_number,
                    package=None,
                    command_hint=f"workflow job {relative}#{job_id}",
                    command_hint_authority="workflow_only",
                    discovery_confidence="yaml_job_indentation",
                    native_id=f"{relative}#{job_id}",
                    reference_candidates=references_in(block),
                )
            )
    return batch


def unquote_yaml_scalar(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        return value[1:-1]
    return value
