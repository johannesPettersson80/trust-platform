"""Audit explicit Mocha registration for VS Code extension test files."""

from __future__ import annotations

import re
from collections import Counter
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

from .test_catalog_common import diagnostic, source_files
from .test_catalog_models import ScanDiagnostic
from .test_catalog_vscode import mask_js_literals, scan_vscode_tests, strip_js_comments


INDEX_PATH = "editors/vscode/src/test/suite/index.ts"
SUITE_PATH = "editors/vscode/src/test/suite"
PRE_REQUIRE_TOKEN = 'mocha.suite.emit("pre-require"'
RUN_TOKEN = "return new Promise"
LITERAL_REQUIRE_RE = re.compile(r"^  require\((?P<quote>['\"])(?P<path>.+?)(?P=quote)\);\s*$")
SAFE_SPECIFIER_RE = re.compile(r"^\./[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*\.test$")


@dataclass(frozen=True)
class VscodeRegistrationEntry:
    specifier: str
    source_line: int
    resolved_path: str


@dataclass(frozen=True)
class VscodeRegistrationAudit:
    index_path: str
    test_files: tuple[str, ...]
    entries: tuple[VscodeRegistrationEntry, ...]
    registered_files: tuple[str, ...]
    unregistered_files: tuple[str, ...]
    missing_targets: tuple[str, ...]
    duplicate_targets: tuple[str, ...]
    fact_count: int
    unregistered_fact_files: tuple[str, ...]
    diagnostics: tuple[ScanDiagnostic, ...]

    @property
    def is_clean(self) -> bool:
        return not any(item.severity == "error" for item in self.diagnostics)


def audit_vscode_test_registration(root: Path) -> VscodeRegistrationAudit:
    root = root.resolve()
    try:
        scan = scan_vscode_tests(root)
        fact_count = len(scan.facts)
        fact_files = tuple(sorted({fact.path for fact in scan.facts}))
        scan_diagnostics = [item for item in scan.diagnostics if item.severity == "error"]
    except (OSError, UnicodeError, ValueError) as exc:
        fact_count = 0
        fact_files = ()
        scan_diagnostics = [
            diagnostic(
                "vscode_fact_scan",
                "editors/vscode/src/test",
                1,
                f"VS Code fact scan failed closed: {exc}",
                severity="error",
            )
        ]
    suite = root / SUITE_PATH
    index = root / INDEX_PATH
    test_files = tuple(
        path.relative_to(root).as_posix()
        for path in source_files(root, SUITE_PATH, (".ts",))
        if path.name.endswith(".test.ts")
    )
    diagnostics = scan_diagnostics
    entries: list[VscodeRegistrationEntry] = []

    if not index.is_file():
        diagnostics.append(diagnostic("missing_registration_index", INDEX_PATH, 1, "index.ts is missing", severity="error"))
        return _build_audit(test_files, entries, diagnostics, fact_count, fact_files)
    try:
        lines = index.read_text().splitlines()
    except (OSError, UnicodeError) as exc:
        diagnostics.append(diagnostic("registration_index_read", INDEX_PATH, 1, str(exc), severity="error"))
        return _build_audit(test_files, entries, diagnostics, fact_count, fact_files)

    code_lines: list[tuple[int, str]] = []
    state: str | None = None
    for line_number, line in enumerate(lines, start=1):
        code, state = strip_js_comments(line, state)
        code_lines.append((line_number, code))
    pre_lines = [line for line, code in code_lines if PRE_REQUIRE_TOKEN in code]
    run_lines = [line for line, code in code_lines if RUN_TOKEN in code]
    if len(pre_lines) != 1 or len(run_lines) != 1 or (pre_lines and run_lines and pre_lines[0] >= run_lines[0]):
        diagnostics.append(
            diagnostic(
                "registration_boundaries",
                INDEX_PATH,
                1,
                "index.ts must have one pre-require boundary before one run boundary",
                severity="error",
            )
        )
        return _build_audit(test_files, entries, diagnostics, fact_count, fact_files)

    for line_number, code in code_lines:
        if not pre_lines[0] < line_number < run_lines[0]:
            continue
        literal = LITERAL_REQUIRE_RE.fullmatch(code)
        if literal:
            specifier = literal.group("path")
            if not _safe_specifier(specifier):
                diagnostics.append(
                    diagnostic(
                        "unsafe_registration_path",
                        INDEX_PATH,
                        line_number,
                        f"unsupported test registration path: {specifier}",
                        severity="error",
                    )
                )
                continue
            target = suite / (specifier[2:] + ".ts")
            resolved_path = target.relative_to(root).as_posix()
            entries.append(VscodeRegistrationEntry(specifier, line_number, resolved_path))
            continue
        if "require(" in mask_js_literals(code):
            diagnostics.append(
                diagnostic(
                    "unsupported_registration",
                    INDEX_PATH,
                    line_number,
                    "test registration must be a direct same-line literal require",
                    severity="error",
                )
            )

    return _build_audit(
        test_files,
        entries,
        diagnostics,
        fact_count,
        fact_files,
        root=root,
    )


