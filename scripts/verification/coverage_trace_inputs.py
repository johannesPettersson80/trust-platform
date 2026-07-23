"""Canonical input identity shared by all execution-trace adapters."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
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
