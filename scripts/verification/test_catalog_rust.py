"""Conservative Rust integration and in-source test discovery."""

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


TEST_ATTRIBUTE_RE = re.compile(
    r"#\[\s*(?:test|(?:tokio|async_std)::test|rstest|test_case)\b"
)
FUNCTION_RE = re.compile(
    r"^(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)"
)
IGNORE_RE = re.compile(r'#\[\s*ignore(?:\s*=\s*"([^"]*)")?\s*\]')


def scan_rust_tests(root: Path) -> ScanBatch:
    batch = ScanBatch()
    crates_root = root / "crates"
    if not crates_root.is_dir():
        batch.diagnostics.append(
            diagnostic(
                "missing_scan_root",
                "crates",
                1,
                "required Rust scan root is missing",
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
        package = load_package_name(root, manifest, batch)
        tests_dir = crate_dir / "tests"
        if tests_dir.is_dir():
            test_paths = [
                path
                for path in source_files(root, relative_path(root, tests_dir), (".rs",))
                if path.is_relative_to(tests_dir)
            ]
            for path in test_paths:
                target = integration_target(tests_dir, path)
                if target is None:
                    authority = "package_only"
                    command = f"cargo test -p {package}"
                else:
                    authority = "conservative"
                    command = f"cargo test -p {package} --test {target}"
                batch.extend(
                    scan_rust_file(
                        root,
                        path,
                        package=package,
                        source_kind="rust_integration_test",
                        command_prefix=command,
                        command_authority=authority,
                    )
                )
        src_dir = crate_dir / "src"
        if src_dir.is_dir():
            src_paths = [
                path
                for path in source_files(root, relative_path(root, src_dir), (".rs",))
                if path.is_relative_to(src_dir)
            ]
            for path in src_paths:
                batch.extend(
                    scan_rust_file(
                        root,
                        path,
                        package=package,
                        source_kind="rust_unit_test",
                        command_prefix=f"cargo test -p {package}",
                        command_authority="package_only",
                    )
                )
    return batch


def load_package_name(root: Path, manifest: Path, batch: ScanBatch) -> str:
    if not manifest.is_file():
        return manifest.parent.name
    relative = relative_path(root, manifest)
    batch.input_paths.add(relative)
    try:
        data = tomllib.loads(manifest.read_text())
        name = data.get("package", {}).get("name")
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


def integration_target(tests_dir: Path, path: Path) -> str | None:
    relative = path.relative_to(tests_dir)
    if len(relative.parts) == 1:
        return path.stem
    top_level_target = tests_dir / f"{relative.parts[0]}.rs"
    return relative.parts[0] if top_level_target.is_file() else None


def scan_rust_file(
    root: Path,
    path: Path,
    *,
    package: str,
    source_kind: str,
    command_prefix: str,
    command_authority: str,
) -> ScanBatch:
    batch = ScanBatch()
    relative = relative_path(root, path)
    batch.input_paths.add(relative)
    try:
        text = path.read_text()
    except (OSError, UnicodeError) as exc:
        batch.diagnostics.append(diagnostic("source_read", relative, 1, str(exc), severity="error"))
        return batch
    original_lines = text.splitlines()
    code_lines = sanitize_rust(text).splitlines()
    pending_attributes: list[str] = []
    pending_original_attributes: list[str] = []
    pending_context: list[str] = []
    attribute_depth = 0
    for line_number, (original, code) in enumerate(zip(original_lines, code_lines, strict=False), start=1):
        original_stripped = original.strip()
        code_stripped = code.strip()
        if attribute_depth > 0:
            pending_attributes.append(code_stripped)
            pending_original_attributes.append(original_stripped)
            pending_context.append(original)
            attribute_depth += code.count("[") - code.count("]")
            continue
        if original_stripped.startswith("//") and not code_stripped:
            pending_context.append(original)
            continue
        if not code_stripped:
            if not original_stripped and not pending_attributes:
                pending_context.clear()
            continue
        if code_stripped.startswith("#["):
            pending_attributes.append(code_stripped)
            pending_original_attributes.append(original_stripped)
            pending_context.append(original)
            attribute_depth = code.count("[") - code.count("]")
            continue
        function = FUNCTION_RE.match(code_stripped)
        attributes = " ".join(pending_attributes)
        if function and TEST_ATTRIBUTE_RE.search(attributes):
            name = function.group(1)
            declared_ignore_state, reason = ignore_state(" ".join(pending_original_attributes))
            references = references_in("\n".join([*pending_context, original]))
            batch.facts.append(
                make_fact(
                    source_kind=source_kind,
                    name=name,
                    path=relative,
                    line=line_number,
                    package=package,
                    command_hint=f"{command_prefix} {name}",
                    command_hint_authority=command_authority,
                    discovery_confidence="exact_attribute",
                    ignore_state=declared_ignore_state,
                    ignore_reason=reason,
                    reference_candidates=references,
                )
            )
        pending_attributes.clear()
        pending_original_attributes.clear()
        pending_context.clear()
        attribute_depth = 0
    if pending_attributes and TEST_ATTRIBUTE_RE.search(" ".join(pending_attributes)):
        batch.diagnostics.append(
            diagnostic(
                "rust_test_without_function",
                relative,
                len(original_lines),
                "test attribute was not followed by a function",
                severity="error",
            )
        )
    return batch


def ignore_state(attributes: str) -> tuple[str, str | None]:
    match = IGNORE_RE.search(attributes)
    if match:
        return "ignored", match.group(1) or "ignore"
    if "cfg_attr" in attributes and re.search(r"\bignore\b", attributes):
        reason = re.search(r'\bignore\s*=\s*"([^"]*)"', attributes)
        return "conditional", reason.group(1) if reason else "cfg_attr"
    return "not_ignored", None


def sanitize_rust(text: str) -> str:
    """Blank comments and literals while preserving newlines and code offsets."""

    output: list[str] = []
    index = 0
    state = "code"
    block_depth = 0
    raw_hashes = ""
    escaped = False
    while index < len(text):
        char = text[index]
        nxt = text[index + 1] if index + 1 < len(text) else ""
        if state == "code":
            if char == "/" and nxt == "/":
                output.extend("  ")
                index += 2
                state = "line_comment"
                continue
            if char == "/" and nxt == "*":
                output.extend("  ")
                index += 2
                state = "block_comment"
                block_depth = 1
                continue
            raw = re.match(r'(?:br|r)(?P<hashes>#{0,255})"', text[index:])
            if raw:
                token = raw.group(0)
                output.extend(" " * len(token))
                index += len(token)
                raw_hashes = raw.group("hashes")
                state = "raw_string"
                continue
            character = re.match(
                r"(?:b)?'(?:\\(?:x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f_]+\}|.)|[^'\\\r\n])'",
                text[index:],
            )
            if character:
                token = character.group(0)
                output.extend(" " * len(token))
                index += len(token)
                continue
            if char == '"' or (char == "b" and nxt == '"'):
                width = 2 if char == "b" else 1
                output.extend(" " * width)
                index += width
                state = "string"
                escaped = False
                continue
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
            if char == "/" and nxt == "*":
                output.extend("  ")
                block_depth += 1
                index += 2
            elif char == "*" and nxt == "/":
                output.extend("  ")
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                output.append("\n" if char == "\n" else " ")
                index += 1
            continue
        if state == "string":
            output.append("\n" if char == "\n" else " ")
            index += 1
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                state = "code"
            continue
        terminator = '"' + raw_hashes
        if text.startswith(terminator, index):
            output.extend(" " * len(terminator))
            index += len(terminator)
            state = "code"
        else:
            output.append("\n" if char == "\n" else " ")
            index += 1
    return "".join(output)
