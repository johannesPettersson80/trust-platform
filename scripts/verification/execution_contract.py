"""Stable semantic projections shared by proof and case provenance."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping
from typing import Any


PROOF_CONTRACT_VERSION = "execution_contract_v2"
INVARIANT_EXECUTION_CONTRACT_VERSION = "invariant_execution_contract_v1"

# These fields describe reviewed lifecycle progress, not the command or behavior
# that a proof observed. New fields remain frozen unless explicitly reviewed here.
CATALOG_LIFECYCLE_FIELDS = frozenset(
    {
        "last_reviewed",
        "spec_gap_ref",
        "status",
        "suite_tiers",
    }
)
INVARIANT_LIFECYCLE_FIELDS = frozenset(
    {
        "evidence_refs",
        "gates",
        "last_reviewed",
        "missing",
        "proof_level",
        "spec_gap_refs",
        "status",
        "tests",
    }
)
COVERAGE_CELL_LIFECYCLE_FIELDS = frozenset(
    {
        "rationale",
        "spec_gap_ref",
        "state",
    }
)


class ExecutionContractError(ValueError):
    """Raised when an execution contract cannot be projected canonically."""


def catalog_execution_contract(test: Mapping[str, Any]) -> dict[str, Any]:
    """Project a catalog row without mutable routing and gap bookkeeping."""

    return _project_record(test, excluded=CATALOG_LIFECYCLE_FIELDS)


def invariant_execution_contract(invariant: Mapping[str, Any]) -> dict[str, Any]:
    """Project invariant behavior while excluding proof and closure lifecycle."""

    projected = _project_record(invariant, excluded=INVARIANT_LIFECYCLE_FIELDS)
    if "coverage" in invariant:
        projected["coverage"] = _project_coverage(invariant["coverage"])
    return projected


def invariant_execution_contract_digest(invariant: Mapping[str, Any]) -> str:
    """Digest the versioned semantic invariant projection used by trace cases."""

    return canonical_contract_digest(
        {
            "contract_version": INVARIANT_EXECUTION_CONTRACT_VERSION,
            "invariant": invariant_execution_contract(invariant),
        }
    )


def canonical_contract_digest(payload: Mapping[str, Any]) -> str:
    """Return a canonical SHA-256 digest for an execution-contract payload."""

    try:
        encoded = json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError) as exc:
        raise ExecutionContractError(
            f"execution contract is not canonical JSON: {exc}"
        ) from exc
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def _project_record(
    value: Mapping[str, Any],
    *,
    excluded: frozenset[str],
) -> dict[str, Any]:
    return {
        key: _project_value(item)
        for key, item in value.items()
        if isinstance(key, str) and not key.startswith("_") and key not in excluded
    }


def _project_value(value: Any) -> Any:
    if isinstance(value, Mapping):
        return {
            key: _project_value(item)
            for key, item in value.items()
            if isinstance(key, str) and not key.startswith("_")
        }
    if isinstance(value, list):
        return [_project_value(item) for item in value]
    return value


def _project_coverage(value: Any) -> Any:
    if not isinstance(value, Mapping):
        return _project_value(value)
    projected = {
        key: _project_value(item)
        for key, item in value.items()
        if isinstance(key, str) and not key.startswith("_") and key != "cells"
    }
    cells = value.get("cells")
    if isinstance(cells, list):
        projected["cells"] = [
            _project_record(cell, excluded=COVERAGE_CELL_LIFECYCLE_FIELDS)
            if isinstance(cell, Mapping)
            else _project_value(cell)
            for cell in cells
        ]
    elif "cells" in value:
        projected["cells"] = _project_value(cells)
    return projected
