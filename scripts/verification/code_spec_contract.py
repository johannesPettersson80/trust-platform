"""Closed Phase 17 mapping contract and live-fact validation."""

from __future__ import annotations

from collections.abc import Mapping
from datetime import date
from typing import Any

from .code_spec_common import DISPOSITIONS, NON_BEHAVIORAL_REASONS, CodeFact


BASE_FIELDS = frozenset(
    {
        "schema_version",
        "code_id",
        "source_kind",
        "path",
        "qualified_symbol",
        "item_kind",
        "signature_digest",
        "reviewed_body_digest",
        "owner",
        "area",
        "visibility",
        "disposition",
        "risk_signals",
        "last_reviewed",
    }
)
DISPOSITION_FIELDS = {
    "specified": frozenset(
        {"spec_ref", "invariant_ids", "test_ids", "cross_area_decision_ref"}
    ),
    "spec_gap": frozenset({"spec_gap_ref"}),
    "inherits_spec": frozenset(
        {"parent_code_id", "inheritance_review_ref", "cross_area_decision_ref"}
    ),
    "non_behavioral": frozenset({"non_behavioral_reason", "rationale"}),
    "generated": frozenset(
        {
            "generator_code_id",
            "generator_input_digest",
            "generator_path",
            "generator_digest",
        }
    ),
    "vendored_external": frozenset(
        {"provenance", "version_revision", "boundary_spec_ref"}
    ),
}
COMMON_OPTIONAL_FIELDS = frozenset(
    {"owner_ack", "owner_ack_expires_on", "cross_area_decision_ref"}
)


def validate_map_document(
    document: Mapping[str, Any],
    *,
    live_facts: Mapping[str, CodeFact],
    spec_sources: Mapping[str, Mapping[str, Any]] | None = None,
    spec_gaps: Mapping[str, Mapping[str, Any]] | None = None,
    invariants: Mapping[str, Mapping[str, Any]] | None = None,
    tests: Mapping[str, Mapping[str, Any]] | None = None,
) -> list[str]:
    """Validate the committed mapping against the exact live denominator."""

    spec_sources = spec_sources or {}
    spec_gaps = spec_gaps or {}
    invariants = invariants or {}
    tests = tests or {}
    failures: list[str] = []
    if set(document) != {"schema_version", "mappings"}:
        failures.append("code-spec map root must contain only schema_version and mappings")
    if document.get("schema_version") != 1:
        failures.append("code-spec map schema_version must equal 1")
    mappings = document.get("mappings")
    if not isinstance(mappings, list):
        return failures + ["code-spec map mappings must be an array"]

    by_id: dict[str, Mapping[str, Any]] = {}
    for index, raw in enumerate(mappings):
        if not isinstance(raw, Mapping):
            failures.append(f"mapping[{index}]: record must be a table")
            continue
        code_id = raw.get("code_id")
        if not isinstance(code_id, str) or not code_id:
            failures.append(f"mapping[{index}]: invalid code_id")
            continue
        if code_id in by_id:
            failures.append(f"{code_id}: duplicate mapping")
            continue
        by_id[code_id] = raw

    for code_id in sorted(set(live_facts) - set(by_id)):
        failures.append(f"{code_id}: missing_spec")
    for code_id in sorted(set(by_id) - set(live_facts)):
        failures.append(f"{code_id}: invented_or_stale_code_identity")

    for code_id in sorted(set(by_id) & set(live_facts)):
        mapping = by_id[code_id]
        fact = live_facts[code_id]
        failures.extend(_validate_fact_binding(mapping, fact))
        failures.extend(
            _validate_disposition(
                mapping,
                fact=fact,
                mappings=by_id,
                live_facts=live_facts,
                spec_sources=spec_sources,
                spec_gaps=spec_gaps,
                invariants=invariants,
                tests=tests,
            )
        )
    failures.extend(_validate_inheritance_cycles(by_id))
    return sorted(set(failures))


