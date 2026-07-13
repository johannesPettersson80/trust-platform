"""Small lexical Markdown scanner for visible prose and local references."""

from __future__ import annotations

import hashlib
import html
import re
from dataclasses import dataclass
from pathlib import PurePosixPath
from urllib.parse import unquote

from .spec_source_models import (
    HeadingObservation,
    LocalReferenceObservation,
    PublicProseBlock,
    ScanDiagnostic,
    stable_block_id,
    stable_document_id,
    stable_heading_id,
)


FENCE_RE = re.compile(r"^[ ]{0,3}(`{3,}|~{3,})(?:[^`]*)$")
HEADING_RE = re.compile(r"^[ ]{0,3}(#{1,6})[ \t]+(.+?)[ \t]*#*[ \t]*$")
SETEXT_RE = re.compile(r"^[ ]{0,3}(=+|-+)[ \t]*$")
INCLUDE_RE = re.compile(r'^\s*--8<--\s+["\']([^"\']+)["\']\s*$')
IMAGE_RE = re.compile(r"!\[[^]]*\]\(\s*(<[^>]+>|[^\s)]+)(?:\s+[^)]*)?\)")
LINK_RE = re.compile(r"(?<!!)\[[^]]+\]\(\s*(<[^>]+>|[^\s)]+)(?:\s+[^)]*)?\)")
REFERENCE_DEFINITION_RE = re.compile(r"^[ ]{0,3}\[[^]]+\]:\s*(<[^>]+>|\S+)")
INLINE_CODE_RE = re.compile(r"(`+)(.*?)\1")
RENDER_IMAGE_RE = re.compile(r"!\[([^]]*)\]\([^)]*\)")
RENDER_LINK_RE = re.compile(r"(?<!!)\[([^]]+)\]\([^)]*\)")
RENDER_REFERENCE_RE = re.compile(r"!?\[([^]]+)\]\[[^]]*\]")
LIST_MARKER_RE = re.compile(r"(?m)^[ \t]*(?:[-+*]|\d+[.)])[ \t]+")
LIST_ITEM_RE = re.compile(r"^[ \t]*(?:[-+*]|\d+[.)])[ \t]+")
TABLE_ROW_RE = re.compile(r"^[ \t]*\|.*\|[ \t]*$")
TABLE_SEPARATOR_RE = re.compile(r"^[ \t]*\|?[ :|-]+\|[ :|-]*\|?[ \t]*$")
BLOCKQUOTE_RE = re.compile(r"^[ \t]*>[ \t]?")
DIRECTIVE_RE = re.compile(r'^[ \t]*(?:!!!|\?\?\?)[ \t]+\S')
REFERENCE_DEFINITION_LINE_RE = re.compile(r"^[ ]{0,3}\[[^]]+\]:")
THEMATIC_BREAK_RE = re.compile(r"^[ ]{0,3}(?:\*\s*){3,}$|^[ ]{0,3}(?:-\s*){3,}$|^[ ]{0,3}(?:_\s*){3,}$")
HTML_TAG_RE = re.compile(r"</?[A-Za-z][^>]*>")
EMPHASIS_RE = re.compile(r"(\*\*|__|~~|\*|_)(\S(?:.*?\S)?)\1")
SLUG_RE = re.compile(r"[^a-z0-9]+")


@dataclass(frozen=True)
class MarkdownScan:
    title: str | None
    headings: tuple[HeadingObservation, ...]
    blocks: tuple[PublicProseBlock, ...]
    references: tuple[LocalReferenceObservation, ...]
    diagnostics: tuple[ScanDiagnostic, ...]


def scan_public_prose(
    path: str,
    text: str,
    document_id: str | None = None,
) -> tuple[tuple[PublicProseBlock, ...], tuple[ScanDiagnostic, ...]]:
    scan = scan_markdown_document(path, text, document_id=document_id)
    return scan.blocks, scan.diagnostics


