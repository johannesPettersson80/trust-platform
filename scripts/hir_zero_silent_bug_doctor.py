#!/usr/bin/env python3
"""Warn-only HIR zero-silent-bug migration doctor.

The Phase 3 migration uses this as a visibility gate. It intentionally reports
known legacy surfaces without failing by default; Phase 6 flips selected rules
to fail once the semantic-kernel allowlist exists.
"""

from __future__ import annotations

import argparse
import re
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
HIR_SRC = REPO_ROOT / "crates" / "trust-hir" / "src"
RUNTIME_SRC = REPO_ROOT / "crates" / "trust-runtime" / "src"
ALLOWLIST = (
    REPO_ROOT / "docs" / "internal" / "architecture" / "hir-semantic-kernel-allowlist.toml"
)


@dataclass(frozen=True)
class Finding:
    rule: str
    path: Path
    line: int
    text: str


RULES: tuple[tuple[str, re.Pattern[str]], ...] = (
    (
        "HIRZSB-WARN-BROAD-LOOKUP",
        re.compile(r"\.(lookup_any|resolve_by_name|lookup_type)\s*\("),
    ),
    (
        "HIRZSB-WARN-TYPED-SENTINEL",
        re.compile(
            r"\breturn\s+(TypeId|SymbolId)::UNKNOWN\b|=>\s*(TypeId|SymbolId)::UNKNOWN\b|unwrap_or\(\s*(TypeId|SymbolId)::UNKNOWN\s*\)"
        ),
    ),
    (
        "HIRZSB-WARN-SILENT-DISCARD",
        re.compile(r"let\s+_\s*=\s*.*\.(define_in_scope|insert)\s*\("),
    ),
    (
        "HIRZSB-WARN-TYPE-UNKNOWN-FALLBACK",
        re.compile(r"unwrap_or\(\s*&?Type::Unknown\s*\)"),
    ),
)


LOSSY_RESULT_OPTION = re.compile(
    r"(?<!::)\b(?:[A-Za-z_][A-Za-z0-9_]*\.)?try_[A-Za-z0-9_]+\s*\([^;{}]*?\)\s*(?:\n\s*)?\.ok\(\)",
    re.MULTILINE,
)
RESOLVER_FN = re.compile(r"\bfn\s+(resolve_[A-Za-z0-9_]+|.*_resolve[A-Za-z0-9_]*)\s*\(")
ALLOWED_DUPLICATED_RESOLVERS: dict[str, set[str]] = {
    "resolve_access_path_target": {
        "crates/trust-hir/src/db/queries/helpers.rs",
        "crates/trust-hir/src/db/queries/collector/validation.rs",
    },
    "resolve_alias_type": {
        "crates/trust-hir/src/symbols/table.rs",
        "crates/trust-hir/src/type_check/compatibility.rs",
    },
    "resolve_alias_type_outcome": {
        "crates/trust-hir/src/symbols/table.rs",
        "crates/trust-hir/src/type_check/compatibility.rs",
    },
    "resolve_member_symbol_in_hierarchy": {
        "crates/trust-hir/src/symbols/table.rs",
        "crates/trust-hir/src/type_check/calls/resolve.rs",
    },
    "resolve_member_symbol_in_type": {
        "crates/trust-hir/src/symbols/table.rs",
        "crates/trust-hir/src/type_check/calls/resolve.rs",
    },
    "resolve_name": {
        "crates/trust-hir/src/db/queries.rs",
        "crates/trust-hir/src/db/queries/database/database_part_02.rs",
    },
    "resolve_using_in_scope": {
        "crates/trust-hir/src/symbols/table.rs",
        "crates/trust-hir/src/type_check/calls/resolve.rs",
    },
}
RUNTIME_DECL_BYPASS: tuple[tuple[Path, re.Pattern[str], str], ...] = (
    (
        RUNTIME_SRC / "harness" / "compiler" / "pou" / "entry_points.rs",
        re.compile(
            r"\.descendants\(\)\s*\.filter\(\|child\|\s*child\.kind\(\)\s*==\s*SyntaxKind::(Program|Function|FunctionBlock|Class|Interface)\s*\)",
            re.MULTILINE,
        ),
        "runtime POU lowering must be driven by HIR declaration catalog entries",
    ),
    (
        RUNTIME_SRC / "harness" / "compiler" / "types.rs",
        re.compile(
            r"predeclare_(function_blocks|classes|interfaces)[\s\S]*?\.descendants\(\)\s*\.filter\(\|child\|\s*child\.kind\(\)\s*==\s*SyntaxKind::(FunctionBlock|Class|Interface)\s*\)",
            re.MULTILINE,
        ),
        "runtime POU predeclaration must be driven by HIR declaration catalog entries",
    ),
)
PUBLIC_RAW_API = (
    HIR_SRC / "symbols" / "table.rs",
    re.compile(
        r"pub\s+fn\s+(lookup_any|set_extends|set_implements|extends_name|implements_names)\s*\("
    ),
)


def iter_rust_files(root: Path) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*.rs")
        if path.is_file() and "_tests" not in path.name and "tests" not in path.parts
    )


