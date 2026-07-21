"""Frozen Phase 17 code-to-spec debt baseline contract."""

from __future__ import annotations

import re
import json
from typing import Any, Mapping

from .code_spec_common import CodeFact


def build_debt_baseline(
    facts: Mapping[str, CodeFact],
    mappings: Mapping[str, Mapping[str, Any]],
    *,
    created_from_commit: str,
    captured_on: str,
) -> dict[str, Any]:
    entries = []
    for code_id, fact in sorted(facts.items()):
        if code_id in mappings:
            continue
        entries.append(_entry_from_fact(fact, captured_on=captured_on))
    return {
        "schema_version": 1,
        "created_from_commit": created_from_commit,
        "captured_on": captured_on,
        "enforcement_mode": "changed",
        "append_policy": "delete_only",
        "entries": entries,
    }


def validate_debt_partition(
    document: Mapping[str, Any],
    *,
    live_facts: Mapping[str, CodeFact],
    mappings: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    failures: list[str] = []
    expected_root = {
        "schema_version",
        "created_from_commit",
        "captured_on",
        "enforcement_mode",
        "append_policy",
        "entries",
    }
    if set(document) != expected_root:
        failures.append("debt baseline root fields drift")
    if document.get("schema_version") != 1:
        failures.append("debt baseline schema_version must equal 1")
    if re.fullmatch(r"[0-9a-f]{40}", str(document.get("created_from_commit", ""))) is None:
        failures.append("debt baseline created_from_commit must be a full Git SHA")
    if document.get("append_policy") != "delete_only":
        failures.append("debt baseline append_policy must equal delete_only")
    mode = document.get("enforcement_mode")
    if mode not in {"changed", "full"}:
        failures.append("debt baseline enforcement_mode must be changed or full")
    entries = document.get("entries")
    if not isinstance(entries, list):
        return failures + ["debt baseline entries must be an array"]
    if mode == "full" and entries:
        failures.append("full enforcement requires an empty debt baseline")

    by_id: dict[str, Mapping[str, Any]] = {}
    for index, entry in enumerate(entries):
        if not isinstance(entry, Mapping):
            failures.append(f"debt entry[{index}] must be a table")
            continue
        code_id = entry.get("code_id")
        if not isinstance(code_id, str) or not code_id:
            failures.append(f"debt entry[{index}] has invalid code_id")
            continue
        if code_id in by_id:
            failures.append(f"{code_id}: duplicate debt baseline entry")
            continue
        by_id[code_id] = entry

    mapped_ids = set(mappings)
    baseline_ids = set(by_id)
    for code_id in sorted(mapped_ids & baseline_ids):
        failures.append(f"{code_id}: overlap between map and debt baseline")
    for code_id in sorted(set(live_facts) - mapped_ids - baseline_ids):
        failures.append(f"{code_id}: missing_spec new fact absent from map and frozen baseline")
    for code_id in sorted(baseline_ids - set(live_facts)):
        failures.append(f"{code_id}: invented_or_stale_debt_identity")
    for code_id in sorted(baseline_ids & set(live_facts)):
        expected = _entry_from_fact(
            live_facts[code_id],
            captured_on=str(by_id[code_id].get("captured_on", "")),
        )
        entry = by_id[code_id]
        if set(entry) != set(expected):
            failures.append(f"{code_id}: debt baseline fields drift")
        for field, value in expected.items():
            actual = entry.get(field)
            if field == "risk_signals" and isinstance(actual, list):
                actual = sorted(actual)
                value = sorted(value)
            if actual != value:
                category = "stale_spec_review" if field in {
                    "signature",
                    "signature_digest",
                    "body_digest",
                    "risk_signals",
                } else "stale_baseline_binding"
                failures.append(f"{code_id}: {category} debt {field} changed")
    return sorted(set(failures))


def validate_delete_only_history(
    current: Mapping[str, Any],
    frozen: Mapping[str, Any],
) -> list[str]:
    failures: list[str] = []
    immutable = {
        "schema_version",
        "created_from_commit",
        "captured_on",
        "append_policy",
    }
    for field in immutable:
        if current.get(field) != frozen.get(field):
            failures.append(f"debt baseline frozen field edited: {field}")
    frozen_entries = _index_entries(frozen.get("entries"), failures, "frozen")
    current_entries = _index_entries(current.get("entries"), failures, "current")
    for code_id in sorted(set(current_entries) - set(frozen_entries)):
        failures.append(f"{code_id}: debt baseline append is forbidden")
    for code_id in sorted(set(current_entries) & set(frozen_entries)):
        if current_entries[code_id] != frozen_entries[code_id]:
            failures.append(f"{code_id}: debt baseline field edit is forbidden")
    current_mode = current.get("enforcement_mode")
    frozen_mode = frozen.get("enforcement_mode")
    if current_mode != frozen_mode:
        if not (
            frozen_mode == "changed"
            and current_mode == "full"
            and not current_entries
        ):
            failures.append("debt baseline enforcement_mode changed before debt reached zero")
    return sorted(set(failures))


def render_debt_baseline_toml(document: Mapping[str, Any]) -> str:
    """Render the closed baseline shape deterministically."""

    lines = [
        f"schema_version = {int(document['schema_version'])}",
        f"created_from_commit = {json.dumps(document['created_from_commit'])}",
        f"captured_on = {json.dumps(document['captured_on'])}",
        f"enforcement_mode = {json.dumps(document['enforcement_mode'])}",
        f"append_policy = {json.dumps(document['append_policy'])}",
    ]
    for entry in document.get("entries", []):
        lines.extend(["", "[[entries]]"])
        for field in (
            "code_id",
            "source_kind",
            "path",
            "qualified_symbol",
            "item_kind",
            "signature",
            "signature_digest",
            "body_digest",
            "owner",
            "area",
            "visibility",
        ):
            lines.append(f"{field} = {json.dumps(entry[field], ensure_ascii=False)}")
        risks = ", ".join(json.dumps(value) for value in entry["risk_signals"])
        lines.append(f"risk_signals = [{risks}]")
        lines.append(f"captured_on = {json.dumps(entry['captured_on'])}")
    return "\n".join(lines) + "\n"


def _entry_from_fact(fact: CodeFact, *, captured_on: str) -> dict[str, Any]:
    return {
        "code_id": fact.code_id,
        "source_kind": fact.source_kind,
        "path": fact.path,
        "qualified_symbol": fact.qualified_symbol,
        "item_kind": fact.item_kind,
        "signature": fact.signature,
        "signature_digest": fact.signature_digest,
        "body_digest": fact.body_digest,
        "owner": fact.owner,
        "area": fact.area,
        "visibility": fact.visibility,
        "risk_signals": list(fact.risk_signals),
        "captured_on": captured_on,
    }


def _index_entries(
    value: Any,
    failures: list[str],
    label: str,
) -> dict[str, Mapping[str, Any]]:
    if not isinstance(value, list):
        failures.append(f"{label} debt entries must be an array")
        return {}
    indexed: dict[str, Mapping[str, Any]] = {}
    for entry in value:
        if not isinstance(entry, Mapping) or not isinstance(entry.get("code_id"), str):
            failures.append(f"{label} debt entry is invalid")
            continue
        code_id = str(entry["code_id"])
        if code_id in indexed:
            failures.append(f"{label} debt entry duplicates {code_id}")
        indexed[code_id] = entry
    return indexed
