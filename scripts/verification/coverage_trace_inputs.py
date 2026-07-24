"""Canonical input identity shared by all execution-trace adapters."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


def trace_input_digest(
    scanner_input_digest: str,
    resolved: Sequence[Mapping[str, Any]],
    code_map: Mapping[str, Any],
) -> str:
    payload = json.dumps(
        {
            "scanner_input_digest": scanner_input_digest,
            "resolved": list(resolved),
            "code_map": code_map,
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def focused_rust_trace_input_provenance(
    *,
    source_input_digest: str,
    resolved: Sequence[Mapping[str, Any]],
    code_map: Mapping[str, Any],
    trace_root: str | None = None,
    raw_evidence_digest: str | None = None,
    llvm_tool_digests: Mapping[str, str] | None = None,
    llvm_tool_paths: Mapping[str, str] | None = None,
) -> dict[str, Any]:
    """Retain every input required to reproduce the focused trace digest."""

    if re.fullmatch(r"sha256:[0-9a-f]{64}", source_input_digest) is None:
        raise ValueError("focused Rust source input digest is malformed")
    resolved_rows = [dict(row) for row in resolved]
    code_map_document = dict(code_map)
    result = {
        "kind": "focused_rust_v1",
        "source_input_digest": source_input_digest,
        "resolution_digest": _canonical_digest(resolved_rows),
        "code_map_digest": _canonical_digest(code_map_document),
        "resolved": resolved_rows,
        "input_digest": trace_input_digest(
            source_input_digest,
            resolved_rows,
            code_map_document,
        ),
    }
    if (
        trace_root is not None
        or raw_evidence_digest is not None
        or llvm_tool_digests is not None
        or llvm_tool_paths is not None
    ):
        if (
            not isinstance(trace_root, str)
            or not trace_root.startswith("target/gate-artifacts/")
            or ".." in trace_root.split("/")
        ):
            raise ValueError("focused Rust trace root is malformed")
        if (
            not isinstance(raw_evidence_digest, str)
            or re.fullmatch(r"sha256:[0-9a-f]{64}", raw_evidence_digest) is None
        ):
            raise ValueError("focused Rust raw evidence digest is malformed")
        result["trace_root"] = trace_root
        result["raw_evidence_digest"] = raw_evidence_digest
        if (
            not isinstance(llvm_tool_digests, Mapping)
            or set(llvm_tool_digests) != {"llvm-profdata", "llvm-cov"}
            or any(
                not isinstance(value, str)
                or re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None
                for value in llvm_tool_digests.values()
            )
        ):
            raise ValueError("focused Rust LLVM tool digests are malformed")
        result["llvm_tool_digests"] = dict(sorted(llvm_tool_digests.items()))
        if (
            not isinstance(llvm_tool_paths, Mapping)
            or set(llvm_tool_paths) != {"llvm-profdata", "llvm-cov"}
            or any(
                not isinstance(value, str) or not Path(value).is_absolute()
                for value in llvm_tool_paths.values()
            )
        ):
            raise ValueError("focused Rust LLVM tool paths are malformed")
        result["llvm_tool_paths"] = dict(sorted(llvm_tool_paths.items()))
    return result


def _canonical_digest(value: Any) -> str:
    payload = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(payload).hexdigest()
