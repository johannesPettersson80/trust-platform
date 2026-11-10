#!/usr/bin/env python3
"""A test never reserves a port and releases it (specification 22, "Native test and docs-capture lifecycle portability").

A test that binds port 0, reads the port and drops the listener before a program binds that port again leaves the port
to any socket that asks in between, a client connection's source port among them. This check fails on test code that
does so:

- a `bind` of port 0 read with `.local_addr()` in the same expression: the listener is a temporary, dropped at the end
  of the statement while its port goes on;
- a function that binds port 0 and returns a port (its return type holds a `u16` and no socket): its listener is dropped
  when it returns.

Test code is what `check_test_temp_paths.py` takes for test code. A helper whose callers try again with other ports
when the program's bind found its port taken (`trust_test_support::reports_a_taken_port`), and after no other failure,
says so with the comment `// port-retry-ok: <why>` on the line of the `bind` or of its function, or on the line before
either.

Usage: check_test_ports.py [ROOT]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from check_test_temp_paths import SCANNED, code_only, test_lines, test_module_files  # noqa: E402

MARKER = "port-retry-ok:"
HELPER_CRATE = "crates/trust-test-support/"
BIND_CALL = re.compile(r"\bbind\s*\(")
# Port 0 in the call's own text: `"127.0.0.1:0"`, `("127.0.0.1", 0)` or `SocketAddr::from(([127, 0, 0, 1], 0))`.
PORT_ZERO = re.compile(r':0"|,\s*0\s*\)')
FUNCTION = re.compile(r"\bfn\s+\w+")
SOCKET_TYPE = re.compile(r"Listener|Socket")
PORT_TYPE = re.compile(r"\bu16\b")
# What may stand between the `bind` call and a read of its address in one expression.
CHAIN_STEP = re.compile(r"\s*(?:\?|\.await\b|\.(?:unwrap|expect|context|with_context|map_err|unwrap_or_else)\s*(?=\())")


def closing(code: str, open_index: int) -> int:
    """The index of the bracket that closes the one at `open_index`, in `code` from `code_only`."""
    pairs = {"(": ")", "{": "}", "[": "]"}
    opener, closer = code[open_index], pairs[code[open_index]]
    depth = 0
    for index in range(open_index, len(code)):
        if code[index] == opener:
            depth += 1
        elif code[index] == closer:
            depth -= 1
            if depth == 0:
                return index
    return len(code) - 1


def functions(code: str) -> list[tuple[int, int, int, str]]:
    """Each function item of `code` with a body: (where `fn` starts, where its body opens, where it closes, its
    return type's text, empty when it returns nothing)."""
    found = []
    for item in FUNCTION.finditer(code):
        index = item.end()
        depth = 0
        while index < len(code):
            c = code[index]
            if c in "(<[":
                depth += 1
            elif c in ")>]" and not (c == ">" and code[index - 1] == "-"):
                depth -= 1
            elif c == ";" and depth == 0:
                break  # a declaration without a body, as in a trait
            elif c == "{" and depth == 0:
                signature = code[item.end():index]
                arrow = signature.find("->")
                returns = signature[arrow + 2:].split(" where ")[0] if arrow >= 0 else ""
                found.append((item.start(), index, closing(code, index), returns))
                break
            index += 1
    return found


def reads_address_at_once(code: str, after: int) -> bool:
    """Whether the expression that a `bind` call ends at `after` goes on to `.local_addr()`, past `?`, `.await`
    and the unwrapping calls, so that its listener is a temporary."""
    index = after
    while True:
        step = CHAIN_STEP.match(code, index)
        if not step:
            break
        index = step.end()
        if index < len(code) and code[index] == "(":
            index = closing(code, index) + 1
    return bool(re.match(r"\s*\.local_addr\s*\(", code[index:]))


def violations(relative: str, text: str, test_files: set[str]) -> list[tuple[int, str]]:
    """The binds of port 0 in the test code of the file `relative` whose listener is dropped while its port goes on,
    without the marker: (line, line text). `test_files` are the files that are test code as a whole."""
    if relative.startswith(HELPER_CRATE):
        return []
    code = code_only(text)
    whole = relative in test_files
    gated = set() if whole else test_lines(code)
    source = text.splitlines()
    items = functions(code)

    def line_of(index: int) -> int:
        return code.count("\n", 0, index) + 1

    def marked(line: int) -> bool:
        return any(MARKER in source[number - 1] for number in (line - 1, line) if 1 <= number <= len(source))

    found = []
    for call in BIND_CALL.finditer(code):
        line = line_of(call.start())
        if not (whole or line in gated):
            continue
        open_index = call.end() - 1
        close_index = closing(code, open_index)
        if not PORT_ZERO.search(text[open_index:close_index + 1]):
            continue
        enclosing = [item for item in items if item[1] < call.start() < item[2]]
        function = max(enclosing, key=lambda item: item[1]) if enclosing else None
        returns_a_port = bool(
            function and PORT_TYPE.search(function[3]) and not SOCKET_TYPE.search(function[3])
        )
        if not (reads_address_at_once(code, close_index + 1) or returns_a_port):
            continue
        if marked(line) or (function and marked(line_of(function[0]))):
            continue
        found.append((line, source[line - 1].strip()))
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
        print("A test never binds port 0 and drops the listener while its port goes on (specification 22): let the "
              "program listen on port 0 and read the port it bound, or keep the listener. A helper whose callers try "
              "again on a taken port says so with `// port-retry-ok: <why>`:")
        print("\n".join(failures))
        return 1
    print("no test reserves a port and releases it")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