def _validate_fact_binding(mapping: Mapping[str, Any], fact: CodeFact) -> list[str]:
    failures: list[str] = []
    code_id = fact.code_id
    if mapping.get("schema_version") != 1:
        failures.append(f"{code_id}: mapping schema_version must equal 1")
    expected = {
        "source_kind": fact.source_kind,
        "path": fact.path,
        "qualified_symbol": fact.qualified_symbol,
        "item_kind": fact.item_kind,
        "signature_digest": fact.signature_digest,
        "owner": fact.owner,
        "area": fact.area,
        "visibility": fact.visibility,
        "risk_signals": list(fact.risk_signals),
    }
    for field, value in expected.items():
        actual = mapping.get(field)
        if field == "risk_signals" and isinstance(actual, list):
            actual = sorted(actual)
            value = sorted(value)
        if actual != value:
            failures.append(f"{code_id}: stale_spec_review {field} changed")
    if mapping.get("reviewed_body_digest") != fact.body_digest:
        failures.append(f"{code_id}: stale_spec_review body digest changed")
    risk_signals = mapping.get("risk_signals")
    if isinstance(risk_signals, list) and risk_signals:
        if not mapping.get("owner_ack"):
            failures.append(f"{code_id}: high-risk mapping requires owner_ack")
        expiry = mapping.get("owner_ack_expires_on")
        if not isinstance(expiry, str) or not expiry:
            failures.append(
                f"{code_id}: high-risk mapping requires owner_ack_expires_on"
            )
        else:
            try:
                expiry_date = date.fromisoformat(expiry)
            except ValueError:
                failures.append(
                    f"{code_id}: owner_ack_expires_on must be a valid ISO date"
                )
            else:
                if expiry_date < date.today():
                    failures.append(f"{code_id}: owner acknowledgement expired")
    return failures


