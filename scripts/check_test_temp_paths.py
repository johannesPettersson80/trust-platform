#!/usr/bin/env python3
"""Each test's temporary path is its own (specification 22, "Native test and docs-capture lifecycle portability").

Tests run at once in one process and in several, so a temporary path that a clock stamp alone makes unique can be one
path for two tests, and the first to finish removes it under the other. Test code therefore takes its temporary paths
from `trust_test_support::unique_temp_path`, which puts the process id and a counter in the name. This check fails on a
`temp_dir()` call in test code anywhere else:

- test code is a file in a `tests` directory, a `tests.rs`, a `*_tests.rs` or a `*_test.rs`, and the items of any
  other file that `#[cfg(test)]` or `#[cfg(all(test, ...))]` gates;
- a call that names a product's own path rather than a test's carries the comment `// product temp path` on its line
  or on the line before.

Usage: check_test_temp_paths.py [ROOT]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

MARKER = "// product temp path"
HELPER_CRATE = "crates/trust-test-support/"
SCANNED = ("crates", "xtask")
# Crates whose tests still take temporary paths of their own: checklist G72 converts them part by part, and
# this list is empty when every test takes its path from trust-test-support.
NOT_YET = ("crates/trust-lsp/", "crates/trust-control-studio/", "crates/trust-runtime/")
TEMP_DIR_CALL = re.compile(r"\benv::temp_dir\(\)")
TEST_GATE = re.compile(r"#\[cfg\((?:test|all\(\s*test\b[^\]]*)\)\]")


def code_only(text: str) -> str:
    """`text` with the insides of comments, strings and character literals blanked, its lines kept, so that
    braces and calls are read in code alone."""
    out = list(text)
    i, n = 0, len(text)

    def blank(start: int, end: int) -> None:
        for k in range(start, min(end, n)):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = text[i]
        if text.startswith("//", i):
            end = text.find("\n", i)
            end = n if end < 0 else end
            blank(i, end)
            i = end
        elif text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(i, j)
            i = j
        elif c == "r" and re.match(r'r#*"', text[i:i + 300]) and not (i and (text[i - 1].isalnum() or text[i - 1] == "_")):
            hashes = len(re.match(r"r(#*)\"", text[i:]).group(1))
            close = '"' + "#" * hashes
            j = text.find(close, i + 2 + hashes)
            j = n if j < 0 else j + len(close)
            blank(i + 1, j)
            i = j
        elif c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            blank(i + 1, j)
            i = j + 1
        elif c == "'":
            if text.startswith("\\", i + 1):
                j = text.find("'", i + 2)
                j = n if j < 0 else j
                blank(i + 1, j)
                i = j + 1
            elif i + 2 < n and text[i + 2] == "'":
                blank(i + 1, i + 2)
                i += 3
            else:
                i += 1  # a lifetime
        else:
            i += 1
    return "".join(out)


def is_test_file(relative: str) -> bool:
    name = relative.rsplit("/", 1)[-1]
    return "/tests/" in f"/{relative}" or name == "tests.rs" or name.endswith(("_tests.rs", "_test.rs"))


def test_lines(code: str) -> set[int]:
    """The line numbers (from 1) of the items that a test gate gates in `code`, which `code_only` produced."""
    lines: set[int] = set()
    for gate in TEST_GATE.finditer(code):
        start = gate.start()
        j = gate.end()
        brace = code.find("{", j)
        semicolon = code.find(";", j)
        if brace < 0 or (0 <= semicolon < brace):
            end = semicolon if semicolon >= 0 else len(code)
        else:
            depth, end = 0, brace
            while end < len(code):
                if code[end] == "{":
                    depth += 1
                elif code[end] == "}":
                    depth -= 1
                    if depth == 0:
                        break
                end += 1
        first = code.count("\n", 0, start) + 1
        last = code.count("\n", 0, end) + 1
        lines.update(range(first, last + 1))
    return lines


def violations(relative: str, text: str) -> list[tuple[int, str]]:
    """The test-code `temp_dir()` calls of the file `relative` without the marker: (line, line text)."""
    if relative.startswith((HELPER_CRATE, *NOT_YET)):
        return []
    code = code_only(text)
    whole = is_test_file(relative)
    gated = set() if whole else test_lines(code)
    source = text.splitlines()
    found = []
    for number, line in enumerate(code.splitlines(), start=1):
        if not TEMP_DIR_CALL.search(line) or not (whole or number in gated):
            continue
        here = source[number - 1]
        before = source[number - 2] if number > 1 else ""
        if MARKER in here or MARKER in before:
            continue
        found.append((number, here.strip()))
    return found


def main(argv: list[str]) -> int:
    root = Path(argv[1]) if len(argv) > 1 else Path(__file__).resolve().parent.parent
    failures = []
    for top in SCANNED:
        for path in sorted((root / top).rglob("*.rs")):
            relative = path.relative_to(root).as_posix()
            if "/target/" in f"/{relative}":
                continue
            for number, line in violations(relative, path.read_text(encoding="utf-8", errors="replace")):
                failures.append(f"{relative}:{number}: {line}")
    if failures:
        print("Test code takes its temporary paths from trust_test_support::unique_temp_path "
              "(specification 22); mark a product's own path with `// product temp path`:")
        print("\n".join(failures))
        return 1
    print("every test's temporary path is its own")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
