#!/usr/bin/env python3
"""Each test's temporary path is its own (specification 22, "Native test and docs-capture lifecycle portability").

Tests run at once in one process and in several, so a temporary path that a clock stamp alone makes unique can be one
path for two tests, and the first to finish removes it under the other. Test code therefore takes its temporary paths
from `trust_test_support::unique_temp_path`, which puts the process id and a counter in the name. This check fails on a
`temp_dir()` call in test code anywhere else:

- test code is a file in a crate's own `tests` or `benches` directory, a module file that a `#[cfg(test)]` or
  `#[cfg(all(test, ...))]` gated `mod name;` declares (with its `#[path]`) and every module file below it, and the
  items of any other file that such a gate gates. A file's name proves nothing: `hmi_test.rs` is a command's module;
- a call that must take the temporary directory itself carries the comment `// temp-root-ok: <why>` on its line or on
  the line before: a product's own path, or a socket path that the 100-byte socket budget keeps short.

Usage: check_test_temp_paths.py [ROOT]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

MARKER = "temp-root-ok:"
HELPER_CRATE = "crates/trust-test-support/"
SCANNED = ("crates", "xtask")
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


MOD_DECLARATION = re.compile(r"((?:#\[[^\]]*\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?mod\s+(r#)?(\w+)\s*;")
PATH_ATTRIBUTE = re.compile(r'#\[path\s*=\s*"([^"]+)"\]')


TEST_DIRECTORY = re.compile(r"^(?:crates/[^/]+|xtask)/(?:tests|benches)/")


def in_test_directory(relative: str) -> bool:
    """Whether `relative` is in a crate's own `tests` or `benches` directory, whose files are the crate's integration
    tests and benchmarks; a module directory named `tests` inside `src` is decided by the module tree."""
    return bool(TEST_DIRECTORY.match(relative))


def declared_modules(relative: str, text: str, code: str) -> list[tuple[str, bool]]:
    """The module files that `relative` declares with `mod name;`: (the file, whether a test gate gates the
    declaration). Each name is `dir/name.rs` or `dir/name/mod.rs`, both given; `dir` is the file's own directory for
    a `mod.rs`, a `lib.rs` or a `main.rs`, and the directory named after the file otherwise; `#[path]` is relative to
    the file's directory."""
    directory = relative.rsplit("/", 1)[0] if "/" in relative else ""
    stem = relative.rsplit("/", 1)[-1][:-3]
    base = directory if stem in ("mod", "lib", "main") else f"{directory}/{stem}"
    gated = test_lines(code)
    found = []
    for declaration in MOD_DECLARATION.finditer(code):
        attributes, name = declaration.group(1), declaration.group(3)
        line = code.count("\n", 0, declaration.start()) + 1
        is_gated = bool(TEST_GATE.search(attributes)) or line in gated
        path = PATH_ATTRIBUTE.search(text[declaration.start():declaration.end()])
        if path:
            found.append((f"{directory}/{path.group(1)}".lstrip("/"), is_gated))
        else:
            found.append((f"{base}/{name}.rs", is_gated))
            found.append((f"{base}/{name}/mod.rs", is_gated))
    return found


def test_module_files(sources: dict[str, str]) -> set[str]:
    """The files of `sources` (relative path -> text) that are test code as a whole: under a `tests` or `benches`
    directory, or declared through a test gate, and every module file below such a file."""
    tests = {relative for relative in sources if in_test_directory(relative)}
    children: dict[str, list[tuple[str, bool]]] = {}
    for relative, text in sources.items():
        children[relative] = declared_modules(relative, text, code_only(text))
    pending = list(tests)
    for relative, declared in children.items():
        for child, is_gated in declared:
            if is_gated and child in sources and child not in tests:
                tests.add(child)
                pending.append(child)
    while pending:
        parent = pending.pop()
        for child, _ in children.get(parent, []):
            if child in sources and child not in tests:
                tests.add(child)
                pending.append(child)
    return tests


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


def violations(relative: str, text: str, test_files: set[str]) -> list[tuple[int, str]]:
    """The test-code `temp_dir()` calls of the file `relative` without the marker: (line, line text). `test_files`
    are the files that are test code as a whole (`test_module_files`)."""
    if relative.startswith(HELPER_CRATE):
        return []
    code = code_only(text)
    whole = relative in test_files
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
    sources = {}
    for top in SCANNED:
        for path in sorted((root / top).rglob("*.rs")):
            relative = path.relative_to(root).as_posix()
            if "/target/" not in f"/{relative}":
                sources[relative] = path.read_text(encoding="utf-8", errors="replace")
    test_files = test_module_files(sources)
    failures = []
    for relative, text in sources.items():
        for number, line in violations(relative, text, test_files):
            failures.append(f"{relative}:{number}: {line}")
    if failures:
        print("Test code takes its temporary paths from trust_test_support::unique_temp_path "
              "(specification 22), or say why a call takes the temporary directory itself with `// temp-root-ok: <why>`:")
        print("\n".join(failures))
        return 1
    print("every test's temporary path is its own")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
