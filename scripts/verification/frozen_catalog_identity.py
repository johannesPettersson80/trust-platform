"""Stable identities for frozen catalogue selections."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from typing import Any


def selection_digest(reviews: Sequence[Mapping[str, Any]]) -> str:
    """Bind a reviewed batch to its exact frozen denominator identities."""

    rows = [
        {
            "discovery_id": _required_string(
                row, "discovery_id", "denominator review"
            ),
            "discovery_source_kind": _required_string(
                row, "discovery_source_kind", "denominator review"
            ),
            "path": _required_string(row, "path", "denominator review"),
            "name": _required_string(row, "name", "denominator review"),
            "ignore_state": _required_string(
                row, "ignore_state", "denominator review"
            ),
        }
        for row in reviews
    ]
    rows.sort(key=lambda row: row["discovery_id"])
    return digest_document({"reviews": rows})


def digest_document(document: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        document,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def digest_bytes(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