def _safe_specifier(specifier: str) -> bool:
    path = PurePosixPath(specifier)
    return (
        SAFE_SPECIFIER_RE.fullmatch(specifier) is not None
        and "\\" not in specifier
        and ".." not in path.parts
        and not specifier.endswith((".ts", ".js"))
        and "?" not in specifier
        and "#" not in specifier
    )


def _build_audit(
    test_files: tuple[str, ...],
    entries: list[VscodeRegistrationEntry],
    diagnostics: list[ScanDiagnostic],
    fact_count: int,
    fact_files: tuple[str, ...],
    *,
    root: Path | None = None,
) -> VscodeRegistrationAudit:
    counts = Counter(entry.resolved_path for entry in entries)
    duplicate_targets = tuple(sorted(path for path, count in counts.items() if count > 1))
    for path in duplicate_targets:
        diagnostics.append(
            diagnostic("duplicate_registration", INDEX_PATH, 1, f"test file is registered more than once: {path}", severity="error")
        )

    existing: set[str] = set()
    missing_targets: list[str] = []
    for entry in entries:
        if root is not None and _is_contained_file(root, entry.resolved_path):
            existing.add(entry.resolved_path)
        else:
            missing_targets.append(entry.resolved_path)
    for path in sorted(set(missing_targets)):
        diagnostics.append(
            diagnostic("missing_registration_target", INDEX_PATH, 1, f"registered test file does not exist: {path}", severity="error")
        )
    unregistered = tuple(sorted(set(test_files) - existing))
    for path in unregistered:
        diagnostics.append(
            diagnostic("unregistered_test_file", path, 1, "VS Code test file is not registered in suite/index.ts", severity="error")
        )
    unregistered_fact_files = tuple(sorted(set(fact_files) - existing))
    for path in unregistered_fact_files:
        diagnostics.append(
            diagnostic(
                "unregistered_vscode_fact_file",
                path,
                1,
                "scanner found VS Code test facts in a file not registered in suite/index.ts",
                severity="error",
            )
        )
    diagnostics.sort(key=lambda item: (item.path, item.line, item.kind, item.message))
    return VscodeRegistrationAudit(
        index_path=INDEX_PATH,
        test_files=test_files,
        entries=tuple(entries),
        registered_files=tuple(sorted(existing)),
        unregistered_files=unregistered,
        missing_targets=tuple(sorted(set(missing_targets))),
        duplicate_targets=duplicate_targets,
        fact_count=fact_count,
        unregistered_fact_files=unregistered_fact_files,
        diagnostics=tuple(diagnostics),
    )


def _is_contained_file(root: Path, relative: str) -> bool:
    suite = (root / SUITE_PATH).resolve()
    candidate = root / relative
    try:
        candidate.resolve().relative_to(suite)
    except (OSError, ValueError):
        return False
    return candidate.is_file()
