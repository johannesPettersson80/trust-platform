"""Generated catalog, denominator, and invariant updates for accepted mappings."""

from __future__ import annotations

import copy
import hashlib
import json
import re
import tomllib
from collections.abc import Mapping, Sequence
from typing import Any


DATE_RE = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$")
CATALOG_FIELDS = (
    "schema_version",
    "id",
    "subject_kind",
    "discovery_id",
    "discovery_source_kind",
    "test_class",
    "area",
    "path",
    "name",
    "command",
    "owner",
    "status",
    "invariants",
    "oracle_ref",
    "expected_result",
    "expected_failure_mode",
    "suite_tiers",
    "requires_hardware",
    "requires_network",
    "duration_class",
    "evidence_destination",
    "last_reviewed",
    "mapping_proposal_id",
    "mapping_proposal_digest",
    "mapping_decision_id",
    "mapping_reviewed_source_digest",
    "mapping_assertion_review_digest",
    "mapping_behavior_refs",
    "mapping_reviewer_ids",
)
DENOMINATOR_ROOT_FIELDS = (
    "schema_version",
    "id",
    "title",
    "denominator_basis",
    "review_basis",
    "last_reviewed",
)
DENOMINATOR_REVIEW_COMMON_FIELDS = (
    "discovery_id",
    "discovery_source_kind",
    "path",
    "name",
    "ignore_state",
    "disposition",
)


def build_accepted_catalog_record(
    *,
    proposal: Mapping[str, Any],
    decision: Mapping[str, Any],
    invariants: Mapping[str, Mapping[str, Any]],
    reviewed_date: str,
) -> dict[str, Any]:
    if not DATE_RE.fullmatch(reviewed_date):
        raise ValueError("accepted catalog record requires YYYY-MM-DD review date")
    if decision.get("outcome") != "accepted":
        raise ValueError("catalog record requires an accepted decision")
    for proposal_field, decision_field in (
        ("id", "proposal_id"),
        ("proposal_digest", "proposal_digest"),
        ("discovery_id", "discovery_id"),
        ("test_source_digest", "reviewed_source_digest"),
    ):
        if proposal.get(proposal_field) != decision.get(decision_field):
            raise ValueError(
                f"accepted decision {decision_field} does not match proposal"
            )
    selected = _strings(
        decision.get("selected_invariant_ids"), "selected invariant IDs"
    )
    if not selected:
        raise ValueError("accepted decision requires selected invariants")
    oracle_refs: set[str] = set()
    for invariant_id in selected:
        invariant = invariants.get(invariant_id)
        oracle = invariant.get("oracle") if isinstance(invariant, Mapping) else None
        oracle_ref = oracle.get("ref") if isinstance(oracle, Mapping) else None
        if not isinstance(oracle_ref, str) or not oracle_ref:
            raise ValueError(f"selected invariant {invariant_id} lacks oracle ref")
        oracle_refs.add(oracle_ref)
    if len(oracle_refs) != 1:
        raise ValueError(
            "one generated catalog row requires one shared selected oracle"
        )
    confirmed = decision.get("confirmed_catalog")
    if not isinstance(confirmed, Mapping):
        raise ValueError("accepted decision lacks confirmed catalog fields")
    discovery_id = _required_string(proposal, "discovery_id", "proposal")
    suffix = discovery_id.removeprefix("DISC_")
    if not suffix or suffix == discovery_id:
        raise ValueError("proposal discovery ID is invalid")
    test_id = f"TEST_AUTO_{suffix}"
    assertion_reviews = decision.get("assertion_reviews")
    if not isinstance(assertion_reviews, Sequence) or isinstance(
        assertion_reviews, (str, bytes)
    ) or any(not isinstance(item, Mapping) for item in assertion_reviews):
        raise ValueError("reviewed assertion reviews must be an object array")
    behavior_refs = _strings(
        decision.get("selected_behavior_refs"), "selected behavior refs"
    )
    reviewers = _strings(decision.get("reviewer_ids"), "reviewer IDs")
    return {
        "schema_version": 2,
        "id": test_id,
        "subject_kind": "generated_test",
        "discovery_id": discovery_id,
        "discovery_source_kind": _required_string(
            proposal, "discovery_source_kind", "proposal"
        ),
        "test_class": _required_string(confirmed, "test_class", "confirmed catalog"),
        "area": _required_string(confirmed, "area", "confirmed catalog"),
        "path": _required_string(proposal, "path", "proposal"),
        "name": _required_string(proposal, "name", "proposal"),
        "command": _required_string(decision, "command", "accepted decision"),
        "owner": _required_string(confirmed, "owner", "confirmed catalog"),
        "status": "mapped",
        "invariants": sorted(selected),
        "oracle_ref": next(iter(oracle_refs)),
        "expected_result": _required_string(
            confirmed, "expected_result", "confirmed catalog"
        ),
        "expected_failure_mode": _required_string(
            confirmed, "expected_failure_mode", "confirmed catalog"
        ),
        "suite_tiers": sorted(
            _strings(confirmed.get("suite_tiers"), "confirmed suite tiers")
        ),
        "requires_hardware": _required_bool(
            confirmed, "requires_hardware", "confirmed catalog"
        ),
        "requires_network": _required_bool(
            confirmed, "requires_network", "confirmed catalog"
        ),
        "duration_class": _required_string(
            confirmed, "duration_class", "confirmed catalog"
        ),
        "evidence_destination": f"target/gate-artifacts/cases/{test_id}.json",
        "last_reviewed": reviewed_date,
        "mapping_proposal_id": _required_string(proposal, "id", "proposal"),
        "mapping_proposal_digest": _required_string(
            proposal, "proposal_digest", "proposal"
        ),
        "mapping_decision_id": _required_string(decision, "id", "decision"),
        "mapping_reviewed_source_digest": _required_string(
            decision, "reviewed_source_digest", "decision"
        ),
        "mapping_assertion_review_digest": _canonical_digest(assertion_reviews),
        "mapping_behavior_refs": sorted(behavior_refs),
        "mapping_reviewer_ids": sorted(reviewers),
    }


