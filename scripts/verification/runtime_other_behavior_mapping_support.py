"""Shared document and identity helpers for the runtime-other transaction."""

from __future__ import annotations

import hashlib
import json
import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any


BREAKPOINT_DISCOVERY_ID = "DISC_1D27FB7DCC4AAEE15C39"
BREAKPOINT_BEHAVIOR_REF = "DEBUG_CONTROL_EXECUTION_001#behavior:2"


@dataclass(frozen=True)
class RuntimeOtherMappingCompilation:
    summary: dict[str, int]
    replacements: dict[str, bytes]


def breakpoint_assignment(
    facts: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    fact = facts[BREAKPOINT_DISCOVERY_ID]
    return {
        "behavior_refs": [BREAKPOINT_BEHAVIOR_REF],
        "catalog_test_id": None,
        "command_hint": fact.get("command_hint"),
        "denominator_disposition": None,
        "discovery_id": BREAKPOINT_DISCOVERY_ID,
        "invariant_ids": ["DEBUG_CONTROL_EXECUTION_001"],
        "name": fact["name"],
        "path": fact["path"],
        "resolution": "new_catalog",
        "source_kind": fact["discovery_source_kind"],
    }


def candidate(
    *,
    discovery_id: str,
    invariant_ids: list[str],
) -> dict[str, Any]:
    return {
        "discovery_id": discovery_id,
        "candidate_tier": "C3",
        "candidate_invariant_ids": invariant_ids,
        "covered_code_ids": [],
        "specified_code_ids": [],
        "spec_gap_code_ids": [],
        "spec_gap_refs": [],
        "mutation_kill_ids": [],
    }


def scan_facts(
    scan: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    facts = {}
    for row in records(scan.get("inferred_facts"), "scanner facts"):
        discovery_id = string(row, "stable_id", "scanner fact")
        facts[discovery_id] = {
            "discovery_id": discovery_id,
            "discovery_source_kind": string(row, "source_kind", discovery_id),
            "path": string(row, "path", discovery_id),
            "name": string(row, "name", discovery_id),
            "ignore_state": string(row, "ignore_state", discovery_id),
            "command_hint": row.get("command_hint"),
        }
    if len(facts) != 4399:
        raise ValueError(f"runtime-other scanner fact count drifted: {len(facts)}")
    return facts


def inventories(path: Path) -> dict[str, Mapping[str, Any]]:
    document = json_document(path)
    value = document.get("inventories")
    if not isinstance(value, Mapping) or any(
        not isinstance(key, str) or not isinstance(record, Mapping)
        for key, record in value.items()
    ):
        raise ValueError(f"{path} assertion inventories are invalid")
    return dict(value)


def load_invariants(
    root: Path,
) -> tuple[dict[str, tuple[str, str]], dict[str, Mapping[str, Any]]]:
    sources: dict[str, tuple[str, str]] = {}
    documents: dict[str, Mapping[str, Any]] = {}
    for path in sorted((root / "verification/invariants").rglob("*.toml")):
        source = path.read_text()
        document = toml_source(source, path.as_posix())
        invariant_id = string(document, "id", path.as_posix())
        if invariant_id in documents:
            raise ValueError(f"duplicate invariant {invariant_id}")
        relative = path.relative_to(root).as_posix()
        sources[invariant_id] = (relative, source)
        documents[invariant_id] = document
    return sources, documents


def load_invariant_documents(
    root: Path,
) -> list[tuple[str, Mapping[str, Any]]]:
    return [
        (path.relative_to(root).as_posix(), toml_document(path))
        for path in sorted((root / "verification/invariants").rglob("*.toml"))
    ]


def require_unique_catalog_provenance(
    *,
    catalog_tests: Sequence[Mapping[str, Any]] | Any,
    test_id: str,
    proposal_id: str,
    decision_id: str,
) -> None:
    rows = list(catalog_tests)
    if sum(row.get("mapping_proposal_id") == proposal_id for row in rows) != 1:
        raise ValueError(f"{test_id} proposal provenance is shared or absent")
    if sum(row.get("mapping_decision_id") == decision_id for row in rows) != 1:
        raise ValueError(f"{test_id} decision provenance is shared or absent")


def records_by_id(value: Any, label: str) -> dict[str, Mapping[str, Any]]:
    selected = records(value, label)
    result = {}
    for row in selected:
        record_id = string(row, "id", label)
        if record_id in result:
            raise ValueError(f"{label} duplicate {record_id}")
        result[record_id] = row
    return result


def records(value: Any, label: str) -> list[Mapping[str, Any]]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an object list")
    if any(not isinstance(row, Mapping) for row in value):
        raise ValueError(f"{label} contains a non-object")
    return list(value)


def strings(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be a string list")
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value) or len(result) != len(set(result)):
        raise ValueError(f"{label} must contain distinct non-empty strings")
    return sorted(result)


def string_sequence(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be a string list")
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value):
        raise ValueError(f"{label} must contain non-empty strings")
    return result


def string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value


def json_document(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a JSON object")
    return document


def toml_document(path: Path) -> dict[str, Any]:
    return toml_source(path.read_text(), path.as_posix())


def toml_source(source: str, label: str) -> dict[str, Any]:
    document = tomllib.loads(source)
    if not isinstance(document, dict):
        raise ValueError(f"{label} is not a TOML document")
    return document


def digest_document(document: Mapping[str, Any]) -> str:
    return digest_bytes(
        json.dumps(
            document,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        ).encode()
    )


def digest_documents(*documents: Mapping[str, Any]) -> str:
    digest = hashlib.sha256()
    for document in documents:
        digest.update(digest_document(document).encode())
        digest.update(b"\0")
    return "sha256:" + digest.hexdigest()


def digest_bytes(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()
