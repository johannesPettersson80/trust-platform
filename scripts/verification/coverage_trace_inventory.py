"""Canonical live-code inventory identity for execution traces."""

from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, is_dataclass
from typing import Any, Mapping


def code_inventory_digest(
    facts: Mapping[str, Any],
    spans: Mapping[str, Any],
) -> str:
    """Bind a trace to the exact live facts and source spans used for attribution."""

    if set(facts) != set(spans):
        raise ValueError(
            "code inventory fact/span sets differ: "
            f"facts_only={sorted(set(facts) - set(spans))}, "
            f"spans_only={sorted(set(spans) - set(facts))}"
        )
    records = []
    for code_id in sorted(facts):
        fact = _record(facts[code_id], f"{code_id} fact")
        span = _record(spans[code_id], f"{code_id} span")
        if fact.get("code_id") != code_id or span.get("code_id") != code_id:
            raise ValueError(f"{code_id} inventory key does not match embedded identity")
        records.append({"fact": fact, "span": span})
    encoded = json.dumps(
        records,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _record(value: Any, label: str) -> dict[str, Any]:
    if is_dataclass(value):
        record = asdict(value)
    elif isinstance(value, Mapping):
        record = dict(value)
    else:
        raise ValueError(f"{label} must be a dataclass or mapping")
    return record
