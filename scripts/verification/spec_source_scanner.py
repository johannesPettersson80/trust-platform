"""Tracked-file orchestration for mechanical specification/public discovery."""

from __future__ import annotations

import hashlib
import subprocess
from dataclasses import dataclass, replace
from pathlib import Path, PurePosixPath

from .spec_source_markdown import MarkdownScan, scan_markdown_document
from .spec_source_models import (
    LocalReferenceObservation,
    PublicProseBlock,
    ScanDiagnostic,
    SpecDocumentFact,
    SpecSourceScan,
    stable_document_id,
)
from .spec_source_scope import is_primary_public_path, is_spec_document_path, is_text_path


class SpecSourceDiscoveryError(RuntimeError):
    """Raised when Git cannot provide the tracked-file denominator."""


@dataclass(frozen=True)
class _LoadedDocument:
    path: str
    text: str
    content_sha256: str
    scan: MarkdownScan


def discover_spec_documents(root: Path) -> SpecSourceScan:
    """Discover tracked documents and the recursively rendered public prose corpus."""

    root = root.resolve()
    tracked_paths = _tracked_paths(root)
    input_paths: set[str] = set()
    diagnostics: list[ScanDiagnostic] = []
    loaded: dict[str, _LoadedDocument | None] = {}
    anchor_cache: dict[str, frozenset[str]] = {}

    def load(path: str) -> _LoadedDocument | None:
        if path in loaded:
            return loaded[path]
        input_paths.add(path)
        candidate = root / path
        if candidate.is_symlink():
            diagnostics.append(
                _error("symlink_document", path, 1, "tracked document is a symlink")
            )
            loaded[path] = None
            return None
        if not candidate.is_file():
            diagnostics.append(
                _error("missing_tracked_document", path, 1, "tracked document is missing")
            )
            loaded[path] = None
            return None
        content = candidate.read_bytes()
        try:
            text = content.decode("utf-8")
        except UnicodeDecodeError as exc:
            diagnostics.append(
                _error(
                    "non_utf8_document",
                    path,
                    1,
                    f"tracked text document is not UTF-8: byte {exc.start}",
                )
            )
            loaded[path] = None
            return None
        markdown = scan_markdown_document(path, text)
        diagnostics.extend(markdown.diagnostics)
        resolved_scan = replace(
            markdown,
            references=tuple(
                _resolve_reference(root, tracked_paths, anchor_cache, reference)
                for reference in markdown.references
            ),
            blocks=tuple(
                replace(
                    block,
                    local_references=tuple(
                        _resolve_reference(root, tracked_paths, anchor_cache, reference)
                        for reference in block.local_references
                    ),
                )
                for block in markdown.blocks
            ),
        )
        value = _LoadedDocument(
            path=path,
            text=text,
            content_sha256=hashlib.sha256(content).hexdigest(),
            scan=resolved_scan,
        )
        loaded[path] = value
        return value

    for path in sorted(path for path in tracked_paths if is_spec_document_path(path)):
        load(path)

    public_entries_by_document: dict[str, set[str]] = {}
    public_blocks: dict[str, PublicProseBlock] = {}
    visited: set[tuple[str, str, int | None, int | None]] = set()

    def visit(
        path: str,
        entry_path: str,
        start: int | None,
        end: int | None,
        stack: tuple[str, ...],
    ) -> None:
        key = (entry_path, path, start, end)
        if key in visited:
            return
        visited.add(key)
        document = load(path)
        if document is None:
            return
        public_entries_by_document.setdefault(path, set()).add(entry_path)
        for block in document.scan.blocks:
            if not _line_range_overlaps(block.line_start, block.line_end, start, end):
                continue
            previous = public_blocks.get(block.block_id, block)
            public_blocks[block.block_id] = previous.with_public_entry(entry_path)
        for reference in document.scan.references:
            if reference.kind != "snippet_include" or not _line_selected(
                reference.source_line, start, end
            ):
                continue
            target = reference.target_path
            if target is None:
                continue
            if target not in tracked_paths:
                diagnostics.append(
                    _error(
                        "untracked_include",
                        path,
                        reference.source_line,
                        f"snippet include target is not tracked: {target}",
                    )
                )
                continue
            if not is_text_path(target):
                continue
            if target in stack:
                diagnostics.append(
                    _error(
                        "include_cycle",
                        path,
                        reference.source_line,
                        "snippet include cycle: " + " -> ".join((*stack, target)),
                    )
                )
                continue
            visit(
                target,
                entry_path,
                reference.target_line_start,
                reference.target_line_end,
                (*stack, target),
            )

    for path in sorted(path for path in tracked_paths if is_primary_public_path(path)):
        visit(path, path, None, None, (path,))

    documents: list[SpecDocumentFact] = []
    for path, document in sorted(loaded.items()):
        if document is None:
            continue
        documents.append(
            SpecDocumentFact(
                document_id=stable_document_id(path),
                path=path,
                format="markdown" if PurePosixPath(path).suffix.lower() == ".md" else "text",
                content_sha256=document.content_sha256,
                title=document.scan.title,
                in_spec_document_scope=is_spec_document_path(path),
                primary_public_surface=is_primary_public_path(path),
                public_entry_paths=tuple(sorted(public_entries_by_document.get(path, set()))),
                headings=document.scan.headings,
                local_references=document.scan.references,
            )
        )
    return SpecSourceScan(
        documents=tuple(documents),
        public_blocks=tuple(
            sorted(public_blocks.values(), key=lambda item: (item.path, item.line_start, item.block_id))
        ),
        diagnostics=tuple(
            sorted(
                set(diagnostics),
                key=lambda item: (item.path, item.line, item.kind, item.message),
            )
        ),
        input_paths=tuple(sorted(input_paths)),
    )