def scan_markdown_document(
    path: str,
    text: str,
    *,
    document_id: str | None = None,
) -> MarkdownScan:
    document_id = document_id or stable_document_id(path)
    diagnostics: list[ScanDiagnostic] = []
    blocks: list[PublicProseBlock] = []
    headings: list[HeadingObservation] = []
    references: list[LocalReferenceObservation] = []
    heading_text: list[str] = []
    heading_identity: list[str] = []
    heading_occurrences: dict[tuple[str, ...], int] = {}
    block_ordinals: dict[tuple[tuple[str, ...], str], int] = {}
    pending: list[tuple[int, str]] = []
    pending_kind: str | None = None
    title: str | None = None
    fence: tuple[str, int, int] | None = None
    comment_open_line: int | None = None
    in_indented_code = False

    def flush() -> None:
        nonlocal pending_kind
        if not pending:
            return
        section = tuple(heading_identity)
        kind = pending_kind or "paragraph"
        ordinal_key = (section, kind)
        ordinal = block_ordinals.get(ordinal_key, 0) + 1
        block_ordinals[ordinal_key] = ordinal
        block_text = "\n".join(line.rstrip() for _, line in pending).strip()
        visible_text = render_visible_text(block_text)
        local: list[LocalReferenceObservation] = []
        for line_number, line in pending:
            local.extend(_references_in_line(path, line_number, line))
        references.extend(local)
        blocks.append(
            PublicProseBlock(
                block_id=stable_block_id(document_id, (*section, f"@{kind}"), ordinal),
                document_id=document_id,
                path=path,
                line_start=pending[0][0],
                line_end=pending[-1][0],
                heading_path=tuple(heading_text),
                section_identity=section,
                block_kind=kind,
                block_ordinal=ordinal,
                text=block_text,
                text_sha256=hashlib.sha256(block_text.encode()).hexdigest(),
                visible_text=visible_text,
                visible_text_sha256=hashlib.sha256(visible_text.encode()).hexdigest(),
                public_entry_paths=(),
                local_references=tuple(local),
            )
        )
        pending.clear()
        pending_kind = None

    def emit_heading(level: int, value: str, line_number: int) -> None:
        nonlocal title
        rendered = render_visible_text(value, strip_list_marker=False)
        if title is None:
            title = rendered
        heading_text[:] = heading_text[: level - 1]
        heading_identity[:] = heading_identity[: level - 1]
        parent = tuple(heading_identity)
        slug = _slug(rendered)
        occurrence_key = (*parent, slug)
        occurrence = heading_occurrences.get(occurrence_key, 0) + 1
        heading_occurrences[occurrence_key] = occurrence
        anchor = slug if occurrence == 1 else f"{slug}_{occurrence - 1}"
        heading_text.append(rendered)
        heading_identity.append(f"{slug}#{occurrence}")
        local = tuple(_references_in_line(path, line_number, value))
        references.extend(local)
        section = tuple(heading_identity)
        headings.append(
            HeadingObservation(
                heading_id=stable_heading_id(document_id, section),
                text=value,
                visible_text=rendered,
                level=level,
                line=line_number,
                anchor=anchor,
                heading_path=tuple(heading_text),
                section_identity=section,
            )
        )
        blocks.append(
            PublicProseBlock(
                block_id=stable_block_id(document_id, (*section, "@heading"), 0),
                document_id=document_id,
                path=path,
                line_start=line_number,
                line_end=line_number,
                heading_path=tuple(heading_text),
                section_identity=section,
                block_kind="heading",
                block_ordinal=0,
                text=value,
                text_sha256=hashlib.sha256(value.encode()).hexdigest(),
                visible_text=rendered,
                visible_text_sha256=hashlib.sha256(rendered.encode()).hexdigest(),
                public_entry_paths=(),
                local_references=local,
            )
        )

    for line_number, raw_line in enumerate(text.splitlines(), 1):
        visible, comment_open_line = _strip_html_comments(
            raw_line, line_number, comment_open_line
        )
        if comment_open_line is not None and not visible.strip():
            flush()
            continue
        fence_match = FENCE_RE.match(visible)
        if fence is not None:
            marker, minimum, _opening_line = fence
            if fence_match and fence_match.group(1)[0] == marker and len(fence_match.group(1)) >= minimum:
                fence = None
            flush()
            continue
        if fence_match:
            token = fence_match.group(1)
            fence = (token[0], len(token), line_number)
            flush()
            continue
        include_match = INCLUDE_RE.match(visible)
        if include_match:
            flush()
            reference, error = _include_reference(path, line_number, include_match.group(1))
            if error is not None:
                diagnostics.append(error)
            elif reference is not None:
                references.append(reference)
            continue
        heading_match = HEADING_RE.match(visible)
        if heading_match:
            flush()
            emit_heading(len(heading_match.group(1)), heading_match.group(2).strip(), line_number)
            continue
        if not visible.strip():
            flush()
            in_indented_code = False
            continue
        setext = SETEXT_RE.match(visible)
        if setext and pending_kind == "paragraph" and len(pending) == 1:
            value_line, value = pending.pop()
            pending_kind = None
            emit_heading(1 if setext.group(1).startswith("=") else 2, value.strip(), value_line)
            continue
        definition = REFERENCE_DEFINITION_LINE_RE.match(visible)
        if definition:
            flush()
            references.extend(_references_in_line(path, line_number, visible))
            continue
        if THEMATIC_BREAK_RE.match(visible) or TABLE_SEPARATOR_RE.match(visible):
            flush()
            continue
        indented = visible.startswith("    ") or visible.startswith("\t")
        if in_indented_code:
            if indented:
                continue
            in_indented_code = False
        if indented and pending_kind not in {
            "paragraph",
            "list_item",
            "directive",
            "blockquote",
        }:
            flush()
            in_indented_code = True
            continue
        if TABLE_ROW_RE.match(visible):
            flush()
            pending_kind = "table_row"
            pending.append((line_number, visible))
            flush()
            continue
        if LIST_ITEM_RE.match(visible):
            flush()
            pending_kind = "list_item"
            pending.append((line_number, visible))
            continue
        if BLOCKQUOTE_RE.match(visible):
            if pending_kind != "blockquote":
                flush()
                pending_kind = "blockquote"
            pending.append((line_number, visible))
            continue
        if DIRECTIVE_RE.match(visible):
            flush()
            pending_kind = "directive"
            pending.append((line_number, visible))
            continue
        if pending_kind not in {"paragraph", "list_item", "directive", "blockquote"}:
            flush()
            pending_kind = "paragraph"
        pending.append((line_number, visible))

    flush()
    if fence is not None:
        diagnostics.append(
            ScanDiagnostic(
                severity="error",
                kind="unclosed_code_fence",
                path=path,
                line=fence[2],
                message="Markdown code fence is not closed before end of file",
            )
        )
    if comment_open_line is not None:
        diagnostics.append(
            ScanDiagnostic(
                severity="error",
                kind="unclosed_html_comment",
                path=path,
                line=comment_open_line,
                message="HTML comment is not closed before end of file",
            )
        )
    return MarkdownScan(
        title=title,
        headings=tuple(headings),
        blocks=tuple(blocks),
        references=tuple(references),
        diagnostics=tuple(diagnostics),
    )


