"""Immutable mechanical records emitted by the specification-source scanner."""

from __future__ import annotations

import hashlib
from dataclasses import asdict, dataclass, replace
from typing import Any


BLOCK_KINDS = (
    "heading",
    "paragraph",
    "list_item",
    "table_row",
    "blockquote",
    "directive",
)


def stable_document_id(path: str) -> str:
    return "SPEC_DOC_" + hashlib.sha256(path.encode()).hexdigest()[:24].upper()


def stable_block_id(document_id: str, section_identity: tuple[str, ...], ordinal: int) -> str:
    identity = "\0".join((document_id, *section_identity, str(ordinal)))
    return "PUBLIC_BLOCK_" + hashlib.sha256(identity.encode()).hexdigest()[:24].upper()


def stable_heading_id(document_id: str, section_identity: tuple[str, ...]) -> str:
    identity = "\0".join((document_id, *section_identity))
    return "SPEC_HEADING_" + hashlib.sha256(identity.encode()).hexdigest()[:24].upper()


@dataclass(frozen=True)
class ScanDiagnostic:
    severity: str
    kind: str
    path: str
    line: int
    message: str

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class LocalReferenceObservation:
    kind: str
    raw_target: str
    source_line: int
    target_path: str | None
    fragment: str | None
    target_line_start: int | None = None
    target_line_end: int | None = None
    exists: bool | None = None
    tracked: bool | None = None
    fragment_exists: bool | None = None

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class HeadingObservation:
    heading_id: str
    text: str
    visible_text: str
    level: int
    line: int
    anchor: str
    heading_path: tuple[str, ...]
    section_identity: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        payload = asdict(self)
        payload["heading_path"] = list(self.heading_path)
        payload["section_identity"] = list(self.section_identity)
        return payload


@dataclass(frozen=True)
class SpecDocumentFact:
    document_id: str
    path: str
    format: str
    content_sha256: str
    title: str | None
    in_spec_document_scope: bool
    primary_public_surface: bool
    public_entry_paths: tuple[str, ...]
    headings: tuple[HeadingObservation, ...]
    local_references: tuple[LocalReferenceObservation, ...]

    def to_dict(self) -> dict[str, Any]:
        payload = asdict(self)
        payload["public_entry_paths"] = list(self.public_entry_paths)
        payload["headings"] = [item.to_dict() for item in self.headings]
        payload["local_references"] = [item.to_dict() for item in self.local_references]
        return payload


@dataclass(frozen=True)
class PublicProseBlock:
    block_id: str
    document_id: str
    path: str
    line_start: int
    line_end: int
    heading_path: tuple[str, ...]
    section_identity: tuple[str, ...]
    block_kind: str
    block_ordinal: int
    text: str
    text_sha256: str
    visible_text: str
    visible_text_sha256: str
    public_entry_paths: tuple[str, ...]
    local_references: tuple[LocalReferenceObservation, ...]

    def with_public_entry(self, path: str) -> "PublicProseBlock":
        return replace(self, public_entry_paths=tuple(sorted({*self.public_entry_paths, path})))

    def to_dict(self) -> dict[str, Any]:
        payload = asdict(self)
        payload["heading_path"] = list(self.heading_path)
        payload["section_identity"] = list(self.section_identity)
        payload["public_entry_paths"] = list(self.public_entry_paths)
        payload["local_references"] = [item.to_dict() for item in self.local_references]
        return payload


@dataclass(frozen=True)
class SpecSourceScan:
    documents: tuple[SpecDocumentFact, ...]
    public_blocks: tuple[PublicProseBlock, ...]
    diagnostics: tuple[ScanDiagnostic, ...]
    input_paths: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "documents": [item.to_dict() for item in self.documents],
            "public_blocks": [item.to_dict() for item in self.public_blocks],
            "diagnostics": [item.to_dict() for item in self.diagnostics],
            "input_paths": list(self.input_paths),
        }
