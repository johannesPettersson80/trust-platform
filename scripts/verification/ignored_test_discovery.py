"""Mechanical discovery helpers for Phase 3 ignored-test observations."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from pathlib import Path

from .ignored_test_models import IgnoredTestFact, InventoryDiagnostic
from .test_catalog_common import references_in, source_files, stable_discovery_id
from .test_catalog_models import InferredTestFact, ScanDiagnostic
from .test_catalog_rust import scan_rust_file
from .test_catalog_scanner import scan_repository
from .test_catalog_vscode import VSCODE_LITERAL_RE, mask_js_literals, strip_js_comments


PLAYWRIGHT_LITERAL_SKIP_RE = re.compile(
    r"\btest\.skip[ \t]*\([ \t]*(?P<quote>['\"`])"
    r"(?P<title>(?:\\.|(?!(?P=quote)).)*)(?P=quote)"
)
PLAYWRIGHT_SKIP_CANDIDATE_RE = re.compile(
    r"\b(?:test\s*\.\s*(?:skip|fixme)|"
    r"(?:test\s*\.\s*)?describe\s*\.\s*skip)\s*\("
)
CALLBACK_OPEN_RE = re.compile(
    r"(?:async\s+)?function(?:\s+[A-Za-z_$][A-Za-z0-9_$]*)?\s*\([^)]*\)\s*\{"
    r"|(?:async\s*)?\([^)]*\)\s*=>\s*\{",
    re.DOTALL,
)
NODE_SKIP_SENTINEL_RE = re.compile(
    r"\b(?:test|it|describe|suite|context)\s*\.\s*(?:skip|fixme)\s*\("
    r"|\b(?:xdescribe|xit)\s*\(|\bthis\s*\.\s*skip\s*\("
)
VSCODE_UNSUPPORTED_NAMED_SKIP_RE = re.compile(
    r"\b(?P<form>(?:describe|suite|context)\s*\.\s*skip|xdescribe|xit)\s*\("
)
VSCODE_DECLARED_SKIP_CANDIDATE_RE = re.compile(
    r"\b(?:test|it)\s*\.\s*skip\s*\("
)
VSCODE_RUNTIME_SKIP_CANDIDATE_RE = re.compile(r"\bthis\s*\.\s*skip\s*\(")


@dataclass
class IgnoredDiscoveryBatch:
    facts: list[IgnoredTestFact] = field(default_factory=list)
    diagnostics: list[InventoryDiagnostic] = field(default_factory=list)
    input_paths: set[str] = field(default_factory=set)
    scanned_files: int = 0


def discover_playwright_skips(root: Path) -> IgnoredDiscoveryBatch:
    """Discover same-line literal test.skip declarations in tracked capture specs."""

    root = root.resolve()
    batch = IgnoredDiscoveryBatch()
    package_path = root / "scripts/captures/package.json"
    package_relative = "scripts/captures/package.json"
    package: str | None = None
    batch.input_paths.add(package_relative)
    try:
        package_value = json.loads(package_path.read_text()).get("name")
        if isinstance(package_value, str) and package_value:
            package = package_value
        else:
            raise ValueError("package name is missing")
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as exc:
        batch.diagnostics.append(
            InventoryDiagnostic(
                "error", "playwright_package", package_relative, 1, str(exc)
            )
        )
    candidates = [
        path
        for path in source_files(root, "scripts/captures", (".js", ".mjs", ".ts"))
        if ".spec." in path.name
    ]
    batch.scanned_files = len(candidates)
    for path in candidates:
        relative = path.resolve().relative_to(root).as_posix()
        batch.input_paths.add(relative)
        try:
            text = path.read_text()
        except (OSError, UnicodeError) as exc:
            batch.diagnostics.append(
                InventoryDiagnostic("error", "playwright_source_read", relative, 1, str(exc))
            )
            continue
        lines = text.splitlines()
        code_lines = _js_code_lines(text)
        masked = "".join(mask_js_literals(line) for line in code_lines)
        offsets = _line_offsets(text)
        for candidate in PLAYWRIGHT_SKIP_CANDIDATE_RE.finditer(masked):
            line_number = _line_number(text, candidate.start())
            local_offset = candidate.start() - offsets[line_number - 1]
            code_line = code_lines[line_number - 1]
            literal = PLAYWRIGHT_LITERAL_SKIP_RE.match(code_line, local_offset)
            if literal:
                title = _decode_title(literal.group("title"))
                if literal.group("quote") == "`" and "${" in title:
                    batch.diagnostics.append(
                        InventoryDiagnostic(
                            "error",
                            "dynamic_playwright_skip",
                            relative,
                            line_number,
                            "interpolated Playwright skip title has no stable literal identity",
                        )
                    )
                    continue
                native_id = f"{relative}#{title}"
                package_relative_path = Path(relative).relative_to("scripts/captures").as_posix()
                batch.facts.append(
                    IgnoredTestFact(
                        discovery_id=stable_discovery_id(
                            source_kind="playwright_test",
                            package=package,
                            native_id=native_id,
                        ),
                        native_id=native_id,
                        discovery_source_kind="playwright_test",
                        name=title,
                        path=relative,
                        line=line_number,
                        package=package,
                        command_hint=(
                            "cd scripts/captures && npx playwright test "
                            f"{package_relative_path}"
                        ),
                        ignore_state="ignored",
                        ignore_mechanism="playwright_literal_skip",
                        ignore_reason="literal test.skip declaration",
                        reference_candidates=references_in(lines[line_number - 1]),
                    )
                )
            else:
                batch.diagnostics.append(
                    InventoryDiagnostic(
                        "error",
                        "dynamic_playwright_skip",
                        relative,
                        line_number,
                        "Playwright skip does not use a same-line literal test title",
                    )
                )
    batch.facts.sort(key=lambda item: (item.path, item.line, item.name, item.discovery_id))
    batch.diagnostics.sort(
        key=lambda item: (item.path, item.line, item.severity, item.kind, item.message)
    )
    return batch


def associate_vscode_runtime_skips(
    root: Path,
    scanner_facts: list[InferredTestFact] | tuple[InferredTestFact, ...],
    scanner_diagnostics: list[ScanDiagnostic] | tuple[ScanDiagnostic, ...],
) -> IgnoredDiscoveryBatch:
    """Bind each this.skip diagnostic only when it is inside one literal test callback."""

    root = root.resolve()
    batch = IgnoredDiscoveryBatch()
    by_path: dict[str, list[InferredTestFact]] = {}
    for fact in scanner_facts:
        if fact.source_kind == "vscode_test":
            by_path.setdefault(fact.path, []).append(fact)
    for facts in by_path.values():
        facts.sort(key=lambda item: (item.line, item.name, item.stable_id))

    ranges_by_path: dict[str, list[tuple[int, int, InferredTestFact]]] = {}
    for path, facts in by_path.items():
        try:
            text = (root / path).read_text()
        except (OSError, UnicodeError) as exc:
            batch.diagnostics.append(
                InventoryDiagnostic("error", "vscode_source_read", path, 1, str(exc))
            )
            continue
        ranges_by_path[path] = _literal_test_callback_ranges(text, facts)

    for diagnostic in scanner_diagnostics:
        if diagnostic.kind != "conditional_runtime_skip":
            continue
        source = root / diagnostic.path
        try:
            offsets = _line_offsets(source.read_text())
            skip_offset = offsets[diagnostic.line - 1]
        except (OSError, UnicodeError, IndexError) as exc:
            batch.diagnostics.append(
                InventoryDiagnostic(
                    "error",
                    "vscode_runtime_skip_location",
                    diagnostic.path,
                    diagnostic.line,
                    str(exc),
                )
            )
            continue
        owners = [
            fact
            for start, end, fact in ranges_by_path.get(diagnostic.path, [])
            if start <= skip_offset <= end
        ]
        if len(owners) != 1:
            batch.diagnostics.append(
                InventoryDiagnostic(
                    "error",
                    "unassociated_vscode_runtime_skip",
                    diagnostic.path,
                    diagnostic.line,
                    "runtime this.skip() is not contained in exactly one literal test callback",
                )
            )
            continue
        owner = owners[0]
        batch.facts.append(
            IgnoredTestFact(
                discovery_id=owner.stable_id,
                native_id=owner.native_id,
                discovery_source_kind=owner.source_kind,
                name=owner.name,
                path=owner.path,
                line=diagnostic.line,
                package=owner.package,
                command_hint=owner.command_hint,
                ignore_state="conditional",
                ignore_mechanism="vscode_runtime_skip",
                ignore_reason=diagnostic.message,
                reference_candidates=owner.reference_candidates,
            )
        )
    return batch


def discover_vscode_unsupported_skip_markers(root: Path) -> IgnoredDiscoveryBatch:
    """Fail closed when the modeled VS Code surface uses an unsupported skip form."""

    root = root.resolve()
    batch = IgnoredDiscoveryBatch()
    candidates = source_files(root, "editors/vscode/src/test", (".ts", ".js"))
    batch.scanned_files = len(candidates)
    for path in candidates:
        relative = path.resolve().relative_to(root).as_posix()
        batch.input_paths.add(relative)
        try:
            text = path.read_text()
        except (OSError, UnicodeError) as exc:
            batch.diagnostics.append(
                InventoryDiagnostic("error", "vscode_source_read", relative, 1, str(exc))
            )
            continue
        code_lines = _js_code_lines(text)
        masked = "".join(mask_js_literals(line) for line in code_lines)
        offsets = _line_offsets(text)
        unsupported: list[tuple[re.Match[str], str]] = [
            (match, match.group("form"))
            for match in VSCODE_UNSUPPORTED_NAMED_SKIP_RE.finditer(masked)
        ]
        for match in VSCODE_DECLARED_SKIP_CANDIDATE_RE.finditer(masked):
            line_number = _line_number(text, match.start())
            code_line = code_lines[line_number - 1]
            literal = VSCODE_LITERAL_RE.match(code_line)
            local_candidate = VSCODE_DECLARED_SKIP_CANDIDATE_RE.search(
                mask_js_literals(code_line)
            )
            supported = (
                "\n" not in match.group(0)
                and literal is not None
                and literal.group("mode") == ".skip"
                and local_candidate is not None
                and match.start() == offsets[line_number - 1] + local_candidate.start()
            )
            if not supported:
                unsupported.append((match, "dynamic test.skip/it.skip"))
        for match in VSCODE_RUNTIME_SKIP_CANDIDATE_RE.finditer(masked):
            if text[match.start() : match.end() + 1] != "this.skip()":
                unsupported.append((match, "unsupported this.skip formatting"))
        for match, marker in unsupported:
            batch.diagnostics.append(
                InventoryDiagnostic(
                    "error",
                    "unsupported_vscode_skip_requires_identity_support",
                    relative,
                    _line_number(text, match.start()),
                    f"{marker} has no modeled stable test identity",
                )
            )
    batch.diagnostics.sort(
        key=lambda item: (item.path, item.line, item.severity, item.kind, item.message)
    )
    return batch


def discover_repository_ignored_tests(root: Path) -> IgnoredDiscoveryBatch:
    """Return the complete metadata-free Phase 3 discovery result for a repository."""

    root = root.resolve()
    scanner = scan_repository(root)
    scanner_facts = list(scanner.inferred_facts)
    scanner_diagnostics = list(scanner.diagnostics)
    playwright = discover_playwright_skips(root)
    runtime = associate_vscode_runtime_skips(root, scanner_facts, scanner_diagnostics)
    vscode_unsupported = discover_vscode_unsupported_skip_markers(root)
    excluded_rust = discover_excluded_rust_ignore_markers(root)
    excluded_node = discover_excluded_node_skip_markers(root)
    batch = IgnoredDiscoveryBatch(
        facts=[
            scanner_ignore_fact(fact)
            for fact in scanner_facts
            if fact.source_kind.startswith("rust_")
            and fact.ignore_state in {"ignored", "conditional"}
        ],
        diagnostics=[
            *runtime.diagnostics,
            *vscode_unsupported.diagnostics,
            *playwright.diagnostics,
            *excluded_rust.diagnostics,
            *excluded_node.diagnostics,
            *unsupported_scanner_ignore_diagnostics(scanner_facts),
        ],
        input_paths={
            *scanner.provenance.input_paths,
            *vscode_unsupported.input_paths,
            *playwright.input_paths,
            *excluded_rust.input_paths,
            *excluded_node.input_paths,
        },
        scanned_files=len(
            set(scanner.provenance.input_paths)
            | vscode_unsupported.input_paths
            | playwright.input_paths
            | excluded_rust.input_paths
            | excluded_node.input_paths
        ),
    )
    batch.facts.extend(runtime.facts)
    batch.facts.extend(playwright.facts)
    batch.diagnostics.extend(
        InventoryDiagnostic(
            item.severity,
            f"existing_catalog_{item.kind}",
            item.path,
            item.line,
            item.message,
        )
        for item in scanner_diagnostics
        if item.kind != "conditional_runtime_skip"
    )
    batch.facts.sort(
        key=lambda item: (
            item.discovery_source_kind,
            item.path,
            item.name,
            item.discovery_id,
        )
    )
    batch.diagnostics.sort(
        key=lambda item: (item.path, item.line, item.severity, item.kind, item.message)
    )
    return batch


def unsupported_scanner_ignore_diagnostics(
    scanner_facts: list[InferredTestFact] | tuple[InferredTestFact, ...],
) -> list[InventoryDiagnostic]:
    """Keep new ignore mechanisms outside the v1 contract fail-closed."""

    return [
        InventoryDiagnostic(
            "error",
            "unsupported_scanner_ignore_mechanism",
            fact.path,
            fact.line,
            f"{fact.source_kind} {fact.ignore_state} fact needs a reviewed Phase 3 mechanism",
        )
        for fact in scanner_facts
        if fact.ignore_state in {"ignored", "conditional"}
        and not fact.source_kind.startswith("rust_")
    ]


def discover_excluded_rust_ignore_markers(root: Path) -> IgnoredDiscoveryBatch:
    """Fail visibly if an excluded Rust surface gains an ignored test."""

    root = root.resolve()
    batch = IgnoredDiscoveryBatch()
    candidates = [
        *source_files(root, "xtask", (".rs",)),
        *source_files(root, "fuzz", (".rs",)),
        *(
            path
            for path in source_files(root, "crates", (".rs",))
            if "fuzz" in path.relative_to(root).parts
        ),
    ]
    unique = sorted(set(candidates))
    batch.scanned_files = len(unique)
    for path in unique:
        relative = path.resolve().relative_to(root).as_posix()
        batch.input_paths.add(relative)
        scanned = scan_rust_file(
            root,
            path,
            package="phase3-excluded-rust-surface",
            source_kind="rust_unit_test",
            command_prefix="cargo test",
            command_authority="package_only",
        )
        batch.diagnostics.extend(
            InventoryDiagnostic(
                item.severity,
                f"excluded_rust_{item.kind}",
                item.path,
                item.line,
                item.message,
            )
            for item in scanned.diagnostics
        )
        for fact in scanned.facts:
            if fact.ignore_state not in {"ignored", "conditional"}:
                continue
            batch.diagnostics.append(
                InventoryDiagnostic(
                    "error",
                    "excluded_rust_ignore_requires_identity_support",
                    fact.path,
                    fact.line,
                    "ignored Rust test is outside the modeled crate surfaces; add identity support before classification",
                )
            )
    return batch


def discover_excluded_node_skip_markers(root: Path) -> IgnoredDiscoveryBatch:
    """Fail visibly when an unmodeled tracked Node test/spec gains a skip marker."""

    root = root.resolve()
    batch = IgnoredDiscoveryBatch()
    candidates = []
    for path in source_files(root, ".", (".js", ".mjs", ".cjs", ".ts")):
        relative = path.resolve().relative_to(root).as_posix()
        if relative.startswith("editors/vscode/src/test/"):
            continue
        if relative.startswith("scripts/captures/") and ".spec." in path.name:
            continue
        parts = path.relative_to(root).parts
        if not (
            ".test." in path.name
            or ".spec." in path.name
            or ".e2e." in path.name
            or any(
                part in {"test", "tests", "__tests__", "spec", "specs"}
                for part in parts[:-1]
            )
        ):
            continue
        candidates.append(path)
    batch.scanned_files = len(candidates)
    for path in sorted(candidates):
        relative = path.resolve().relative_to(root).as_posix()
        batch.input_paths.add(relative)
        try:
            text = path.read_text()
        except (OSError, UnicodeError) as exc:
            batch.diagnostics.append(
                InventoryDiagnostic("error", "excluded_node_source_read", relative, 1, str(exc))
            )
            continue
        for match in NODE_SKIP_SENTINEL_RE.finditer(_mask_js_source(text)):
            batch.diagnostics.append(
                InventoryDiagnostic(
                    "error",
                    "excluded_node_skip_requires_identity_support",
                    relative,
                    _line_number(text, match.start()),
                    "skip marker is outside modeled VS Code and Playwright surfaces",
                )
            )
    return batch


def scanner_ignore_fact(fact: InferredTestFact) -> IgnoredTestFact:
    """Convert an existing-catalog ignored/conditional fact without adding intent."""

    if fact.ignore_state not in {"ignored", "conditional"}:
        raise ValueError("scanner fact is not ignored or conditional")
    if fact.source_kind.startswith("rust_"):
        mechanism = "rust_cfg_attr" if fact.ignore_state == "conditional" else "rust_attribute"
    elif fact.source_kind == "vscode_test":
        mechanism = "vscode_literal_skip"
    else:
        mechanism = "scanner_declared_ignore"
    return IgnoredTestFact(
        discovery_id=fact.stable_id,
        native_id=fact.native_id,
        discovery_source_kind=fact.source_kind,
        name=fact.name,
        path=fact.path,
        line=fact.line,
        package=fact.package,
        command_hint=fact.command_hint,
        ignore_state=fact.ignore_state,
        ignore_mechanism=mechanism,
        ignore_reason=fact.ignore_reason or "declared ignore marker",
        reference_candidates=fact.reference_candidates,
    )


def _decode_title(value: str) -> str:
    try:
        return bytes(value, "utf-8").decode("unicode_escape")
    except UnicodeDecodeError:
        return value


def _literal_test_callback_ranges(
    text: str,
    facts: list[InferredTestFact],
) -> list[tuple[int, int, InferredTestFact]]:
    masked = _mask_js_source(text)
    offsets = _line_offsets(text)
    result: list[tuple[int, int, InferredTestFact]] = []
    ordered = sorted(facts, key=lambda item: (item.line, item.name, item.stable_id))
    for index, fact in enumerate(ordered):
        if fact.line < 1 or fact.line > len(offsets):
            continue
        start = offsets[fact.line - 1]
        end = offsets[ordered[index + 1].line - 1] if index + 1 < len(ordered) else len(masked)
        match = CALLBACK_OPEN_RE.search(masked, start, end)
        if match is None:
            continue
        opening = match.end() - 1
        closing = _matching_brace(masked, opening)
        if closing is not None:
            result.append((opening, closing, fact))
    return result


def _mask_js_source(text: str) -> str:
    return "".join(mask_js_literals(line) for line in _js_code_lines(text))


def _js_code_lines(text: str) -> list[str]:
    state: str | None = None
    output: list[str] = []
    for line in text.splitlines(keepends=True):
        code, state = strip_js_comments(line, state)
        output.append(code)
    return output


def _matching_brace(text: str, opening: int) -> int | None:
    depth = 0
    for index in range(opening, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return index
    return None


def _line_offsets(text: str) -> list[int]:
    offsets = [0]
    offsets.extend(index + 1 for index, char in enumerate(text) if char == "\n")
    return offsets


def _line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1
