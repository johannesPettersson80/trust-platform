"""Reusable authentication receipts for validated focused Rust traces."""

from __future__ import annotations

import hashlib
import json
import os
import tempfile
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .coverage_trace_evidence import RawTraceEvidence, validate_external_evidence
from .focused_mapping_context import canonical_input_file


@dataclass(frozen=True)
class AuthenticatedTraceReceipt:
    """Receipt-backed source closure accepted by candidate generation."""

    evidence: RawTraceEvidence
    source_originals: Mapping[str, bytes]
    rebuilt: Mapping[str, Any]


def write_authenticated_trace_receipt(
    *,
    root: Path,
    receipt_path: Path,
    trace: Mapping[str, Any],
    requested: Sequence[str],
    evidence: RawTraceEvidence,
    source_originals: Mapping[str, bytes],
) -> dict[str, Any]:
    """Write one canonical receipt after trace validation has succeeded."""

    root = root.resolve()
    requested_ids = sorted(requested)
    if len(requested_ids) != len(set(requested_ids)):
        raise ValueError("authenticated trace receipt request IDs must be unique")
    trace_ids = _trace_requested_ids(trace)
    if trace_ids != requested_ids:
        raise ValueError("authenticated trace receipt request set differs from trace")
    provenance = trace.get("input_provenance")
    if not isinstance(provenance, Mapping):
        raise ValueError("authenticated trace receipt requires trace provenance")
    resolved = provenance.get("resolved")
    if not isinstance(resolved, list) or any(
        not isinstance(row, Mapping) for row in resolved
    ):
        raise ValueError("authenticated trace receipt resolved identity is malformed")
    request_relative = (evidence.trace_root / "requested-tests.json").relative_to(
        root
    ).as_posix()
    request_bytes = evidence.repository_originals.get(request_relative)
    if request_bytes is None:
        raise ValueError("authenticated trace receipt lacks requested-tests evidence")
    normalized_sources = _validated_sources(root, source_originals)
    failures = validate_external_evidence(evidence)
    if failures:
        raise ValueError("; ".join(failures))
    document = {
        "schema_version": 1,
        "kind": "focused_trace_authentication_receipt_v1",
        "source_commit": trace.get("source_commit"),
        "trace_digest": _canonical_digest(trace),
        "rebuilt_trace_digest": _canonical_digest(trace),
        "trace_root": evidence.trace_root.relative_to(root).as_posix(),
        "requested_discovery_ids": requested_ids,
        "requested_tests_digest": _digest(request_bytes),
        "resolved_digest": _canonical_digest(resolved),
        "raw_evidence": {
            "digest": evidence.digest,
            "repository_files": {
                path: _digest(content)
                for path, content in sorted(evidence.repository_originals.items())
            },
            "executable_files": {
                path.as_posix(): _digest(content)
                for path, content in sorted(
                    evidence.executable_originals.items(),
                    key=lambda item: item[0].as_posix(),
                )
            },
            "tool_digests": dict(sorted(evidence.tool_digests.items())),
            "tool_paths": dict(sorted(evidence.tool_paths.items())),
        },
        "source_files": {
            path: _digest(content)
            for path, content in sorted(normalized_sources.items())
        },
    }
    _write_json_atomic(receipt_path, document)
    return document


