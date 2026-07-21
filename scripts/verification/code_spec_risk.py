"""Closed symbol-specific acknowledgement contract for Phase 17 risk facts."""

from __future__ import annotations

import re
from collections.abc import Mapping
from datetime import date
from typing import Any

from .code_spec_common import CodeFact


RISK_ACK_PATTERN = re.compile(r"^RISK_ACK_[0-9A-F]{64}$")
RISK_ACK_FIELDS = frozenset(
    {
        "schema_version",
        "id",
        "code_id",
        "owner",
        "area",
        "body_digest",
        "risk_signals",
        "review_ref",
        "reviewed_by",
        "reviewed_on",
        "expires_on",
        "rationale",
    }
)


def risk_ack_id(code_id: str) -> str:
    """Derive the one-to-one acknowledgement identity for a code fact."""

    prefix = "CODE_"
    if not code_id.startswith(prefix):
        raise ValueError(f"risk acknowledgement requires a CODE_* identity: {code_id}")
    identity = f"RISK_ACK_{code_id.removeprefix(prefix)}"
    if RISK_ACK_PATTERN.fullmatch(identity) is None:
        raise ValueError(f"risk acknowledgement identity is invalid: {identity}")
    return identity


def owner_ack_is_anchored(value: object) -> bool:
    """Return whether a mapping names one exact machine-readable ack record."""

    return isinstance(value, str) and RISK_ACK_PATTERN.fullmatch(value) is not None


def validate_risk_acknowledgement(
    acknowledgement: Mapping[str, Any],
    *,
    fact: CodeFact,
    mapping: Mapping[str, Any],
) -> list[str]:
    """Bind one acknowledgement to one exact reviewed risk-bearing fact."""

    code_id = fact.code_id
    failures: list[str] = []
    if set(acknowledgement) != RISK_ACK_FIELDS:
        failures.append(f"{code_id}: risk acknowledgement fields drift")
    if acknowledgement.get("schema_version") != 1:
        failures.append(f"{code_id}: risk acknowledgement schema_version must equal 1")
    expected_id = risk_ack_id(code_id)
    if acknowledgement.get("id") != expected_id:
        failures.append(f"{code_id}: risk acknowledgement identity changed")
    if acknowledgement.get("code_id") != code_id:
        failures.append(f"{code_id}: risk acknowledgement code identity changed")
    if acknowledgement.get("owner") != fact.owner:
        failures.append(f"{code_id}: risk acknowledgement owner changed")
    if acknowledgement.get("area") != fact.area:
        failures.append(f"{code_id}: risk acknowledgement area changed")
    if acknowledgement.get("body_digest") != fact.body_digest:
        failures.append(f"{code_id}: risk acknowledgement body digest changed")
    risks = acknowledgement.get("risk_signals")
    normalized_risks = sorted(risks) if isinstance(risks, list) else risks
    if normalized_risks != sorted(fact.risk_signals):
        failures.append(f"{code_id}: risk acknowledgement risk signals changed")
    if acknowledgement.get("expires_on") != mapping.get("owner_ack_expires_on"):
        failures.append(f"{code_id}: risk acknowledgement expiry differs from mapping")
    if acknowledgement.get("reviewed_by") != fact.owner:
        failures.append(f"{code_id}: risk acknowledgement must be reviewed by its owner")
    review_ref = acknowledgement.get("review_ref")
    if not isinstance(review_ref, str) or "#" not in review_ref or not review_ref.strip("#"):
        failures.append(f"{code_id}: risk acknowledgement review_ref must name an exact anchor")
    rationale = acknowledgement.get("rationale")
    if not isinstance(rationale, str) or not rationale.strip():
        failures.append(f"{code_id}: risk acknowledgement rationale must be nonempty")
    reviewed_on = _iso_date(acknowledgement.get("reviewed_on"))
    expires_on = _iso_date(acknowledgement.get("expires_on"))
    if reviewed_on is None:
        failures.append(f"{code_id}: risk acknowledgement reviewed_on must be a valid ISO date")
    if expires_on is None:
        failures.append(f"{code_id}: risk acknowledgement expires_on must be a valid ISO date")
    elif expires_on < date.today():
        failures.append(f"{code_id}: risk acknowledgement ledger entry expired")
    if reviewed_on is not None and expires_on is not None and reviewed_on > expires_on:
        failures.append(f"{code_id}: risk acknowledgement review date is after expiry")
    return failures


def index_risk_acknowledgements(
    document: Mapping[str, Any],
) -> tuple[dict[str, Mapping[str, Any]], list[str]]:
    """Index the closed ledger without allowing duplicate identities."""

    failures: list[str] = []
    if set(document) != {"schema_version", "acknowledgements"}:
        failures.append("risk acknowledgement ledger root fields drift")
    if document.get("schema_version") != 1:
        failures.append("risk acknowledgement ledger schema_version must equal 1")
    records = document.get("acknowledgements")
    if not isinstance(records, list):
        return {}, failures + ["risk acknowledgement ledger acknowledgements must be an array"]
    indexed: dict[str, Mapping[str, Any]] = {}
    for index, record in enumerate(records):
        if not isinstance(record, Mapping):
            failures.append(f"risk acknowledgement[{index}] must be a table")
            continue
        acknowledgement_id = record.get("id")
        if not owner_ack_is_anchored(acknowledgement_id):
            failures.append(f"risk acknowledgement[{index}] has invalid anchored identity")
            continue
        assert isinstance(acknowledgement_id, str)
        if acknowledgement_id in indexed:
            failures.append(f"{acknowledgement_id}: duplicate risk acknowledgement")
            continue
        indexed[acknowledgement_id] = record
    return dict(sorted(indexed.items())), failures


def validate_risk_ack_document(
    document: Mapping[str, Any],
    *,
    live_facts: Mapping[str, CodeFact],
    mappings: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    """Validate a complete acknowledgement ledger against mapped live facts."""

    indexed, failures = index_risk_acknowledgements(document)
    expected_ids: set[str] = set()
    for code_id, mapping in sorted(mappings.items()):
        fact = live_facts.get(code_id)
        if fact is None or not fact.risk_signals:
            continue
        acknowledgement_id = mapping.get("owner_ack")
        if owner_ack_is_anchored(acknowledgement_id):
            assert isinstance(acknowledgement_id, str)
            expected_ids.add(acknowledgement_id)
            acknowledgement = indexed.get(acknowledgement_id)
            if acknowledgement is None:
                failures.append(f"{code_id}: missing risk acknowledgement ledger record")
            else:
                failures.extend(
                    validate_risk_acknowledgement(
                        acknowledgement,
                        fact=fact,
                        mapping=mapping,
                    )
                )

    for acknowledgement_id in sorted(set(indexed) - expected_ids):
        record = indexed[acknowledgement_id]
        code_id = record.get("code_id")
        if code_id not in live_facts:
            failures.append(
                f"{acknowledgement_id}: risk acknowledgement names invented or stale code identity"
            )
        elif code_id not in mappings:
            failures.append(f"{acknowledgement_id}: risk acknowledgement has no mapped code fact")
        elif not live_facts[str(code_id)].risk_signals:
            failures.append(f"{acknowledgement_id}: acknowledgement names a non-risk code fact")
        else:
            failures.append(f"{acknowledgement_id}: acknowledgement is not selected by its mapping")
    return sorted(set(failures))


def _iso_date(value: object) -> date | None:
    if not isinstance(value, str):
        return None
    try:
        return date.fromisoformat(value)
    except ValueError:
        return None
