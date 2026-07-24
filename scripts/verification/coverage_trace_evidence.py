"""Closed byte snapshots for focused Rust execution evidence."""

from __future__ import annotations

import hashlib
import json
import subprocess
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .focused_mapping_context import canonical_input_file


@dataclass(frozen=True)
class RawTraceEvidence:
    """Exact raw trace files and test artifacts captured in one pass."""

    trace_root: Path
    repository_originals: Mapping[str, bytes]
    executable_originals: Mapping[Path, bytes]
    tool_digests: Mapping[str, str]
    tool_paths: Mapping[str, str]
    digest: str


def capture_raw_trace_evidence(
    *,
    root: Path,
    trace_root: Path,
    llvm_tools_dir: Path,
) -> RawTraceEvidence:
    root = root.resolve()
    trace_root = canonical_input_file(
        root,
        trace_root,
        "focused trace evidence root",
        required_parent="target/gate-artifacts",
        require_directory=True,
    )
    requested = canonical_input_file(
        root, trace_root / "requested-tests.json", "focused trace requests"
    )
    config = canonical_input_file(
        root, trace_root / "nextest.toml", "focused trace nextest configuration"
    )
    manifests = sorted((trace_root / "attempts").glob("*/attempt.json"))
    if not manifests:
        raise ValueError("focused trace evidence has no attempt manifests")
    repository_originals = {
        requested.relative_to(root).as_posix(): requested.read_bytes(),
        config.relative_to(root).as_posix(): config.read_bytes(),
    }
    executable_paths: set[Path] = set()
    tool_paths = _trusted_llvm_tools(llvm_tools_dir)
    attempts = []
    for manifest_path in manifests:
        manifest_path = canonical_input_file(
            root, manifest_path, "focused trace attempt manifest"
        )
        manifest_bytes = manifest_path.read_bytes()
        try:
            manifest = json.loads(manifest_bytes)
        except (UnicodeError, json.JSONDecodeError) as error:
            raise ValueError("focused trace attempt manifest is unreadable") from error
        if not isinstance(manifest, Mapping):
            raise ValueError("focused trace attempt manifest must be an object")
        profiles_dir = manifest_path.parent / "profiles"
        profiles = sorted(profiles_dir.glob("*.profraw"))
        if not profiles:
            raise ValueError("focused trace attempt has no raw profiles")
        profile_bytes = {}
        for profile in profiles:
            canonical = canonical_input_file(
                root, profile, "focused raw LLVM profile"
            )
            content = canonical.read_bytes()
            relative = canonical.relative_to(root).as_posix()
            repository_originals[relative] = content
            profile_bytes[canonical.name] = content
        repository_originals[
            manifest_path.relative_to(root).as_posix()
        ] = manifest_bytes
        executable_raw = manifest.get("test_executable")
        if not isinstance(executable_raw, str) or not executable_raw:
            raise ValueError("focused trace attempt lacks test executable")
        executable = Path(executable_raw)
        if not executable.is_absolute() or not executable.is_file():
            raise ValueError("focused trace test executable is unavailable")
        executable_paths.add(executable.resolve())
        recorded = manifest.get("test_executable_digest")
        actual = _file_digest(executable)
        if recorded != actual:
            raise ValueError("focused trace test executable digest drifted")
        current_profile_digest = _profile_digest(profile_bytes)
        if manifest.get("profile_digest") != current_profile_digest:
            raise ValueError("focused trace profile digest drifted")
        attempts.append(
            {
                "manifest": manifest_path.relative_to(trace_root).as_posix(),
                "manifest_digest": _digest(manifest_bytes),
                "profile_digest": current_profile_digest,
                "test_executable": executable.as_posix(),
                "test_executable_digest": actual,
            }
        )
    repository_originals = dict(sorted(repository_originals.items()))
    executable_originals = {
        path: path.read_bytes()
        for path in sorted(executable_paths | set(tool_paths.values()))
    }
    tool_digests = {
        name: _digest(path.read_bytes()) for name, path in tool_paths.items()
    }
    serialized_tool_paths = {
        name: path.as_posix() for name, path in tool_paths.items()
    }
    payload = {
        "kind": "focused_raw_trace_evidence_v1",
        "trace_root": trace_root.relative_to(root).as_posix(),
        "repository_files": {
            path: _digest(content)
            for path, content in sorted(repository_originals.items())
        },
        "executables": {
            path.as_posix(): _digest(content)
            for path, content in sorted(
                executable_originals.items(), key=lambda item: item[0].as_posix()
            )
        },
        "llvm_tools": {
            name: {
                "path": path.as_posix(),
                "digest": tool_digests[name],
            }
            for name, path in sorted(tool_paths.items())
        },
        "attempts": attempts,
    }
    return RawTraceEvidence(
        trace_root=trace_root,
        repository_originals=repository_originals,
        executable_originals=executable_originals,
        tool_digests=tool_digests,
        tool_paths=serialized_tool_paths,
        digest=_canonical_digest(payload),
    )


def validate_external_evidence(
    evidence: RawTraceEvidence,
) -> list[str]:
    return [
        f"focused trace executable changed after snapshot: {path}"
        for path, expected in evidence.executable_originals.items()
        if not path.is_file() or path.read_bytes() != expected
    ]


def _file_digest(path: Path) -> str:
    return _digest(path.read_bytes())


def _profile_digest(profiles: Mapping[str, bytes]) -> str:
    if not profiles:
        raise ValueError("focused trace attempt has no raw profiles")
    digest = hashlib.sha256()
    for name, content in sorted(profiles.items()):
        digest.update(name.encode())
        digest.update(b"\0")
        digest.update(content)
        digest.update(b"\0")
    return "sha256:" + digest.hexdigest()


def _trusted_llvm_tools(llvm_tools_dir: Path) -> dict[str, Path]:
    directory = llvm_tools_dir.resolve(strict=True)
    if not directory.is_dir():
        raise ValueError("focused LLVM tools path must be a directory")
    trusted_directory = _rustc_llvm_tools_directory()
    if directory != trusted_directory:
        raise ValueError(
            "focused LLVM tools must come from the active rustc toolchain"
        )
    result = {}
    for name in ("llvm-profdata", "llvm-cov"):
        path = (directory / name).resolve(strict=True)
        if path.parent != directory or not path.is_file() or path.is_symlink():
            raise ValueError(f"focused LLVM tool is not canonical: {name}")
        result[name] = path
    return result


def _rustc_llvm_tools_directory() -> Path:
    sysroot_result = subprocess.run(
        ["rustc", "--print", "sysroot"],
        check=False,
        capture_output=True,
        text=True,
    )
    verbose_result = subprocess.run(
        ["rustc", "-vV"],
        check=False,
        capture_output=True,
        text=True,
    )
    if sysroot_result.returncode != 0 or verbose_result.returncode != 0:
        raise ValueError("active rustc toolchain identity is unavailable")
    sysroot = sysroot_result.stdout.strip()
    verbose = verbose_result.stdout
    host = next(
        (
            line.removeprefix("host:").strip()
            for line in verbose.splitlines()
            if line.startswith("host:")
        ),
        "",
    )
    if not sysroot or not host:
        raise ValueError("active rustc toolchain identity is unavailable")
    directory = (
        Path(sysroot) / "lib" / "rustlib" / host / "bin"
    ).resolve(strict=True)
    if not directory.is_dir():
        raise ValueError("active rustc LLVM tools directory is unavailable")
    return directory


def _digest(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _canonical_digest(value: Any) -> str:
    content = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()
    return _digest(content)
