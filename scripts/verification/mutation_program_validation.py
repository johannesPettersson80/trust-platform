"""File-level at-rest validation for Phase 10 mutation-program reports."""

from __future__ import annotations

import hashlib
import json
import subprocess
from collections.abc import Mapping
from pathlib import Path, PurePosixPath

from .mutation_program_live import build_live_mutation_program_state, validate_source_revision
from .mutation_program_report import DEFAULT_JSON_PATH, DEFAULT_MARKDOWN_PATH, render_markdown
from .mutation_program_report_contract import validate_report_payload, validate_schema_contract
from .report_input_contract import validate_bound_input_paths
from .test_catalog_common import input_digest
from .test_catalog_json_schema import validate_json_schema_instance


REPORT_SCHEMA_PATH = "verification/schemas/mutation-survivor-report.schema.json"


def validate_report_files(
    root: Path,
    json_path: Path,
    markdown_path: Path,
    schema_path: Path,
) -> list[str]:
    root = root.resolve()
    failures: list[str] = []
    if schema_path.as_posix() != REPORT_SCHEMA_PATH:
        failures.append("schema path must identify the committed mutation survivor report schema")
    json_file = _safe_report_path(root, json_path, "JSON", failures)
    markdown_file = _safe_report_path(root, markdown_path, "Markdown", failures)
    schema_file = _safe_report_path(root, schema_path, "schema", failures)
    if json_file is None or markdown_file is None or schema_file is None:
        return sorted(set(failures))
    if json_file == markdown_file:
        return sorted(set([*failures, "JSON and Markdown paths must be distinct"]))
    try:
        json_text = json_file.read_text()
        payload = json.loads(json_text)
    except (OSError, json.JSONDecodeError) as exc:
        return [f"mutation survivor report JSON cannot be read: {exc}"]
    canonical = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if json_text != canonical:
        failures.append("mutation survivor report JSON must use canonical sorted-key formatting")
    try:
        schema = json.loads(schema_file.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        return [f"mutation survivor report schema cannot be read: {exc}"]
    failures.extend(validate_schema_contract(schema))
    failures.extend(validate_json_schema_instance(payload, schema))
    try:
        expected_state = build_live_mutation_program_state(root, require_clean_commit=False)
    except (OSError, ValueError) as exc:
        failures.append(f"current live Phase 10 state cannot be built: {exc}")
        expected_state = None
    failures.extend(validate_report_payload(payload, expected_state=expected_state))
    input_paths_value = payload.get("input_paths") if isinstance(payload, Mapping) else None
    if isinstance(input_paths_value, list) and all(isinstance(item, str) for item in input_paths_value):
        paths = tuple(input_paths_value)
        failures.extend(validate_bound_input_paths(root, paths))
        if payload.get("input_digest") != input_digest(root, list(paths)):
            failures.append("input_digest does not match current bound input contents")
        failures.extend(validate_source_revision(root, payload.get("commit"), paths))
    output_paths = payload.get("output_paths") if isinstance(payload, Mapping) else None
    if isinstance(output_paths, Mapping):
        if output_paths.get("json") != DEFAULT_JSON_PATH.as_posix():
            failures.append("JSON path does not match the canonical report output path")
        if output_paths.get("markdown") != DEFAULT_MARKDOWN_PATH.as_posix():
            failures.append("Markdown path does not match the canonical report output path")
    if isinstance(payload, Mapping):
        failures.extend(_validate_artifacts_and_resolutions(root, payload))
    try:
        markdown_text = markdown_file.read_text()
    except OSError as exc:
        failures.append(f"mutation survivor report Markdown cannot be read: {exc}")
    else:
        digest = hashlib.sha256(json_text.encode()).hexdigest()
        try:
            expected_markdown = render_markdown(payload, json_digest=digest)
        except Exception as exc:
            failures.append(f"mutation survivor report Markdown cannot be reconstructed: {exc}")
        else:
            if markdown_text != expected_markdown:
                failures.append("Markdown does not exactly match the canonical JSON render")
    return sorted(set(failures))


def _validate_artifacts_and_resolutions(root: Path, payload: Mapping) -> list[str]:
    failures: list[str] = []
    shards = payload.get("shards")
    if isinstance(shards, list):
        for index, shard in enumerate(shards):
            if not isinstance(shard, Mapping):
                continue
            artifact = shard.get("result_artifact")
            if isinstance(artifact, Mapping):
                _validate_file_digest(root, artifact.get("path"), artifact.get("sha256"), f"shards[{index}] result artifact", failures)
            confirmation = shard.get("delivered_build_confirmation")
            if isinstance(confirmation, Mapping):
                _validate_file_digest(
                    root,
                    confirmation.get("artifact_path"),
                    confirmation.get("artifact_sha256"),
                    f"shards[{index}] delivered artifact",
                    failures,
                )
    survivors = payload.get("survivors")
    if isinstance(survivors, list):
        for index, survivor in enumerate(survivors):
            if not isinstance(survivor, Mapping):
                continue
            value = survivor.get("resolution_ref")
            path = _safe_existing_file(root, value)
            if path is None:
                failures.append(f"survivors[{index}] resolution_ref is not a durable regular file")
                continue
            tracked = subprocess.run(
                ["git", "-C", str(root), "ls-files", "--error-unmatch", str(value)],
                check=False,
                capture_output=True,
            )
            ignored = subprocess.run(
                [
                    "git",
                    "-C",
                    str(root),
                    "check-ignore",
                    "--no-index",
                    "-q",
                    "--",
                    str(value),
                ],
                check=False,
                capture_output=True,
            )
            if tracked.returncode != 0 or ignored.returncode == 0:
                failures.append(f"survivors[{index}] resolution_ref must be tracked and not ignored")
    return failures


def _validate_file_digest(
    root: Path,
    path_value: object,
    digest_value: object,
    label: str,
    failures: list[str],
) -> None:
    path = _safe_existing_file(root, path_value)
    if path is None:
        failures.append(f"{label} is not a safe regular file")
        return
    actual = "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
    if digest_value != actual:
        failures.append(f"{label} SHA-256 does not match the bound file")


def _safe_existing_file(root: Path, value: object) -> Path | None:
    if not isinstance(value, str) or not value or "\\" in value:
        return None
    relative = PurePosixPath(value)
    if relative.is_absolute() or ".." in relative.parts or "." in relative.parts:
        return None
    candidate = root
    for part in relative.parts:
        candidate /= part
        if candidate.is_symlink():
            return None
    try:
        resolved = candidate.resolve(strict=True)
        resolved.relative_to(root)
    except (OSError, ValueError):
        return None
    return resolved if resolved.is_file() else None


def _safe_report_path(
    root: Path,
    value: Path,
    label: str,
    failures: list[str],
) -> Path | None:
    raw = value.as_posix()
    relative = PurePosixPath(raw)
    if (
        not relative.parts
        or value.is_absolute()
        or "\\" in raw
        or ".." in relative.parts
        or "." in relative.parts
    ):
        failures.append(f"{label} path must be normalized and workspace-relative")
        return None
    candidate = root
    for part in relative.parts:
        candidate /= part
        if candidate.is_symlink():
            failures.append(f"{label} path must not contain a symlink")
            return None
    try:
        candidate.resolve(strict=False).relative_to(root)
    except ValueError:
        failures.append(f"{label} path escapes the workspace")
        return None
    return candidate
