"""LLVM profile merge and JSON export for isolated trace attempts."""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
from collections.abc import Iterator, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .coverage_trace_resume import profile_digest


@dataclass(frozen=True)
class _CoverageExportJob:
    discovery_id: str
    attempt_dir: Path
    executable: Path
    profiles: tuple[Path, ...]
    exit_code: int


class CoverageExportStore(Mapping[str, Mapping[str, Any]]):
    """Merge, export, and discard one validated LLVM document at a time."""

    def __init__(
        self,
        *,
        jobs: Mapping[str, _CoverageExportJob],
        profdata_tool: Path,
        cov_tool: Path,
        resolved_sources: Sequence[str] | None,
    ) -> None:
        self._jobs = dict(sorted(jobs.items()))
        self._profdata_tool = profdata_tool
        self._cov_tool = cov_tool
        self._resolved_sources = (
            tuple(resolved_sources) if resolved_sources is not None else None
        )

    def __getitem__(self, discovery_id: str) -> Mapping[str, Any]:
        job = self._jobs[discovery_id]
        if job.exit_code != 0:
            raise ValueError(
                f"{discovery_id} failed attempt has no coverage attribution"
            )
        merged_descriptor, merged_temporary = tempfile.mkstemp(
            prefix=".merged.profdata.", suffix=".staged", dir=job.attempt_dir
        )
        os.close(merged_descriptor)
        merged_path = Path(merged_temporary)
        merged_path.unlink()
        export_path: Path | None = None
        try:
            merge = subprocess.run(
                [
                    str(self._profdata_tool),
                    "merge",
                    "-sparse",
                    *(str(profile) for profile in job.profiles),
                    "-o",
                    str(merged_path),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            if merge.returncode != 0:
                raise ValueError(
                    f"{discovery_id} llvm-profdata merge failed: "
                    f"{merge.stderr.strip()}"
                )
            export_descriptor, export_temporary = tempfile.mkstemp(
                prefix=".coverage.json.", suffix=".staged", dir=job.attempt_dir
            )
            export_path = Path(export_temporary)
            with os.fdopen(export_descriptor, "w", encoding="utf-8") as stream:
                exported = subprocess.run(
                    _llvm_cov_export_command(
                        cov_tool=self._cov_tool,
                        merged_profile=merged_path,
                        executable=job.executable,
                        resolved_sources=self._resolved_sources,
                    ),
                    check=False,
                    stdout=stream,
                    stderr=subprocess.PIPE,
                    text=True,
                )
                stream.flush()
                os.fsync(stream.fileno())
            if exported.returncode != 0:
                raise ValueError(
                    f"{discovery_id} llvm-cov export failed: "
                    f"{exported.stderr.strip()}"
                )
            try:
                document = json.loads(export_path.read_text())
            except (OSError, UnicodeError, json.JSONDecodeError) as error:
                raise ValueError(
                    f"{discovery_id} llvm-cov emitted invalid JSON"
                ) from error
            if not isinstance(document, Mapping):
                raise ValueError(f"{discovery_id} llvm-cov export must be an object")
            return document
        finally:
            for generated_path in (
                merged_path,
                job.attempt_dir / "merged.profdata",
                job.attempt_dir / "coverage.json",
            ):
                if generated_path.exists():
                    generated_path.unlink()
            if export_path is not None and export_path.exists():
                export_path.unlink()

    def __iter__(self) -> Iterator[str]:
        return iter(self._jobs)

    def __len__(self) -> int:
        return len(self._jobs)


def llvm_cov_export_command(
    *,
    cov_tool: Path,
    merged_profile: Path,
    executable: Path,
    root: Path,
    source_paths: Sequence[str],
) -> list[str]:
    """Build a repository-source-only export command with no function duplication."""

    return _llvm_cov_export_command(
        cov_tool=cov_tool,
        merged_profile=merged_profile,
        executable=executable,
        resolved_sources=_resolved_coverage_sources(root, source_paths),
    )


def _resolved_coverage_sources(root: Path, source_paths: Sequence[str]) -> tuple[str, ...]:
    repository_root = root.resolve()
    resolved_sources: list[str] = []
    for source_path in sorted(set(source_paths)):
        candidate = Path(source_path)
        if candidate.is_absolute():
            raise ValueError(
                f"coverage source path must be repository-relative: {source_path}"
            )
        resolved = (repository_root / candidate).resolve()
        if not resolved.is_relative_to(repository_root):
            raise ValueError(f"coverage source path escapes repository: {source_path}")
        if not resolved.is_file():
            raise ValueError(f"coverage source path is unavailable: {source_path}")
        resolved_sources.append(str(resolved))
    if not resolved_sources:
        raise ValueError("coverage export requires at least one repository source path")
    return tuple(resolved_sources)


def _llvm_cov_export_command(
    *,
    cov_tool: Path,
    merged_profile: Path,
    executable: Path,
    resolved_sources: Sequence[str] | None,
) -> list[str]:
    command = [
        str(cov_tool),
        "export",
        "-format=text",
        "-skip-functions",
        f"-instr-profile={merged_profile}",
        str(executable),
    ]
    if resolved_sources is not None:
        command.extend(["--sources", *resolved_sources])
    return command


def export_attempt_coverage(
    *,
    root: Path,
    trace_root: Path,
    llvm_tools_dir: Path,
    source_paths: Sequence[str] | None = None,
    allow_nonzero_exit: bool = False,
) -> Mapping[str, Mapping[str, Any]]:
    """Merge and export attempts without retaining the campaign in memory."""

    manifests = sorted((trace_root / "attempts").glob("*/attempt.json"))
    jobs: dict[str, _CoverageExportJob] = {}
    parsed: list[tuple[Path, Mapping[str, Any]]] = []
    for path in manifests:
        try:
            document = json.loads(path.read_text())
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise ValueError(f"cannot load attempt manifest {path}") from error
        if not isinstance(document, Mapping):
            raise ValueError(f"attempt manifest {path} must be an object")
        exit_code = document.get("exit_code")
        accepted_exit = (
            isinstance(exit_code, int)
            and not isinstance(exit_code, bool)
            and exit_code >= 0
            and (allow_nonzero_exit or exit_code == 0)
        )
        if document.get("status") != "completed" or not accepted_exit:
            raise ValueError(f"{path} is not a successful completed attempt")
        parsed.append((path, document))
    profdata_tool = llvm_tools_dir / "llvm-profdata"
    cov_tool = llvm_tools_dir / "llvm-cov"
    for tool in (profdata_tool, cov_tool):
        if not tool.is_file():
            raise ValueError(f"required LLVM tool is missing: {tool}")
    for path, manifest in parsed:
        discovery_id = manifest.get("discovery_id")
        if not isinstance(discovery_id, str) or not discovery_id:
            raise ValueError(f"{path} lacks discovery_id")
        if discovery_id in jobs:
            raise ValueError(f"attempt manifests duplicate {discovery_id}")
        executable_raw = manifest.get("test_executable")
        if not isinstance(executable_raw, str) or not executable_raw:
            raise ValueError(f"{discovery_id} lacks test executable")
        executable = Path(executable_raw)
        if not executable.is_absolute() or not executable.is_file():
            raise ValueError(f"{discovery_id} test executable is unavailable")
        attempt_dir = path.parent
        profiles = sorted((attempt_dir / "profiles").glob("*.profraw"))
        if not profiles:
            raise ValueError(f"{discovery_id} has no raw profiles")
        recorded_profile_digest = manifest.get("profile_digest")
        if recorded_profile_digest is not None:
            if recorded_profile_digest != profile_digest(attempt_dir / "profiles"):
                raise ValueError(f"{discovery_id} profile digest drifted")
        elif manifest["exit_code"] != 0:
            raise ValueError(f"{discovery_id} failed attempt lacks profile digest")
        jobs[discovery_id] = _CoverageExportJob(
            discovery_id=discovery_id,
            attempt_dir=attempt_dir,
            executable=executable,
            profiles=tuple(profiles),
            exit_code=manifest["exit_code"],
        )
    store = CoverageExportStore(
        jobs=jobs,
        profdata_tool=profdata_tool,
        cov_tool=cov_tool,
        resolved_sources=(
            _resolved_coverage_sources(root, source_paths)
            if source_paths is not None
            else None
        ),
    )
    for job in jobs.values():
        if job.exit_code != 0:
            _validate_failed_profiles(job, profdata_tool)
    return store


def _validate_failed_profiles(
    job: _CoverageExportJob,
    profdata_tool: Path,
) -> None:
    """Prove a failed attempt still emitted readable instrumentation data."""

    descriptor, temporary = tempfile.mkstemp(
        prefix=".failed.profdata.", suffix=".staged", dir=job.attempt_dir
    )
    os.close(descriptor)
    merged_path = Path(temporary)
    merged_path.unlink()
    try:
        merge = subprocess.run(
            [
                str(profdata_tool),
                "merge",
                "-sparse",
                *(str(profile) for profile in job.profiles),
                "-o",
                str(merged_path),
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        if merge.returncode != 0:
            raise ValueError(
                f"{job.discovery_id} failed-profile merge failed: "
                f"{merge.stderr.strip()}"
            )
    finally:
        if merged_path.exists():
            merged_path.unlink()
