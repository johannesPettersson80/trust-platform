"""Canonical catalog and invariant contract frozen into proof evidence."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping
from typing import Any


class ProofContractError(ValueError):
    """Raised when proof metadata cannot form a complete canonical contract."""


def proof_contract_digest(
    *,
    test: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
) -> str:
    """Digest the complete catalog row and every explicitly linked invariant."""

    payload = proof_contract_payload(test=test, invariants=invariants)
    try:
        canonical = json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
    except (TypeError, ValueError) as exc:
        raise ProofContractError(f"proof contract is not canonical JSON: {exc}") from exc
    return f"sha256:{hashlib.sha256(canonical).hexdigest()}"


def proof_contract_payload(
    *,
    test: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    """Build the content-only proof contract prior to canonical serialization."""

    test_id = test.get("id")
    if not isinstance(test_id, str) or not test_id:
        raise ProofContractError("proof contract test has no valid id")
    invariant_ids = test.get("invariants")
    if not isinstance(invariant_ids, list) or not all(
        isinstance(invariant_id, str) and invariant_id for invariant_id in invariant_ids
    ):
        raise ProofContractError(f"{test_id} invariants must be a string array")

    seen: set[str] = set()
    invariant_records: list[dict[str, Any]] = []
    for invariant_id in invariant_ids:
        if invariant_id in seen:
            raise ProofContractError(f"{test_id} has duplicate invariant {invariant_id}")
        seen.add(invariant_id)
        invariant = invariants.get(invariant_id)
        if not isinstance(invariant, Mapping):
            raise ProofContractError(f"{test_id} links unknown invariant {invariant_id}")
        if invariant.get("id") != invariant_id:
            raise ProofContractError(
                f"{test_id} invariant {invariant_id} record has mismatched id "
                f"{invariant.get('id')!r}"
            )
        invariant_records.append(_content_record(invariant))

    return {
        "schema_version": 1,
        "test": _content_record(test),
        "invariants": invariant_records,
    }


def _content_record(value: Mapping[str, Any]) -> dict[str, Any]:
    return {
        key: _content_value(item)
        for key, item in value.items()
        if isinstance(key, str) and not key.startswith("_")
    }


def _content_value(value: Any) -> Any:
    if isinstance(value, Mapping):
        return _content_record(value)
    if isinstance(value, list):
        return [_content_value(item) for item in value]
    return value