def render_visible_text(value: str, *, strip_list_marker: bool = True) -> str:
    """Render a deterministic plain-text view without interpreting semantics."""

    rendered = LIST_MARKER_RE.sub("", value) if strip_list_marker else value
    rendered = RENDER_IMAGE_RE.sub(lambda match: match.group(1), rendered)
    rendered = RENDER_LINK_RE.sub(lambda match: match.group(1), rendered)
    rendered = RENDER_REFERENCE_RE.sub(lambda match: match.group(1), rendered)
    rendered = INLINE_CODE_RE.sub(lambda match: match.group(2), rendered)
    for _ in range(3):
        updated = EMPHASIS_RE.sub(lambda match: match.group(2), rendered)
        if updated == rendered:
            break
        rendered = updated
    rendered = HTML_TAG_RE.sub(" ", rendered)
    rendered = rendered.replace("|", " ")
    return " ".join(html.unescape(rendered).split())


def _strip_html_comments(
    line: str,
    line_number: int,
    open_line: int | None,
) -> tuple[str, int | None]:
    output: list[str] = []
    cursor = 0
    while cursor < len(line):
        if open_line is not None:
            end = line.find("-->", cursor)
            if end < 0:
                return "".join(output), open_line
            cursor = end + 3
            open_line = None
            continue
        start = line.find("<!--", cursor)
        if start < 0:
            output.append(line[cursor:])
            break
        output.append(line[cursor:start])
        cursor = start + 4
        open_line = line_number
    return "".join(output), open_line


