"""Discovery of runnable Structured Text test declarations under crate tests."""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

from .test_catalog_common import (
    ScanBatch,
    diagnostic,
    make_fact,
    references_in,
    relative_path,
    source_files,
)


TEST_DECLARATION_RE = re.compile(
    r"^\s*TEST_(PROGRAM|FUNCTION_BLOCK)\s+([A-Za-z_][A-Za-z0-9_]*)\b",
    re.MULTILINE,
)


def scan_structured_text_tests(root: Path) -> ScanBatch:
    batch = ScanBatch()
    crates_root = root / "crates"
    if not crates_root.is_dir():
        batch.diagnostics.append(
            diagnostic(
                "missing_scan_root",
                "crates",
                1,
                "required Structured Text scan root is missing",
                severity="error",
            )
        )
        return batch
    manifests = [
        path
        for path in source_files(root, "crates", (".toml",))
        if path.name == "Cargo.toml" and path.parent.parent == crates_root
    ]
    for manifest in manifests:
        crate_dir = manifest.parent
        tests_dir = crate_dir / "tests"
        if not tests_dir.is_dir():
            continue
        package = load_package_name(root, manifest, batch)
        relative_tests = relative_path(root, tests_dir)
        for path in source_files(root, relative_tests, (".st", ".pou")):
            batch.extend(scan_st_file(root, path, tests_dir=tests_dir, package=package))
    return batch


def load_package_name(root: Path, manifest: Path, batch: ScanBatch) -> str:
    relative = relative_path(root, manifest)
    batch.input_paths.add(relative)
    try:
        name = tomllib.loads(manifest.read_text()).get("package", {}).get("name")
    except Exception as exc:
        batch.diagnostics.append(
            diagnostic("cargo_manifest_parse", relative, 1, str(exc), severity="error")
        )
        return manifest.parent.name
    if not isinstance(name, str) or not name:
        batch.diagnostics.append(
            diagnostic(
                "cargo_package_missing",
                relative,
                1,
                "package.name is missing",
                severity="error",
            )
        )
        return manifest.parent.name
    return name


def scan_st_file(root: Path, path: Path, *, tests_dir: Path, package: str) -> ScanBatch:
    batch = ScanBatch()
    relative = relative_path(root, path)
    batch.input_paths.add(relative)
    try:
        text = path.read_text()
    except (OSError, UnicodeError) as exc:
        batch.diagnostics.append(diagnostic("source_read", relative, 1, str(exc), severity="error"))
        return batch
    project_root = st_project_root(path, tests_dir)
    if project_root is None:
        if TEST_DECLARATION_RE.search(sanitize_st(text)):
            batch.diagnostics.append(
                diagnostic(
                    "st_project_root_missing",
                    relative,
                    1,
                    "test declaration is not under a project src directory",
                    severity="error",
                )
            )
        return batch
    project_relative = relative_path(root, project_root)
    original_lines = text.splitlines()
    code_lines = sanitize_st(text).splitlines()
    pending_comments: list[str] = []
    for line_number, (original, code) in enumerate(
        zip(original_lines, code_lines, strict=False), start=1
    ):
        stripped = code.strip()
        if not stripped:
            if original.strip().startswith("//"):
                pending_comments.append(original)
            elif not original.strip():
                pending_comments.clear()
            continue
        match = TEST_DECLARATION_RE.match(code)
        if match:
            declaration_kind = f"TEST_{match.group(1)}"
            name = match.group(2)
            batch.facts.append(
                make_fact(
                    source_kind="structured_text_test",
                    name=name,
                    native_id=f"{declaration_kind}::{relative}#{name}",
                    path=relative,
                    line=line_number,
                    package=package,
                    command_hint=(
                        "cargo run -p trust-dev -- test "
                        f"--project {project_relative} --filter {name}"
                    ),
                    command_hint_authority="conservative",
                    discovery_confidence="lexical_declaration",
                    reference_candidates=references_in("\n".join([*pending_comments, original])),
                )
            )
        elif re.match(r"^\s*TEST_(?:PROGRAM|FUNCTION_BLOCK)\b", code):
            batch.diagnostics.append(
                diagnostic(
                    "st_test_name_missing",
                    relative,
                    line_number,
                    "Structured Text test declaration has no literal identifier",
                    severity="error",
                )
            )
        pending_comments.clear()
    return batch


def st_project_root(path: Path, tests_dir: Path) -> Path | None:
    for parent in path.parents:
        if parent.name == "src" and parent.parent.is_relative_to(tests_dir):
            return parent.parent
        if parent == tests_dir:
            break
    return None


def sanitize_st(text: str) -> str:
    output: list[str] = []
    index = 0
    state = "code"
    block_depth = 0
    while index < len(text):
        char = text[index]
        nxt = text[index + 1] if index + 1 < len(text) else ""
        if state == "code":
            if char == "/" and nxt == "/":
                output.extend("  ")
                index += 2
                state = "line_comment"
            elif char == "(" and nxt == "*":
                output.extend("  ")
                index += 2
                block_depth = 1
                state = "block_comment"
            elif char == "'":
                output.append(" ")
                index += 1
                state = "string"
            else:
                output.append(char)
                index += 1
            continue
        if state == "line_comment":
            if char == "\n":
                output.append(char)
                state = "code"
            else:
                output.append(" ")
            index += 1
            continue
        if state == "block_comment":
            if char == "(" and nxt == "*":
                output.extend("  ")
                block_depth += 1
                index += 2
            elif char == "*" and nxt == ")":
                output.extend("  ")
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                output.append("\n" if char == "\n" else " ")
                index += 1
            continue
        if char == "'" and nxt == "'":
            output.extend("  ")
            index += 2
        elif char == "'":
            output.append(" ")
            index += 1
            state = "code"
        else:
            output.append("\n" if char == "\n" else " ")
            index += 1
    return "".join(output)