def append_catalog_record(source: str, record: Mapping[str, Any]) -> str:
    document = tomllib.loads(source)
    existing = document.get("tests")
    if not isinstance(existing, list):
        raise ValueError("catalog source must contain tests array")
    test_id = _required_string(record, "id", "catalog record")
    discovery_id = _required_string(record, "discovery_id", "catalog record")
    if any(row.get("id") == test_id for row in existing if isinstance(row, Mapping)):
        raise ValueError(f"catalog already contains {test_id}")
    if any(
        row.get("discovery_id") == discovery_id
        for row in existing
        if isinstance(row, Mapping)
    ):
        raise ValueError(f"catalog already maps {discovery_id}")
    if set(record) != set(CATALOG_FIELDS):
        raise ValueError("generated catalog record fields drift")
    rendered = re.sub(
        r"(?m)^tests\s*=\s*\[\]\s*\n?",
        "",
        source,
        count=1,
    ).rstrip()
    lines = ["", "", "[[tests]]"]
    lines.extend(f"{field} = {_toml_value(record[field])}" for field in CATALOG_FIELDS)
    result = rendered + "\n".join(lines) + "\n"
    parsed = tomllib.loads(result)
    if parsed.get("tests") != [*existing, dict(record)]:
        raise ValueError("generated catalog append failed round-trip validation")
    return result


def append_invariant_test(source: str, test_id: str) -> str:
    document = tomllib.loads(source)
    tests = document.get("tests")
    if not isinstance(tests, list) or any(not isinstance(item, str) for item in tests):
        raise ValueError("invariant must contain a string tests array")
    if test_id in tests:
        raise ValueError(f"invariant already references {test_id}")
    updated = sorted([*tests, test_id])
    match = re.search(r"(?m)^tests\s*=\s*\[", source)
    if match is None:
        raise ValueError("invariant source lacks top-level tests assignment")
    start = match.start()
    close = source.find("]", match.end())
    if close < 0:
        raise ValueError("invariant tests assignment is unterminated")
    result = source[:start] + f"tests = {_toml_value(updated)}" + source[close + 1 :]
    if tomllib.loads(result).get("tests") != updated:
        raise ValueError("invariant tests update failed round-trip validation")
    return result