def _validate_disposition(
    mapping: Mapping[str, Any],
    *,
    fact: CodeFact,
    mappings: Mapping[str, Mapping[str, Any]],
    live_facts: Mapping[str, CodeFact],
    spec_sources: Mapping[str, Mapping[str, Any]],
    spec_gaps: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
) -> list[str]:
    code_id = fact.code_id
    disposition = mapping.get("disposition")
    if disposition not in DISPOSITIONS:
        return [f"{code_id}: unknown disposition {disposition!r}"]
    allowed = BASE_FIELDS | COMMON_OPTIONAL_FIELDS | DISPOSITION_FIELDS[disposition]
    extras = sorted(set(mapping) - allowed)
    failures = [f"{code_id}: field {field} is invalid for {disposition}" for field in extras]

    if disposition == "specified":
        spec_ref = mapping.get("spec_ref")
        invariant_ids = mapping.get("invariant_ids")
        test_ids = mapping.get("test_ids")
        if not isinstance(spec_ref, str) or "#" not in spec_ref:
            failures.append(f"{code_id}: specified mapping requires exact spec_ref fragment")
        else:
            source_id, _fragment = spec_ref.split("#", 1)
            source = spec_sources.get(source_id)
            if not _oracle_eligible(source):
                failures.append(f"{code_id}: specified spec_ref is not active and oracle-eligible")
        if not isinstance(invariant_ids, list) or not invariant_ids:
            failures.append(f"{code_id}: specified mapping requires invariant_ids")
        if not isinstance(test_ids, list) or not test_ids:
            failures.append(f"{code_id}: specified mapping requires test_ids")
        for invariant_id in invariant_ids if isinstance(invariant_ids, list) else []:
            invariant = invariants.get(invariant_id)
            if not invariant:
                failures.append(f"{code_id}: unknown invariant {invariant_id}")
            else:
                if invariant.get("area") != fact.area and not mapping.get("cross_area_decision_ref"):
                    failures.append(f"{code_id}: invariant {invariant_id} crosses area without decision")
                if isinstance(spec_ref, str):
                    source_id = spec_ref.split("#", 1)[0]
                    invariant_sources = set(
                        invariant.get("spec", {}).get("source_refs", [])
                        if isinstance(invariant.get("spec"), Mapping)
                        else []
                    )
                    oracle = invariant.get("oracle")
                    oracle_ref = oracle.get("ref") if isinstance(oracle, Mapping) else None
                    oracle_source = oracle_ref.split("#", 1)[0] if isinstance(oracle_ref, str) else None
                    if source_id not in invariant_sources and oracle_source != source_id:
                        failures.append(
                            f"{code_id}: invariant {invariant_id} does not reference {source_id}"
                        )
        for test_id in test_ids if isinstance(test_ids, list) else []:
            test = tests.get(test_id)
            if not test:
                failures.append(f"{code_id}: unknown test {test_id}")
                continue
            if not test.get("suite_tiers") or test.get("status") in {"ignored", "planned", "blocked"}:
                failures.append(f"{code_id}: test {test_id} has no executable suite route")
            linked = set(test.get("invariants", []))
            if isinstance(invariant_ids, list) and not linked.intersection(invariant_ids):
                failures.append(f"{code_id}: test {test_id} does not link a mapped invariant")
            for invariant_id in invariant_ids if isinstance(invariant_ids, list) else []:
                invariant = invariants.get(invariant_id)
                if invariant and test_id not in invariant.get("tests", []):
                    failures.append(
                        f"{code_id}: invariant {invariant_id} does not link mapped test {test_id}"
                    )
    elif disposition == "spec_gap":
        gap_id = mapping.get("spec_gap_ref")
        gap = spec_gaps.get(gap_id) if isinstance(gap_id, str) else None
        if not gap or gap.get("resolution_status") not in {
            "open",
            "decision_recorded",
            "spec_updated",
            "test_mapped",
        }:
            failures.append(f"{code_id}: spec_gap mapping requires an open owning gap")
        elif gap.get("area") != fact.area:
            failures.append(f"{code_id}: spec gap area does not match code area")
        elif code_id not in gap.get("affected_code_ids", []):
            failures.append(f"{code_id}: spec gap does not name affected code identity")
    elif disposition == "inherits_spec":
        parent_id = mapping.get("parent_code_id")
        if fact.visibility != "private":
            failures.append(f"{code_id}: only private facts may inherit a specification")
        if not isinstance(parent_id, str) or parent_id not in mappings or parent_id not in live_facts:
            failures.append(f"{code_id}: inheritance parent is not a live mapped fact")
        else:
            parent = live_facts[parent_id]
            if parent.owner != fact.owner or parent.area != fact.area:
                if not mapping.get("inheritance_review_ref"):
                    failures.append(f"{code_id}: cross-owner or cross-area inheritance requires review")
    elif disposition == "non_behavioral":
        if mapping.get("non_behavioral_reason") not in NON_BEHAVIORAL_REASONS:
            failures.append(f"{code_id}: invalid non_behavioral reason")
        if not mapping.get("rationale"):
            failures.append(f"{code_id}: non_behavioral mapping requires rationale")
        if fact.risk_signals:
            failures.append(f"{code_id}: risk-bearing fact cannot be non_behavioral")
    elif disposition == "generated":
        generator_id = mapping.get("generator_code_id")
        joined = isinstance(generator_id, str) and generator_id in live_facts
        temporary = bool(mapping.get("generator_path") and mapping.get("generator_digest"))
        if not joined and not temporary:
            failures.append(f"{code_id}: generated mapping lacks governed generator")
        if not mapping.get("generator_input_digest"):
            failures.append(f"{code_id}: generated mapping requires generator_input_digest")
        elif joined and mapping.get("generator_input_digest") != live_facts[generator_id].body_digest:
            failures.append(
                f"{code_id}: generator_input_digest does not match live generator"
            )
        if temporary:
            failures.append(f"{code_id}: pending_generator_join")
    elif disposition == "vendored_external":
        for field in ("provenance", "version_revision", "boundary_spec_ref"):
            if not mapping.get(field):
                failures.append(f"{code_id}: vendored_external requires {field}")
        boundary_ref = mapping.get("boundary_spec_ref")
        if isinstance(boundary_ref, str) and boundary_ref:
            if "#" not in boundary_ref or not boundary_ref.split("#", 1)[1]:
                failures.append(
                    f"{code_id}: vendored boundary requires exact spec fragment"
                )
            else:
                source_id, _fragment = boundary_ref.split("#", 1)
                if not _oracle_eligible(spec_sources.get(source_id)):
                    failures.append(
                        f"{code_id}: vendored boundary spec is not active and oracle-eligible"
                    )
    return failures


def _oracle_eligible(source: Mapping[str, Any] | None) -> bool:
    return bool(
        source
        and source.get("source_status") == "active"
        and source.get("oracle_eligible") is True
        and source.get("authority") != "public_claim"
    )


def _validate_inheritance_cycles(mappings: Mapping[str, Mapping[str, Any]]) -> list[str]:
    failures: list[str] = []
    for start, mapping in mappings.items():
        if mapping.get("disposition") != "inherits_spec":
            continue
        seen = {start}
        current = mapping
        while current.get("disposition") == "inherits_spec":
            parent = current.get("parent_code_id")
            if not isinstance(parent, str) or parent not in mappings:
                break
            if parent in seen:
                failures.append(f"{start}: inheritance cycle")
                break
            seen.add(parent)
            current = mappings[parent]
        if current.get("disposition") not in {"specified", "spec_gap", "inherits_spec"}:
            failures.append(f"{start}: inheritance must terminate at specified or spec_gap")
    return failures
