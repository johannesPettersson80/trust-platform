"""Shared provenance closure and path checks for verification reports."""

from __future__ import annotations

from collections.abc import Iterable
from pathlib import Path, PurePosixPath


def validator_code_input_paths(root: Path) -> set[str]:
    """Return validator code inputs without mutable evidence/data registries."""

    root = root.resolve()
    paths: set[str] = {
        ".github/workflows/ci.yml",
        ".github/workflows/salsa-hardening.yml",
        "Cargo.toml",
        "crates/trust-ads-server/fuzz/.gitignore",
        "docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md",
        "fuzz/.gitignore",
        "scripts/gen_cases.py",
        "scripts/runtime_comms_fuzz_gate.sh",
        "scripts/runtime_vm_malformed_bytecode_fuzz_gate.sh",
        "scripts/salsa_fuzz_gate.sh",
        "scripts/validate_verification_metadata.py",
        "verification/malformed-input-taxonomy.md",
        "verification/malformed-input-taxonomy.toml",
        "verification/schemas/malformed-input-taxonomy.schema.json",
        "verification/runtime-anomaly-taxonomy.toml",
        "verification/schemas/runtime-anomaly-taxonomy.schema.json",
        "verification/fuzz-program.toml",
        "verification/schemas/fuzz-program.schema.json",
        "verification/gate-inventory.toml",
        "verification/mutation-program.toml",
        "verification/schemas/mutation-program.schema.json",
    }
    paths.update(
        path
        for path in _files_under(root, root / "scripts/verification")
        if path.endswith(".py")
    )
    return paths


def validate_bound_input_paths(root: Path, input_paths: Iterable[str]) -> list[str]:
    """Reject missing, escaping, or symlinked report inputs."""

    root = root.resolve()
    failures: list[str] = []
    for value in sorted(set(input_paths)):
        if not _is_safe_relative_path(value):
            failures.append(f"input path must be normalized and workspace-relative: {value!r}")
            continue
        candidate = root
        for part in PurePosixPath(value).parts:
            candidate /= part
            if candidate.is_symlink():
                failures.append(f"input path contains a symlink component: {value}")
                break
        try:
            resolved = (root / value).resolve(strict=True)
        except OSError as exc:
            failures.append(f"input path cannot be resolved as a regular file: {value}: {exc}")
            continue
        try:
            resolved.relative_to(root)
        except ValueError:
            failures.append(f"input path escapes the workspace after resolution: {value}")
        if not resolved.is_file():
            failures.append(f"input path is not a regular file: {value}")
    return sorted(set(failures))


def resolve_report_output_path(
    root: Path,
    value: Path,
    label: str,
) -> tuple[str, Path]:
    """Return a canonical relative path and contained destination for a report."""

    root = root.resolve()
    raw = value.as_posix()
    relative = PurePosixPath(raw)
    if (
        not relative.parts
        or value.is_absolute()
        or "\\" in raw
        or ".." in relative.parts
        or "." in relative.parts
    ):
        raise ValueError(
            f"{label} output path must be normalized and workspace-relative"
        )
    candidate = root
    for part in relative.parts:
        candidate /= part
        if candidate.is_symlink():
            raise ValueError(f"{label} output path must not contain a symlink")
    try:
        candidate.resolve(strict=False).relative_to(root)
    except ValueError as exc:
        raise ValueError(f"{label} output path escapes the workspace") from exc
    return raw, candidate


def _files_under(root: Path, directory: Path) -> set[str]:
    if not directory.is_dir():
        return set()
    return {
        path.relative_to(root).as_posix()
        for path in directory.rglob("*")
        if (path.is_file() or path.is_symlink())
        and "__pycache__" not in path.parts
        and path.suffix != ".pyc"
    }


def _is_safe_relative_path(value: str) -> bool:
    if not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return not path.is_absolute() and ".." not in path.parts and "." not in path.parts