def map_denominator_review(
    *,
    document: Mapping[str, Any],
    proposal: Mapping[str, Any],
    catalog_test_id: str,
    reviewed_date: str,
) -> dict[str, Any]:
    updated = copy.deepcopy(dict(document))
    reviews = updated.get("reviews")
    if not isinstance(reviews, list):
        raise ValueError("denominator document lacks reviews")
    discovery_id = _required_string(proposal, "discovery_id", "proposal")
    matches = [
        (index, row)
        for index, row in enumerate(reviews)
        if isinstance(row, Mapping) and row.get("discovery_id") == discovery_id
    ]
    if len(matches) != 1:
        raise ValueError(
            f"denominator must contain exactly one review for {discovery_id}"
        )
    index, current = matches[0]
    for field in ("discovery_source_kind", "path", "name"):
        if current.get(field) != proposal.get(field):
            raise ValueError(f"denominator {field} does not match proposal")
    reviews[index] = {
        field: current[field] for field in DENOMINATOR_REVIEW_COMMON_FIELDS[:-1]
    } | {
        "disposition": "catalog_mapped",
        "catalog_test_id": catalog_test_id,
        "last_reviewed": reviewed_date,
    }
    updated["reviews"] = sorted(
        reviews, key=lambda row: str(row.get("discovery_id", ""))
    )
    updated["last_reviewed"] = reviewed_date
    return updated


def render_denominator_document(document: Mapping[str, Any]) -> str:
    if set(document) != {
        *DENOMINATOR_ROOT_FIELDS,
        "reason_definitions",
        "reviews",
    }:
        raise ValueError("denominator root fields drift")
    lines = [
        f"{field} = {_toml_value(document[field])}"
        for field in DENOMINATOR_ROOT_FIELDS
    ]
    definitions = document.get("reason_definitions")
    if not isinstance(definitions, Mapping):
        raise ValueError("denominator reason definitions must be a table")
    lines.extend(["", "[reason_definitions]"])
    lines.extend(
        f"{key} = {_toml_value(definitions[key])}" for key in sorted(definitions)
    )
    reviews = document.get("reviews")
    if not isinstance(reviews, Sequence) or isinstance(reviews, (str, bytes)):
        raise ValueError("denominator reviews must be an array")
    for review in sorted(
        reviews,
        key=lambda row: str(row.get("discovery_id", ""))
        if isinstance(row, Mapping)
        else "",
    ):
        if not isinstance(review, Mapping):
            raise ValueError("denominator review must be a table")
        disposition = review.get("disposition")
        fields = list(DENOMINATOR_REVIEW_COMMON_FIELDS)
        fields.append(
            "catalog_test_id"
            if disposition == "catalog_mapped"
            else "reason_code"
        )
        fields.append("last_reviewed")
        if set(review) != set(fields):
            raise ValueError("denominator review fields drift")
        lines.extend(["", "[[reviews]]"])
        lines.extend(f"{field} = {_toml_value(review[field])}" for field in fields)
    rendered = "\n".join(lines) + "\n"
    if tomllib.loads(rendered) != dict(document):
        raise ValueError("denominator render failed round-trip validation")
    return rendered


def _toml_value(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        if any(not isinstance(item, str) for item in value):
            raise ValueError("generated TOML arrays must contain strings")
        return "[" + ", ".join(json.dumps(item, ensure_ascii=False) for item in value) + "]"
    raise ValueError(f"unsupported generated TOML value {value!r}")


def _canonical_digest(values: Sequence[str]) -> str:
    encoded = json.dumps(
        sorted(values), sort_keys=True, separators=(",", ":")
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _strings(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be an array")
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value) or len(result) != len(set(result)):
        raise ValueError(f"{label} must contain distinct non-empty strings")
    return result


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _required_bool(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> bool:
    value = record.get(field)
    if not isinstance(value, bool):
        raise ValueError(f"{label} requires boolean {field}")
    return value