def scan_file(path: Path) -> list[Finding]:
    findings: list[Finding] = []
    rel = path.relative_to(REPO_ROOT)
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.strip()
        if not stripped or stripped.startswith("//"):
            continue
        for rule, pattern in RULES:
            if pattern.search(line):
                findings.append(Finding(rule, rel, line_no, stripped))
    return findings


def scan_duplicated_resolvers() -> list[Finding]:
    resolver_locations: dict[str, list[Finding]] = {}
    for path in iter_rust_files(HIR_SRC):
        rel = path.relative_to(REPO_ROOT)
        for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            match = RESOLVER_FN.search(line)
            if match is None:
                continue
            resolver_locations.setdefault(match.group(1), []).append(
                Finding(
                    "HIRZSB-WARN-DUPLICATED-RESOLVER",
                    rel,
                    line_no,
                    line.strip(),
                )
            )

    return duplicated_resolver_findings(resolver_locations)


def duplicated_resolver_findings(
    resolver_locations: dict[str, list[Finding]],
) -> list[Finding]:
    findings: list[Finding] = []
    for name, locations in resolver_locations.items():
        if len(locations) <= 1:
            continue
        allowed_paths = ALLOWED_DUPLICATED_RESOLVERS.get(name)
        actual_paths = {location.path.as_posix() for location in locations}
        if allowed_paths is not None and actual_paths <= allowed_paths:
            continue
        findings.extend(locations)
    return findings


def allowlist_findings_from_lines(lines: list[str], rel_path: Path) -> list[Finding]:
    max_entries = 5
    entries = 0
    current_entry: dict[str, tuple[int, str]] = {}
    entry_start = 0
    granularity_findings: list[Finding] = []

    def finish_entry() -> None:
        if not current_entry:
            return
        function = current_entry.get("function")
        file_range = current_entry.get("file_range")
        if function is None and file_range is None:
            granularity_findings.append(
                Finding(
                    "HIRZSB-ALLOWLIST-GRANULARITY",
                    rel_path,
                    entry_start,
                    "allowlist entry must name one function or one file_range",
                )
            )
            return
        if function is not None and "::" not in function[1]:
            granularity_findings.append(
                Finding(
                    "HIRZSB-ALLOWLIST-GRANULARITY",
                    rel_path,
                    function[0],
                    "function allowlist entry must name one fully qualified function",
                )
            )
        if file_range is not None and not re.search(r"\.rs:\d+", file_range[1]):
            granularity_findings.append(
                Finding(
                    "HIRZSB-ALLOWLIST-GRANULARITY",
                    rel_path,
                    file_range[0],
                    "file_range allowlist entry must include a Rust file and line",
                )
            )

    for line_no, line in enumerate(lines, 1):
        stripped = line.strip()
        if stripped.startswith("max_entries"):
            _, value = stripped.split("=", 1)
            max_entries = int(value.strip())
        if stripped == "[[entries]]":
            finish_entry()
            current_entry = {}
            entry_start = line_no
            entries += 1
            continue
        if "=" in stripped and current_entry is not None:
            key, value = stripped.split("=", 1)
            key = key.strip()
            value = value.strip().strip('"')
            if key in {"function", "file_range"}:
                current_entry[key] = (line_no, value)
    finish_entry()

    if entries <= max_entries:
        return granularity_findings
    return [
        Finding(
            "HIRZSB-ALLOWLIST-LIMIT",
            rel_path,
            1,
            f"semantic kernel allowlist has {entries} entries; max is {max_entries}",
        )
    ] + granularity_findings


def scan_lossy_result_option() -> list[Finding]:
    findings: list[Finding] = []
    for path in iter_rust_files(HIR_SRC):
        source = path.read_text(encoding="utf-8")
        rel = path.relative_to(REPO_ROOT)
        for match in LOSSY_RESULT_OPTION.finditer(source):
            line_no = source.count("\n", 0, match.start()) + 1
            text = " ".join(match.group(0).split())
            findings.append(
                Finding(
                    "HIRZSB-WARN-LOSSY-RESULT-OPTION",
                    rel,
                    line_no,
                    text,
                )
            )
    return findings


def scan_allowlist() -> list[Finding]:
    if not ALLOWLIST.exists():
        return [
            Finding(
                "HIRZSB-ALLOWLIST-MISSING",
                ALLOWLIST.relative_to(REPO_ROOT),
                1,
                "semantic kernel allowlist file is missing",
            )
        ]

    lines = ALLOWLIST.read_text(encoding="utf-8").splitlines()
    return allowlist_findings_from_lines(lines, ALLOWLIST.relative_to(REPO_ROOT))


def scan_runtime_declaration_bypasses() -> list[Finding]:
    findings: list[Finding] = []
    for path, pattern, message in RUNTIME_DECL_BYPASS:
        if not path.exists():
            continue
        source = path.read_text(encoding="utf-8")
        if not pattern.search(source):
            continue
        findings.append(
            Finding(
                "HIRZSB-FAIL-RUNTIME-DECL-BYPASS",
                path.relative_to(REPO_ROOT),
                1,
                message,
            )
        )
    return findings


