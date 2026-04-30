#!/usr/bin/env python3
"""Require focused test evidence for bug-fix changes."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
ZERO_SHA = "0" * 40

BUGFIX_PATTERNS = [
    re.compile(pattern, re.IGNORECASE)
    for pattern in [
        r"\bfix(?:e[sd])?\b",
        r"\bbug(?:s)?\b",
        r"\bregression(?!-test-first)(?:s)?\b",
        r"\bcrash(?:e[sd]|ing)?\b",
        r"\bpanic(?:s|ked|king)?\b",
        r"\bhang(?:s|ing)?\b",
        r"\bstall(?:s|ed|ing)?\b",
        r"\btimeout(?:s|ed)?\b",
        r"\bsilent (?:bug|error|failure)s?\b",
        r"\bwrong\b",
        r"\bincorrect\b",
        r"\bundefined behavior\b",
        r"\b(?:close[sd]?|fixe[sd]?|resolve[sd]?)\s+#\d+\b",
    ]
]

EVIDENCE_PATTERN = re.compile(r"^Regression-test-first:\s*(?P<value>.+)$", re.IGNORECASE | re.MULTILINE)

BEHAVIOR_PREFIXES = (
    ".github/workflows/",
    "crates/trust-debug/",
    "crates/trust-hir/",
    "crates/trust-ide/",
    "crates/trust-lsp/",
    "crates/trust-runtime/",
    "crates/trust-syntax/",
    "crates/trust-wasm-analysis/",
    "editors/vscode/",
    "scripts/",
    "xtask/",
)

BEHAVIOR_FILES = {
    "Cargo.toml",
    "Cargo.lock",
    "justfile",
}

DOC_SUFFIXES = (".md", ".adoc", ".txt", ".puml", ".svg", ".png", ".jpg", ".jpeg")
BEHAVIOR_SUFFIXES = (
    ".rs",
    ".st",
    ".toml",
    ".json",
    ".yaml",
    ".yml",
    ".ts",
    ".tsx",
    ".js",
    ".mjs",
    ".cjs",
    ".py",
    ".sh",
    ".ps1",
)

INLINE_TEST_ADDITION = re.compile(
    r"^\+.*("
    r"#\[test\]|#\[tokio::test\]|#\[cfg\(test\)\]|mod tests\b|"
    r"\btest\(|\bit\(|\bdescribe\("
    r")",
    re.IGNORECASE,
)


def run_git(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=REPO_ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=check,
    )


def ref_exists(ref: str | None) -> bool:
    if not ref or ref == ZERO_SHA:
        return False
    result = run_git(["cat-file", "-e", f"{ref}^{{commit}}"], check=False)
    return result.returncode == 0


def resolve_range(base: str | None, head: str | None) -> tuple[str, str]:
    resolved_head = head if ref_exists(head) else "HEAD"
    if ref_exists(base):
        merge_base = run_git(["merge-base", base or "", resolved_head], check=False)
        if merge_base.returncode == 0 and merge_base.stdout.strip():
            return merge_base.stdout.strip(), resolved_head
        return base or "HEAD^", resolved_head
    parent = run_git(["rev-parse", f"{resolved_head}^"], check=False)
    if parent.returncode == 0:
        return parent.stdout.strip(), resolved_head
    return resolved_head, resolved_head


def changed_files(base: str, head: str) -> list[str]:
    result = run_git(["diff", "--name-only", "--diff-filter=ACMRT", f"{base}..{head}"])
    return [line.strip() for line in result.stdout.splitlines() if line.strip()]


def commit_messages(base: str, head: str) -> str:
    result = run_git(["log", "--format=%B%n---commit---", f"{base}..{head}"], check=False)
    return result.stdout if result.returncode == 0 else ""


def event_text() -> str:
    event_path = os.environ.get("GITHUB_EVENT_PATH")
    if not event_path:
        return ""
    path = Path(event_path)
    if not path.is_file():
        return ""
    try:
        event = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return ""
    pull_request = event.get("pull_request") or {}
    parts = [
        str(pull_request.get("title") or ""),
        str(pull_request.get("body") or ""),
    ]
    return "\n".join(part for part in parts if part)


def has_bugfix_signal(text: str) -> bool:
    return any(pattern.search(text) for pattern in BUGFIX_PATTERNS)


def evidence_marker(text: str) -> str | None:
    match = EVIDENCE_PATTERN.search(text)
    if not match:
        return None
    value = match.group("value").strip()
    if value.lower() in {"", "n/a", "none", "not applicable"}:
        return None
    return value


def is_behavior_file(path: str) -> bool:
    if path in BEHAVIOR_FILES:
        return True
    if path.endswith(DOC_SUFFIXES):
        return False
    if not path.endswith(BEHAVIOR_SUFFIXES):
        return False
    return any(path.startswith(prefix) for prefix in BEHAVIOR_PREFIXES)


def is_test_file(path: str) -> bool:
    normalized = path.replace("\\", "/")
    parts = normalized.lower().split("/")
    name = parts[-1]
    if "tests" in parts or "test" in parts:
        return True
    if normalized.startswith("editors/vscode/src/test/"):
        return True
    return (
        name.startswith("test_")
        or name.endswith("_test.rs")
        or name.endswith(".test.ts")
        or name.endswith(".test.tsx")
        or name.endswith(".spec.ts")
        or name.endswith(".spec.tsx")
        or name.endswith(".test.js")
        or name.endswith(".spec.js")
    )


def has_inline_test_addition(base: str, head: str, files: list[str]) -> bool:
    candidate_files = [path for path in files if is_behavior_file(path)]
    if not candidate_files:
        return False
    result = run_git(["diff", "--unified=0", f"{base}..{head}", "--", *candidate_files], check=False)
    if result.returncode != 0:
        return False
    return any(INLINE_TEST_ADDITION.search(line) for line in result.stdout.splitlines())


def findings_for_inputs(
    *,
    changed: list[str],
    text: str,
    inline_test_added: bool,
) -> list[str]:
    if not has_bugfix_signal(text):
        return []

    behavior_files = [path for path in changed if is_behavior_file(path)]
    if not behavior_files:
        return []

    test_files = [path for path in changed if is_test_file(path)]
    marker = evidence_marker(text)
    if test_files or marker or inline_test_added:
        return []

    preview = ", ".join(behavior_files[:8])
    if len(behavior_files) > 8:
        preview += f", ... ({len(behavior_files)} files)"
    return [
        "bug-fix signal found, but no focused regression-test evidence was found",
        f"behavior files changed: {preview}",
        "add/change a focused test file, add an inline test, or include "
        "`Regression-test-first: <focused failing test command or existing failing test>` "
        "in the PR body or commit message",
    ]


def check_range(base: str | None, head: str | None) -> int:
    resolved_base, resolved_head = resolve_range(base, head)
    changed = changed_files(resolved_base, resolved_head)
    text = "\n".join(
        part
        for part in [
            commit_messages(resolved_base, resolved_head),
            event_text(),
        ]
        if part
    )
    inline_test_added = has_inline_test_addition(resolved_base, resolved_head, changed)
    findings = findings_for_inputs(changed=changed, text=text, inline_test_added=inline_test_added)
    if findings:
        print("regression-test-first check: failed")
        print(f"range: {resolved_base}..{resolved_head}")
        for finding in findings:
            print(f"- {finding}")
        return 1
    if has_bugfix_signal(text):
        print("regression-test-first check: ok (bug-fix signal has test evidence or no behavior change)")
    else:
        print("regression-test-first check: ok (no bug-fix signal)")
    return 0


def run_self_test() -> int:
    cases = [
        {
            "name": "bug fix touching HIR without test fails",
            "changed": ["crates/trust-hir/src/type_check/expr.rs"],
            "text": "fix: report wrong kind",
            "inline": False,
            "want_findings": True,
        },
        {
            "name": "bug fix with new integration test passes",
            "changed": [
                "crates/trust-runtime/src/vm.rs",
                "crates/trust-runtime/tests/vm_regression.rs",
            ],
            "text": "fix: prevent VM hang",
            "inline": False,
            "want_findings": False,
        },
        {
            "name": "bug fix with existing-test marker passes",
            "changed": ["scripts/runtime_mesh_tls_stability_gate.sh"],
            "text": "fix: stop hidden timeout\n\nRegression-test-first: GATE_TEST_TIMEOUT_SECONDS=1 ./scripts/runtime_mesh_tls_stability_gate.sh --iterations 1",
            "inline": False,
            "want_findings": False,
        },
        {
            "name": "bug fix with inline test addition passes",
            "changed": ["crates/trust-syntax/src/parser.rs"],
            "text": "fix: parser recovery regression",
            "inline": True,
            "want_findings": False,
        },
        {
            "name": "non-bug behavior change passes",
            "changed": ["crates/trust-runtime/src/lib.rs"],
            "text": "refactor: split runtime host modules",
            "inline": False,
            "want_findings": False,
        },
    ]
    errors: list[str] = []
    for case in cases:
        findings = findings_for_inputs(
            changed=case["changed"],
            text=case["text"],
            inline_test_added=case["inline"],
        )
        got_findings = bool(findings)
        if got_findings != case["want_findings"]:
            errors.append(f"{case['name']}: expected findings={case['want_findings']} got {findings}")
    if errors:
        print("regression-test-first self-test: failed")
        for error in errors:
            print(f"- {error}")
        return 1
    print("regression-test-first self-test: ok")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base")
    parser.add_argument("--head")
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        return run_self_test()
    return check_range(args.base, args.head)


if __name__ == "__main__":
    raise SystemExit(main())
