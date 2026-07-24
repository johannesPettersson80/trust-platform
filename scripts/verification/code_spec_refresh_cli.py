"""CLI for generating schema-closed code-spec refresh proposals."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

from .code_spec_refresh import (
    CodeSpecRefreshError,
    build_refresh_proposals,
    canonical_json,
    validate_refresh_proposals,
)
from .code_spec_provenance import (
    CodeSpecProvenanceError,
    PRIVATE_INPUT_MANIFEST_PATH,
    input_closure_digest,
    load_private_input_manifest,
    validate_clean_source_revision,
)
from .code_spec_report import validate_report_bytes
from .test_catalog_json_schema import validate_json_schema_instance


DEFAULT_REPORT_JSON = (
    "target/gate-artifacts/code-spec-coverage/code-spec-coverage.json"
)
DEFAULT_REPORT_MARKDOWN = (
    "target/gate-artifacts/code-spec-coverage/code-spec-coverage.md"
)
DEFAULT_OUTPUT = (
    "target/gate-artifacts/code-spec-refresh/code-spec-refresh-proposals.json"
)
CODE_MAP_PATH = "verification/code-spec-map.toml"
REPORT_SCHEMA_PATH = "verification/schemas/code-spec-coverage-report.schema.json"
REFRESH_SCHEMA_PATH = "verification/schemas/code-spec-refresh-proposals.schema.json"


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate review-only code-to-spec refresh proposals."
    )
    parser.add_argument("--root", default=".")
    parser.add_argument("--report-json", default=DEFAULT_REPORT_JSON)
    parser.add_argument("--report-markdown", default=DEFAULT_REPORT_MARKDOWN)
    parser.add_argument("--output", default=DEFAULT_OUTPUT)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    root = Path(args.root).resolve()
    try:
        report_path = _repo_path(root, args.report_json, must_exist=True)
        markdown_path = _repo_path(root, args.report_markdown, must_exist=True)
        output_path = _repo_path(root, args.output, must_exist=False)
        code_map_path = _repo_path(root, CODE_MAP_PATH, must_exist=True)
        report_schema_path = _repo_path(
            root, REPORT_SCHEMA_PATH, must_exist=True
        )
        refresh_schema_path = _repo_path(
            root, REFRESH_SCHEMA_PATH, must_exist=True
        )
        _require_quarantine_output(
            root,
            output_path,
            inputs=(
                report_path,
                markdown_path,
                code_map_path,
                report_schema_path,
                refresh_schema_path,
            ),
        )
        report_bytes = report_path.read_bytes()
        markdown = markdown_path.read_text(encoding="utf-8")
        report = json.loads(report_bytes)
        report_schema = json.loads(report_schema_path.read_text(encoding="utf-8"))
        report_failures = validate_json_schema_instance(report, report_schema)
        report_failures.extend(validate_report_bytes(report_bytes, markdown))
        if report_failures:
            raise CodeSpecRefreshError(
                "source report validation failed: "
                + "; ".join(sorted(set(report_failures)))
            )
        _verify_source_commit(root, str(report.get("commit", "")))
        _verify_report_provenance(root, report)
        code_map_bytes = code_map_path.read_bytes()
        _verify_tracked_bytes(
            root=root,
            commit=str(report["commit"]),
            relative_path=CODE_MAP_PATH,
            current=code_map_bytes,
        )
        payload = build_refresh_proposals(
            report=report,
            source_report_path=report_path.relative_to(root).as_posix(),
            source_report_sha256=_sha256(report_bytes),
            code_map=tomllib.loads(code_map_bytes.decode("utf-8")),
            code_map_sha256=_sha256(code_map_bytes),
        )
        refresh_schema = json.loads(
            refresh_schema_path.read_text(encoding="utf-8")
        )
        failures = validate_json_schema_instance(payload, refresh_schema)
        failures.extend(
            validate_refresh_proposals(
                payload,
                source_report=report,
                source_report_path=report_path.relative_to(root).as_posix(),
                source_report_sha256=_sha256(report_bytes),
                source_code_map=tomllib.loads(code_map_bytes.decode("utf-8")),
                source_code_map_sha256=_sha256(code_map_bytes),
            )
        )
        if failures:
            raise CodeSpecRefreshError(
                "generated refresh proposals failed validation: "
                + "; ".join(sorted(set(failures)))
            )
        _atomic_write(output_path, canonical_json(payload).encode("utf-8"))
    except (
        CodeSpecRefreshError,
        OSError,
        UnicodeError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
    ) as error:
        print(f"code-spec refresh proposal generation failed: {error}", file=sys.stderr)
        return 1
    summary = payload["summary"]
    print(
        "code-spec refresh proposals generated: "
        f"missing={summary['missing_mapping_proposals']} "
        f"rebinds={summary['digest_rebind_proposals']} "
        f"stale_identities={summary['stale_identity_proposals']} "
        f"semantic_groups={summary['semantic_review_groups']} "
        f"output={output_path.relative_to(root).as_posix()}"
    )
    return 0


def _repo_path(root: Path, value: str, *, must_exist: bool) -> Path:
    candidate = Path(value)
    if candidate.is_absolute():
        raise CodeSpecRefreshError("paths must be repository-relative")
    unresolved = root
    for part in candidate.parts:
        unresolved /= part
        if unresolved.is_symlink():
            raise CodeSpecRefreshError(f"path cannot traverse a symlink: {value}")
    resolved = unresolved.resolve()
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise CodeSpecRefreshError(f"path escapes repository root: {value}") from error
    if must_exist and not resolved.is_file():
        raise CodeSpecRefreshError(f"required input is not a file: {value}")
    if not must_exist and resolved.exists() and not resolved.is_file():
        raise CodeSpecRefreshError(f"output is not a file: {value}")
    return resolved


def _require_quarantine_output(
    root: Path,
    output: Path,
    *,
    inputs: tuple[Path, ...],
) -> None:
    if output in inputs:
        raise CodeSpecRefreshError("proposal output cannot overwrite an input")
    artifact_root = (root / "target/gate-artifacts/code-spec-refresh").resolve()
    if output == artifact_root:
        raise CodeSpecRefreshError(
            "proposal output must be a file below the dedicated "
            "code-spec-refresh quarantine"
        )
    try:
        output.relative_to(artifact_root)
    except ValueError as error:
        raise CodeSpecRefreshError(
            "proposal output must stay inside the dedicated code-spec-refresh "
            "quarantine"
        ) from error


def _verify_source_commit(root: Path, source_commit: str) -> None:
    result = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or result.stdout.strip() != source_commit:
        raise CodeSpecRefreshError(
            "source report commit does not match current HEAD"
        )


def _verify_report_provenance(root: Path, report: dict) -> None:
    raw_paths = report.get("input_paths")
    if not isinstance(raw_paths, list) or any(
        not isinstance(path, str) or not path for path in raw_paths
    ):
        raise CodeSpecRefreshError("source report input_paths are malformed")
    input_paths = tuple(raw_paths)
    if CODE_MAP_PATH not in input_paths:
        raise CodeSpecRefreshError("source report input closure omits code-spec map")
    try:
        private_inputs = (
            load_private_input_manifest(root)
            if PRIVATE_INPUT_MANIFEST_PATH in input_paths
            else {}
        )
        failures = validate_clean_source_revision(
            root,
            str(report.get("commit", "")),
            input_paths,
            allowed_untracked=private_inputs,
        )
        live_digest = input_closure_digest(root, input_paths)
    except CodeSpecProvenanceError as error:
        raise CodeSpecRefreshError(
            f"source report input closure is invalid: {error}"
        ) from error
    if failures:
        raise CodeSpecRefreshError(
            "source report provenance failed: " + "; ".join(failures)
        )
    if live_digest != report.get("input_digest"):
        raise CodeSpecRefreshError(
            "source report input_digest differs from exact input closure"
        )


def _verify_tracked_bytes(
    *,
    root: Path,
    commit: str,
    relative_path: str,
    current: bytes,
) -> None:
    result = subprocess.run(
        ["git", "-C", str(root), "show", f"{commit}:{relative_path}"],
        check=False,
        capture_output=True,
    )
    if result.returncode != 0 or result.stdout != current:
        raise CodeSpecRefreshError(
            f"{relative_path} bytes differ from source report commit"
        )


def _sha256(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _atomic_write(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(
        prefix=f".{path.name}.",
        suffix=".staged",
        dir=path.parent,
    )
    temporary_path = Path(temporary)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary_path, path)
    finally:
        if temporary_path.exists():
            temporary_path.unlink()
