"""Shared helpers for mechanical test-catalog source scanners."""

from __future__ import annotations

import hashlib
import re
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

from .test_catalog_models import InferredTestFact, ScanDiagnostic


REFERENCE_RE = re.compile(
    r"\b(?:VERIF-[A-Z0-9-]+|MP-[0-9A-Z-]+|EVID_[A-Z0-9_]+|"
    r"SPEC_GAP_[A-Z0-9_]+|TEST_[A-Z0-9_]+|RISK_[A-Z0-9_]+|"
    r"SEAM-(?:TEST|IMPL)-[A-Z0-9-]+)\b"
)
REFERENCE_PATH_RE = re.compile(
    r"docs/internal/testing/(?:checklists|evidence)/[A-Za-z0-9_./-]+"
)


@dataclass
class ScanBatch:
    facts: list[InferredTestFact] = field(default_factory=list)
    diagnostics: list[ScanDiagnostic] = field(default_factory=list)
    input_paths: set[str] = field(default_factory=set)

    def extend(self, other: "ScanBatch") -> None:
        self.facts.extend(other.facts)
        self.diagnostics.extend(other.diagnostics)
        self.input_paths.update(other.input_paths)


def make_fact(
    *,
    source_kind: str,
    name: str,
    native_id: str | None = None,
    path: str,
    line: int,
    package: str | None,
    command_hint: str,
    command_hint_authority: str,
    discovery_confidence: str,
    ignore_state: str = "not_ignored",
    ignore_reason: str | None = None,
    reference_candidates: tuple[str, ...] = (),
) -> InferredTestFact:
    native_id = native_id or f"{path}#{name}"
    identity = f"{source_kind}\0{package or ''}\0{native_id}".encode()
    stable_id = "DISC_" + hashlib.sha256(identity).hexdigest()[:20].upper()
    return InferredTestFact(
        stable_id=stable_id,
        native_id=native_id,
        source_kind=source_kind,
        name=name,
        path=path,
        line=line,
        package=package,
        command_hint=command_hint,
        command_hint_authority=command_hint_authority,
        discovery_confidence=discovery_confidence,
        ignore_state=ignore_state,
        ignore_reason=ignore_reason,
        reference_candidates=reference_candidates,
    )


def relative_path(root: Path, path: Path) -> str:
    return path.resolve().relative_to(root.resolve()).as_posix()


def references_in(text: str) -> tuple[str, ...]:
    references = set(REFERENCE_RE.findall(text))
    references.update(match.rstrip(".,;:)") for match in REFERENCE_PATH_RE.findall(text))
    return tuple(sorted(references))


def diagnostic(
    kind: str,
    path: str,
    line: int,
    message: str,
    *,
    severity: str = "warning",
) -> ScanDiagnostic:
    return ScanDiagnostic(severity=severity, kind=kind, path=path, line=line, message=message)


def source_files(root: Path, relative_root: str, suffixes: tuple[str, ...]) -> list[Path]:
    """Return tracked files when possible, with a fixture-friendly filesystem fallback."""

    try:
        result = subprocess.run(
            [
                "git",
                "-C",
                str(root),
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
                "--",
                relative_root,
            ],
            check=False,
            capture_output=True,
        )
    except OSError:
        result = None
    if result is not None and result.returncode == 0:
        paths = [root / item.decode() for item in result.stdout.split(b"\0") if item]
        return sorted(path for path in paths if path.is_file() and path.suffix in suffixes)
    directory = root / relative_root
    if not directory.is_dir():
        return []
    return sorted(path for path in directory.rglob("*") if path.is_file() and path.suffix in suffixes)


def input_digest(root: Path, paths: list[str]) -> str:
    digest = hashlib.sha256()
    for relative in paths:
        digest.update(relative.encode())
        digest.update(b"\0")
        try:
            content = (root / relative).read_bytes()
        except OSError as exc:
            content = f"<unreadable:{type(exc).__name__}>".encode()
        digest.update(hashlib.sha256(content).digest())
        digest.update(b"\0")
    return f"sha256:{digest.hexdigest()}"