def _references_in_line(
    source_path: str,
    line_number: int,
    line: str,
) -> list[LocalReferenceObservation]:
    masked = INLINE_CODE_RE.sub(lambda match: " " * len(match.group(0)), line)
    observations: list[LocalReferenceObservation] = []
    image_spans: set[tuple[int, int]] = set()
    for match in IMAGE_RE.finditer(masked):
        image_spans.add(match.span())
        observation = _markdown_reference(
            "markdown_image", source_path, line_number, match.group(1)
        )
        if observation is not None:
            observations.append(observation)
    for match in LINK_RE.finditer(masked):
        if any(start <= match.start() and match.end() <= end for start, end in image_spans):
            continue
        observation = _markdown_reference(
            "markdown_link", source_path, line_number, match.group(1)
        )
        if observation is not None:
            observations.append(observation)
    definition = REFERENCE_DEFINITION_RE.match(masked)
    if definition:
        observation = _markdown_reference(
            "reference_definition", source_path, line_number, definition.group(1)
        )
        if observation is not None:
            observations.append(observation)
    return observations


def _markdown_reference(
    kind: str,
    source_path: str,
    line_number: int,
    raw_target: str,
) -> LocalReferenceObservation | None:
    raw_target = raw_target.removeprefix("<").removesuffix(">")
    if _is_external_target(raw_target):
        return None
    target, separator, fragment = unquote(raw_target).partition("#")
    target = target.partition("?")[0]
    if target.startswith("/"):
        return None
    if not target:
        target_path = source_path
    else:
        target_path = _normalized_relative_target(source_path, target)
    return LocalReferenceObservation(
        kind=kind,
        raw_target=raw_target,
        source_line=line_number,
        target_path=target_path,
        fragment=fragment if separator else None,
    )


def _include_reference(
    source_path: str,
    line_number: int,
    raw_target: str,
) -> tuple[LocalReferenceObservation | None, ScanDiagnostic | None]:
    parsed = _parse_include_target(raw_target)
    if parsed is None:
        return None, ScanDiagnostic(
            severity="error",
            kind="invalid_include",
            path=source_path,
            line=line_number,
            message=f"snippet include target is malformed: {raw_target!r}",
        )
    target, start, end = parsed
    candidate = PurePosixPath(target)
    if candidate.is_absolute() or ".." in candidate.parts or "." in candidate.parts or "\\" in target:
        return None, ScanDiagnostic(
            severity="error",
            kind="escaping_include",
            path=source_path,
            line=line_number,
            message=f"snippet include must be normalized and workspace-relative: {raw_target!r}",
        )
    return (
        LocalReferenceObservation(
            kind="snippet_include",
            raw_target=raw_target,
            source_line=line_number,
            target_path=candidate.as_posix(),
            fragment=None,
            target_line_start=start,
            target_line_end=end,
        ),
        None,
    )


def _parse_include_target(value: str) -> tuple[str, int | None, int | None] | None:
    parts = value.rsplit(":", 2)
    start: int | None = None
    end: int | None = None
    if len(parts) >= 2 and parts[-1].isdigit():
        end_or_start = int(parts.pop())
        if parts and parts[-1].isdigit():
            start = int(parts.pop())
            end = end_or_start
        else:
            start = end_or_start
    target = ":".join(parts)
    if not target or start == 0 or end == 0 or (start and end and start > end):
        return None
    return target, start, end


def _normalized_relative_target(source_path: str, target: str) -> str | None:
    parts: list[str] = list(PurePosixPath(source_path).parent.parts)
    for part in PurePosixPath(target).parts:
        if part in {"", "."}:
            continue
        if part == "..":
            if not parts:
                return None
            parts.pop()
        else:
            parts.append(part)
    return PurePosixPath(*parts).as_posix()


def _is_external_target(target: str) -> bool:
    lowered = target.lower()
    return lowered.startswith(("http://", "https://", "mailto:", "tel:", "data:"))


def _slug(value: str) -> str:
    slug = SLUG_RE.sub("-", value.lower()).strip("-")
    return slug or "section"