def load_authenticated_trace_receipt(
    *,
    root: Path,
    receipt_path: Path,
    trace: Mapping[str, Any],
    requested: Sequence[str],
    trace_root: Path,
    llvm_tools_dir: Path,
    evidence: RawTraceEvidence,
) -> AuthenticatedTraceReceipt:
    """Revalidate a receipt without Cargo, nextest, profdata, or llvm-cov."""

    del llvm_tools_dir  # Tool paths and bytes are bound by RawTraceEvidence.
    root = root.resolve()
    receipt_path = canonical_input_file(
        root,
        receipt_path,
        "focused trace authentication receipt",
        required_parent="target/gate-artifacts",
    )
    try:
        receipt = json.loads(receipt_path.read_text())
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("focused trace authentication receipt is unreadable") from error
    if not isinstance(receipt, Mapping):
        raise ValueError("focused trace authentication receipt must be an object")
    required = {
        "schema_version",
        "kind",
        "source_commit",
        "trace_digest",
        "rebuilt_trace_digest",
        "trace_root",
        "requested_discovery_ids",
        "requested_tests_digest",
        "resolved_digest",
        "raw_evidence",
        "source_files",
    }
    if set(receipt) != required:
        raise ValueError("focused trace authentication receipt fields are invalid")
    if receipt.get("schema_version") != 1 or receipt.get("kind") != (
        "focused_trace_authentication_receipt_v1"
    ):
        raise ValueError("focused trace authentication receipt version is unsupported")
    requested_ids = sorted(requested)
    if len(requested_ids) != len(set(requested_ids)):
        raise ValueError("focused trace authentication receipt request IDs must be unique")
    if receipt.get("requested_discovery_ids") != requested_ids:
        raise ValueError("focused trace authentication receipt request set drifted")
    if _trace_requested_ids(trace) != requested_ids:
        raise ValueError("focused trace request set differs from receipt")
    trace_digest = _canonical_digest(trace)
    if receipt.get("trace_digest") != trace_digest or receipt.get(
        "rebuilt_trace_digest"
    ) != trace_digest:
        raise ValueError("focused trace authentication receipt trace digest drifted")
    if receipt.get("source_commit") != trace.get("source_commit"):
        raise ValueError("focused trace authentication receipt source commit drifted")
    resolved = trace.get("input_provenance", {}).get("resolved")
    if not isinstance(resolved, list) or receipt.get("resolved_digest") != (
        _canonical_digest(resolved)
    ):
        raise ValueError("focused trace authentication receipt identity drifted")
    trace_root = trace_root.resolve()
    if trace_root != evidence.trace_root or receipt.get("trace_root") != (
        trace_root.relative_to(root).as_posix()
    ):
        raise ValueError("focused trace authentication receipt root drifted")
    request_relative = (trace_root / "requested-tests.json").relative_to(
        root
    ).as_posix()
    request_bytes = evidence.repository_originals.get(request_relative)
    if request_bytes is None or receipt.get("requested_tests_digest") != _digest(
        request_bytes
    ):
        raise ValueError("focused trace authentication receipt request bytes drifted")
    raw = receipt.get("raw_evidence")
    expected_raw = {
        "digest": evidence.digest,
        "repository_files": {
            path: _digest(content)
            for path, content in sorted(evidence.repository_originals.items())
        },
        "executable_files": {
            path.as_posix(): _digest(content)
            for path, content in sorted(
                evidence.executable_originals.items(),
                key=lambda item: item[0].as_posix(),
            )
        },
        "tool_digests": dict(sorted(evidence.tool_digests.items())),
        "tool_paths": dict(sorted(evidence.tool_paths.items())),
    }
    if raw != expected_raw:
        raise ValueError("focused trace authentication receipt raw evidence drifted")
    failures = validate_external_evidence(evidence)
    if failures:
        raise ValueError("; ".join(failures))
    source_digests = receipt.get("source_files")
    if not isinstance(source_digests, Mapping) or not source_digests:
        raise ValueError("focused trace authentication receipt lacks source closure")
    sources: dict[str, bytes] = {}
    for relative, expected_digest in sorted(source_digests.items()):
        if not isinstance(relative, str) or not isinstance(expected_digest, str):
            raise ValueError("focused trace authentication source binding is invalid")
        source = canonical_input_file(
            root, root / relative, f"focused covered source {relative}"
        )
        content = source.read_bytes()
        if _digest(content) != expected_digest:
            raise ValueError(f"focused trace authentication source drifted: {relative}")
        sources[relative] = content
    return AuthenticatedTraceReceipt(
        evidence=evidence,
        source_originals=sources,
        rebuilt=dict(trace),
    )


def _trace_requested_ids(trace: Mapping[str, Any]) -> list[str]:
    scope = trace.get("scope")
    ids = scope.get("requested_discovery_ids") if isinstance(scope, Mapping) else None
    if not isinstance(ids, list) or any(not isinstance(item, str) for item in ids):
        raise ValueError("focused trace requested identity set is malformed")
    return sorted(ids)


def _validated_sources(
    root: Path, source_originals: Mapping[str, bytes]
) -> dict[str, bytes]:
    if not source_originals:
        raise ValueError("authenticated trace receipt requires source closure")
    sources: dict[str, bytes] = {}
    for relative, expected in sorted(source_originals.items()):
        source = canonical_input_file(
            root, root / relative, f"focused covered source {relative}"
        )
        current = source.read_bytes()
        if current != expected:
            raise ValueError(f"focused covered source changed: {relative}")
        sources[relative] = current
    return sources


def _write_json_atomic(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    content = json.dumps(document, indent=2, sort_keys=True) + "\n"
    descriptor, temporary = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".staged", dir=path.parent
    )
    staged = Path(temporary)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            stream.write(content)
            stream.flush()
            os.fsync(stream.fileno())
        if path.exists() and path.read_text() != content:
            raise ValueError("authenticated trace receipt destination already differs")
        os.replace(staged, path)
    finally:
        if staged.exists():
            staged.unlink()


def _digest(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _canonical_digest(value: Any) -> str:
    return _digest(
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        ).encode()
    )
