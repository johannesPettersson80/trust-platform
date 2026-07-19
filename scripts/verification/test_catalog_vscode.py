"""Conservative discovery of literal VS Code extension tests."""

from __future__ import annotations

import json
import re
from pathlib import Path

from .test_catalog_common import (
    ScanBatch,
    diagnostic,
    make_fact,
    references_in,
    relative_path,
    source_files,
)


VSCODE_LITERAL_RE = re.compile(
    r"^\s*(?:test|it)(?P<mode>\.skip|\.only)?\s*\(\s*"
    r"(?P<quote>['\"`])(?P<title>(?:\\.|(?!(?P=quote)).)*)(?P=quote)"
)
VSCODE_CANDIDATE_RE = re.compile(r"^\s*(?:test|it)(?:\.(?:skip|only))?\s*\(")


def scan_vscode_tests(root: Path) -> ScanBatch:
    batch = ScanBatch()
    test_root = root / "editors/vscode/src/test"
    if not test_root.is_dir():
        batch.diagnostics.append(
            diagnostic(
                "missing_scan_root",
                "editors/vscode/src/test",
                1,
                "required VS Code test root is missing",
                severity="error",
            )
        )
        return batch
    package = vscode_package(root, batch)
    for path in source_files(root, "editors/vscode/src/test", (".ts", ".js")):
        relative = relative_path(root, path)
        batch.input_paths.add(relative)
        try:
            lines = path.read_text().splitlines()
        except (OSError, UnicodeError) as exc:
            batch.diagnostics.append(diagnostic("source_read", relative, 1, str(exc), severity="error"))
            continue
        lexical_state: str | None = None
        pending_comments: list[str] = []
        for line_number, line in enumerate(lines, start=1):
            code, lexical_state = strip_js_comments(line, lexical_state)
            stripped = code.strip()
            if not stripped:
                if line.strip().startswith(("//", "/*", "*")):
                    pending_comments.append(line)
                elif not line.strip():
                    pending_comments.clear()
                continue
            if "this.skip()" in mask_js_literals(stripped):
                batch.diagnostics.append(
                    diagnostic(
                        "conditional_runtime_skip",
                        relative,
                        line_number,
                        "runtime this.skip() cannot be represented as a declared ignore attribute",
                    )
                )
            literal = VSCODE_LITERAL_RE.match(code)
            if literal:
                title = decode_js_title(literal.group("title"))
                if literal.group("quote") == "`" and "${" in title:
                    batch.diagnostics.append(
                        diagnostic(
                            "dynamic_test_name",
                            relative,
                            line_number,
                            "template-literal test title contains interpolation",
                        )
                    )
                    pending_comments.clear()
                    continue
                ignored = literal.group("mode") == ".skip"
                if literal.group("mode") == ".only":
                    batch.diagnostics.append(
                        diagnostic(
                            "focused_test",
                            relative,
                            line_number,
                            "test.only is a visible focus marker, not an ignore attribute",
                        )
                    )
                batch.facts.append(
                    make_fact(
                        source_kind="vscode_test",
                        name=title,
                        path=relative,
                        line=line_number,
                        package=package,
                        command_hint="cd editors/vscode && npm test",
                        command_hint_authority="package_only",
                        discovery_confidence="literal_call",
                        ignore_state="ignored" if ignored else "not_ignored",
                        ignore_reason="skip" if ignored else None,
                        reference_candidates=references_in("\n".join([*pending_comments, line])),
                    )
                )
            elif VSCODE_CANDIDATE_RE.match(code):
                batch.diagnostics.append(
                    diagnostic(
                        "dynamic_test_name",
                        relative,
                        line_number,
                        "test call does not use a same-line literal title",
                    )
                )
            pending_comments.clear()
    return batch


def vscode_package(root: Path, batch: ScanBatch) -> str:
    path = root / "editors/vscode/package.json"
    relative = "editors/vscode/package.json"
    if not path.is_file():
        return "vscode-extension"
    batch.input_paths.add(relative)
    try:
        name = json.loads(path.read_text()).get("name")
    except Exception as exc:
        batch.diagnostics.append(diagnostic("package_json_parse", relative, 1, str(exc), severity="error"))
        return "vscode-extension"
    return name if isinstance(name, str) and name else "vscode-extension"


def strip_js_comments(line: str, state: str | None) -> tuple[str, str | None]:
    output: list[str] = []
    index = 0
    quote: str | None = None
    escaped = False
    regex_literal = False
    regex_class = False
    while index < len(line):
        char = line[index]
        nxt = line[index + 1] if index + 1 < len(line) else ""
        if state == "block_comment":
            if char == "*" and nxt == "/":
                state = None
                output.extend("  ")
                index += 2
            else:
                output.append(" ")
                index += 1
            continue
        if state == "template":
            if char == "\\" and index + 1 < len(line):
                output.extend("  ")
                index += 2
                continue
            if char == "`":
                output.append(" ")
                state = None
            else:
                output.append(" ")
            index += 1
            continue
        if regex_literal:
            output.append(" ")
            index += 1
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == "[":
                regex_class = True
            elif char == "]":
                regex_class = False
            elif char == "/" and not regex_class:
                regex_literal = False
            continue
        if quote:
            output.append(char)
            index += 1
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
            continue
        if char in {'"', "'", "`"}:
            quote = char
            output.append(char)
            index += 1
        elif char == "/" and nxt == "/":
            output.extend(" " * (len(line) - index))
            break
        elif char == "/" and nxt == "*":
            output.extend("  ")
            state = "block_comment"
            index += 2
        elif char == "/" and looks_like_regex_start(output):
            output.append(" ")
            regex_literal = True
            regex_class = False
            escaped = False
            index += 1
        else:
            output.append(char)
            index += 1
    if quote == "`":
        state = "template"
    return "".join(output), state


def looks_like_regex_start(output: list[str]) -> bool:
    prefix = "".join(output).rstrip()
    if not prefix:
        return True
    return prefix[-1] in "([{:;,=!?&|"


def mask_js_literals(line: str) -> str:
    output: list[str] = []
    quote: str | None = None
    escaped = False
    for char in line:
        if quote is not None:
            output.append(" ")
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
        elif char in {'"', "'", "`"}:
            quote = char
            output.append(" ")
        else:
            output.append(char)
    return "".join(output)


def decode_js_title(value: str) -> str:
    return re.sub(r"\\(['\"`\\])", r"\1", value)
