"""Canonical catalog and invariant contract frozen into proof evidence."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from .execution_contract import (
    PROOF_CONTRACT_VERSION,
    ExecutionContractError,
    canonical_contract_digest,
    catalog_execution_contract,
    invariant_execution_contract,
)

__all__ = [
    "PROOF_CONTRACT_VERSION",
    "ProofContractError",
    "proof_contract_digest",
    "proof_contract_payload",
]


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
        return canonical_contract_digest(payload)
    except ExecutionContractError as exc:
        raise ProofContractError(str(exc)) from exc


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
        invariant_records.append(invariant_execution_contract(invariant))

    return {
        "contract_version": PROOF_CONTRACT_VERSION,
        "test": catalog_execution_contract(test),
        "invariants": invariant_records,
    }