def _tracked_paths(root: Path) -> set[str]:
    try:
        result = subprocess.run(
            ["git", "-C", str(root), "ls-files", "-z", "--cached"],
            check=False,
            capture_output=True,
        )
    except OSError as exc:
        raise SpecSourceDiscoveryError(f"cannot execute git ls-files: {exc}") from exc
    if result.returncode != 0:
        message = result.stderr.decode("utf-8", errors="replace").strip()
        raise SpecSourceDiscoveryError(f"git ls-files failed: {message}")
    paths: set[str] = set()
    for raw in result.stdout.split(b"\0"):
        if not raw:
            continue
        try:
            path = raw.decode("utf-8")
        except UnicodeDecodeError as exc:
            raise SpecSourceDiscoveryError(
                f"tracked path is not UTF-8 at output byte {exc.start}"
            ) from exc
        if _is_normalized_path(path):
            paths.add(path)
        else:
            raise SpecSourceDiscoveryError(f"git returned a noncanonical tracked path: {path!r}")
    return paths


def _resolve_reference(
    root: Path,
    tracked_paths: set[str],
    anchor_cache: dict[str, frozenset[str]],
    reference: LocalReferenceObservation,
) -> LocalReferenceObservation:
    target = reference.target_path
    if target is None or not _is_normalized_path(target):
        return replace(reference, exists=False, tracked=False, fragment_exists=False)
    target = _resolve_target_path(target, tracked_paths)
    candidate = root / target
    exists = candidate.is_file() and not candidate.is_symlink()
    tracked = target in tracked_paths
    fragment_exists: bool | None = None
    if reference.fragment is not None:
        fragment_exists = (
            exists
            and tracked
            and reference.fragment in _heading_anchors(root, target, anchor_cache)
        )
    return replace(
        reference,
        target_path=target,
        exists=exists,
        tracked=tracked,
        fragment_exists=fragment_exists,
    )


def _resolve_target_path(target: str, tracked_paths: set[str]) -> str:
    if target in tracked_paths:
        return target
    candidate = PurePosixPath(target)
    options: list[str] = []
    if candidate.suffix == "":
        options.extend((f"{target}.md", f"{target.rstrip('/')}/index.md"))
    elif target.endswith("/"):
        options.append(f"{target}index.md")
    return next((option for option in options if option in tracked_paths), target)


def _heading_anchors(
    root: Path,
    target: str,
    cache: dict[str, frozenset[str]],
) -> frozenset[str]:
    if target in cache:
        return cache[target]
    if PurePosixPath(target).suffix.lower() != ".md":
        cache[target] = frozenset()
        return cache[target]
    try:
        text = (root / target).read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        cache[target] = frozenset()
        return cache[target]
    scan = scan_markdown_document(target, text)
    cache[target] = frozenset(heading.anchor for heading in scan.headings)
    return cache[target]


def _is_normalized_path(value: str) -> bool:
    if not value or "\\" in value:
        return False
    path = PurePosixPath(value)
    return not path.is_absolute() and "." not in path.parts and ".." not in path.parts


def _line_selected(line: int, start: int | None, end: int | None) -> bool:
    return (start is None or line >= start) and (end is None or line <= end)


def _line_range_overlaps(
    line_start: int,
    line_end: int,
    selected_start: int | None,
    selected_end: int | None,
) -> bool:
    return (selected_end is None or line_start <= selected_end) and (
        selected_start is None or line_end >= selected_start
    )


def _error(kind: str, path: str, line: int, message: str) -> ScanDiagnostic:
    return ScanDiagnostic(severity="error", kind=kind, path=path, line=line, message=message)
