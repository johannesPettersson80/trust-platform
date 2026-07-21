"""Input closure and source-revision checks for Phase 17 reports."""

from __future__ import annotations

import hashlib
import re
import subprocess
import tomllib
from pathlib import Path
from typing import Iterable, Mapping

from .code_spec_common import DIGEST_PATTERN, normalize_repo_path, sha256_digest


class CodeSpecProvenanceError(ValueError):
    """Raised when report provenance is not reproducible."""


PRIVATE_INPUT_MANIFEST_PATH = "verification/code-spec-private-inputs.toml"


def load_private_input_manifest(root: Path) -> dict[str, str]:
    """Load exact digest bindings for intentionally ignored report inputs."""

    path = root / PRIVATE_INPUT_MANIFEST_PATH
    try:
        document = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise CodeSpecProvenanceError(f"cannot load private input manifest: {error}") from error
    if set(document) != {"schema_version", "inputs"}:
        raise CodeSpecProvenanceError("private input manifest root fields drift")
    if document.get("schema_version") != 1:
        raise CodeSpecProvenanceError("private input manifest schema_version must equal 1")
    records = document.get("inputs")
    if not isinstance(records, list):
        raise CodeSpecProvenanceError("private input manifest inputs must be an array")
    bindings: dict[str, str] = {}
    expected_fields = {"path", "digest", "owner", "reason", "last_reviewed"}
    for index, record in enumerate(records):
        if not isinstance(record, Mapping) or set(record) != expected_fields:
            raise CodeSpecProvenanceError(f"private input manifest inputs[{index}] fields drift")
        raw_path = record.get("path")
        digest = record.get("digest")
        owner = record.get("owner")
        reason = record.get("reason")
        last_reviewed = record.get("last_reviewed")
        if not isinstance(raw_path, str):
            raise CodeSpecProvenanceError(f"private input manifest inputs[{index}] path is invalid")
        try:
            normalized = normalize_repo_path(raw_path)
        except ValueError as error:
            raise CodeSpecProvenanceError(str(error)) from error
        if normalized in bindings:
            raise CodeSpecProvenanceError(f"private input manifest duplicates {normalized}")
        if not isinstance(digest, str) or re.fullmatch(DIGEST_PATTERN, digest) is None:
            raise CodeSpecProvenanceError(f"private input manifest digest is invalid: {normalized}")
        if not isinstance(owner, str) or not owner or not isinstance(reason, str) or not reason:
            raise CodeSpecProvenanceError(f"private input manifest review metadata is invalid: {normalized}")
        if not isinstance(last_reviewed, str) or re.fullmatch(r"\d{4}-\d{2}-\d{2}", last_reviewed) is None:
            raise CodeSpecProvenanceError(f"private input manifest review date is invalid: {normalized}")
        bindings[normalized] = digest
    return dict(sorted(bindings.items()))


def input_closure_digest(root: Path, paths: Iterable[str]) -> str:
    normalized = []
    for raw_path in paths:
        try:
            normalized.append(normalize_repo_path(raw_path))
        except ValueError as error:
            raise CodeSpecProvenanceError(str(error)) from error
    if len(normalized) != len(set(normalized)):
        raise CodeSpecProvenanceError("input closure contains duplicate path identities")
    digest = hashlib.sha256()
    for path in sorted(normalized):
        content = _read_input(root, path)
        encoded_path = path.encode("utf-8")
        digest.update(len(encoded_path).to_bytes(8, "big"))
        digest.update(encoded_path)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return "sha256:" + digest.hexdigest()


def validate_private_input_bindings(
    root: Path,
    bindings: Mapping[str, str],
) -> list[str]:
    """Validate exact bytes for intentionally ignored, non-Git inputs."""

    failures: list[str] = []
    for raw_path, digest in sorted(bindings.items()):
        try:
            path = normalize_repo_path(raw_path)
        except ValueError as error:
            failures.append(f"private input manifest path is invalid: {error}")
            continue
        if re.fullmatch(DIGEST_PATTERN, digest) is None:
            failures.append(f"private input manifest digest is invalid: {path}")
            continue
        tracked = _git(root, "ls-files", "--error-unmatch", "--", path)
        if tracked.returncode == 0:
            failures.append(f"private input manifest redundantly names tracked path: {path}")
            continue
        ignored = _git(root, "check-ignore", "-q", "--", path)
        if ignored.returncode != 0:
            failures.append(f"private report input is not intentionally ignored: {path}")
            continue
        try:
            live_digest = sha256_digest(_read_input(root, path))
        except CodeSpecProvenanceError as error:
            failures.append(str(error))
            continue
        if live_digest != digest:
            failures.append(
                f"private report input digest changed: {path} "
                f"expected={digest} live={live_digest}"
            )
    return sorted(set(failures))


def validate_clean_source_revision(
    root: Path,
    commit: str,
    paths: Iterable[str],
    *,
    allowed_untracked: Mapping[str, str] | None = None,
) -> list[str]:
    failures: list[str] = []
    allowed_untracked = allowed_untracked or {}
    normalized = list(paths)
    normalized_allowed: dict[str, str] = {}
    for raw_path, digest in allowed_untracked.items():
        try:
            path = normalize_repo_path(raw_path)
        except ValueError as error:
            failures.append(f"private input manifest path is invalid: {error}")
            continue
        if re.fullmatch(DIGEST_PATTERN, digest) is None:
            failures.append(f"private input manifest digest is invalid: {path}")
            continue
        normalized_allowed[path] = digest
    for path in sorted(set(normalized_allowed) - set(normalized)):
        failures.append(f"private input manifest names path outside report closure: {path}")
    failures.extend(validate_private_input_bindings(root, normalized_allowed))
    if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
        failures.append("commit must identify a clean full Git SHA")
    head = _git(root, "rev-parse", "HEAD")
    if head.returncode != 0:
        failures.append("repository HEAD cannot be resolved")
    elif head.stdout.strip() != commit:
        failures.append("report commit does not equal repository HEAD")
    status = _git(root, "status", "--porcelain", "--untracked-files=all")
    if status.returncode != 0:
        failures.append("repository clean state cannot be determined")
    elif status.stdout:
        failures.append("repository must be clean for report provenance")
    try:
        input_closure_digest(root, normalized)
    except CodeSpecProvenanceError as error:
        failures.append(str(error))
    for path in sorted(set(normalized)):
        tracked = _git(root, "ls-files", "--error-unmatch", "--", path)
        if tracked.returncode != 0 and path not in normalized_allowed:
            failures.append(f"report input is not tracked: {path}")
    return sorted(set(failures))


def _read_input(root: Path, path: str) -> bytes:
    current = root
    for part in Path(path).parts:
        current = current / part
        if current.is_symlink():
            raise CodeSpecProvenanceError(f"input closure rejects symlink path {path}")
    try:
        canonical_root = root.resolve(strict=True)
        resolved = (root / path).resolve(strict=True)
        resolved.relative_to(canonical_root)
    except (FileNotFoundError, ValueError) as error:
        raise CodeSpecProvenanceError(f"input closure path is unreadable or escapes: {path}") from error
    if not resolved.is_file():
        raise CodeSpecProvenanceError(f"input closure path is not a file: {path}")
    return resolved.read_bytes()


def _git(root: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", "-C", str(root), *arguments],
        check=False,
        capture_output=True,
        text=True,
    )