def scan_public_raw_apis() -> list[Finding]:
    path, pattern = PUBLIC_RAW_API
    if not path.exists():
        return []
    findings: list[Finding] = []
    rel = path.relative_to(REPO_ROOT)
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if pattern.search(line):
            findings.append(
                Finding(
                    "HIRZSB-FAIL-PUBLIC-RAW-API",
                    rel,
                    line_no,
                    line.strip(),
                )
            )
    return findings


def run_self_test() -> int:
    errors: list[str] = []
    fixtures: tuple[tuple[str, re.Pattern[str], str], ...] = (
        (
            "HIRZSB-WARN-BROAD-LOOKUP",
            RULES[0][1],
            "symbols.lookup_any(name);",
        ),
        (
            "HIRZSB-WARN-TYPED-SENTINEL",
            RULES[1][1],
            "return TypeId::UNKNOWN;",
        ),
        (
            "HIRZSB-WARN-SILENT-DISCARD",
            RULES[2][1],
            "let _ = table.define_in_scope(scope, name, id);",
        ),
        (
            "HIRZSB-WARN-TYPE-UNKNOWN-FALLBACK",
            RULES[3][1],
            "symbols.type_by_id(id).unwrap_or(&Type::Unknown);",
        ),
        (
            "HIRZSB-WARN-LOSSY-RESULT-OPTION",
            LOSSY_RESULT_OPTION,
            "self.try_eval_const_int_expr(node)\n    .ok()",
        ),
        (
            "HIRZSB-FAIL-PUBLIC-RAW-API",
            PUBLIC_RAW_API[1],
            "pub fn lookup_any(&self, name: &str) -> Option<SymbolId> { None }",
        ),
    )
    for rule, pattern, source in fixtures:
        if pattern.search(source) is None:
            errors.append(f"{rule} did not match known-bad fixture")

    duplicate_findings = duplicated_resolver_findings(
        {
            "resolve_thing": [
                Finding("HIRZSB-WARN-DUPLICATED-RESOLVER", Path("a.rs"), 1, ""),
                Finding("HIRZSB-WARN-DUPLICATED-RESOLVER", Path("b.rs"), 1, ""),
            ]
        }
    )
    if not any(f.rule == "HIRZSB-WARN-DUPLICATED-RESOLVER" for f in duplicate_findings):
        errors.append("HIRZSB-WARN-DUPLICATED-RESOLVER did not catch duplicate fixture")

    allowlist_lines = [
        "max_entries = 1",
        "[[entries]]",
        'function = "crate::first"',
        "[[entries]]",
        'function = "too_broad"',
    ]
    allowlist_findings = allowlist_findings_from_lines(
        allowlist_lines,
        Path("docs/internal/architecture/hir-semantic-kernel-allowlist.toml"),
    )
    if not any(f.rule == "HIRZSB-ALLOWLIST-LIMIT" for f in allowlist_findings):
        errors.append("HIRZSB-ALLOWLIST-LIMIT did not catch over-limit fixture")
    if not any(f.rule == "HIRZSB-ALLOWLIST-GRANULARITY" for f in allowlist_findings):
        errors.append("HIRZSB-ALLOWLIST-GRANULARITY did not catch broad fixture")

    runtime_sources = (
        "root.descendants().filter(|child| child.kind() == SyntaxKind::Program)",
        "fn predeclare_function_blocks() { root.descendants().filter(|child| child.kind() == SyntaxKind::FunctionBlock); }",
    )
    for (_, pattern, message), source in zip(RUNTIME_DECL_BYPASS, runtime_sources):
        if pattern.search(source) is None:
            errors.append(f"HIRZSB-FAIL-RUNTIME-DECL-BYPASS did not catch {message}")

    if errors:
        print("HIR zero-silent-bug doctor self-test: failed")
        for error in errors:
            print(f"- {error}")
        return 1

    print("HIR zero-silent-bug doctor self-test: ok")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--fail",
        action="store_true",
        help="return non-zero when findings are present; Phase 6 will use this",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run known-bad doctor rule fixtures and exit",
    )
    args = parser.parse_args()

    if args.self_test:
        return run_self_test()

    findings: list[Finding] = []
    for path in iter_rust_files(HIR_SRC):
        findings.extend(scan_file(path))
    findings.extend(scan_lossy_result_option())
    findings.extend(scan_duplicated_resolvers())
    findings.extend(scan_allowlist())
    findings.extend(scan_runtime_declaration_bypasses())
    findings.extend(scan_public_raw_apis())

    if not findings:
        print("HIR zero-silent-bug doctor: no findings")
        return 0

    print(f"HIR zero-silent-bug doctor: {len(findings)} warn-only finding(s)")
    for finding in findings:
        print(f"{finding.rule} {finding.path}:{finding.line}: {finding.text}")

    has_fail_finding = any(finding.rule.startswith("HIRZSB-FAIL-") for finding in findings)
    return 1 if args.fail or has_fail_finding else 0


if __name__ == "__main__":
    raise SystemExit(main())
