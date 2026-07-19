"""Linear historical redirect rules for moved or renamed catalog tests."""

from __future__ import annotations

import re
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .test_catalog_models import InferredTestFact
from .test_refactor_behavior_lock import PROPOSAL_EVIDENCE_FIELDS
from .test_refactor_identity import (
    IDENTITY_FIELDS,
    catalog_identity,
    fact_identity,
    validate_identity,
    validate_live_path,
)


REDIRECT_FIELDS = {
    "schema_version",
    "id",
    "proposal_id",
    "test_id",
    "status",
    "old_identity",
    "new_identity",
    "before_behavior_lock_evidence",
    "after_behavior_lock_evidence",
    "last_reviewed",
}
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
REDIRECT_ID_RE = re.compile(r"^TEST_REDIRECT_[A-Z0-9_]+$")
PROPOSAL_ID_RE = re.compile(r"^TEST_REFACTOR_[A-Z0-9_]+$")
TEST_ID_RE = re.compile(r"^TEST_[A-Z0-9_]+$")


def validate_redirect_contract(
    *,
    root: Path,
    redirects: Mapping[str, Mapping[str, Any]],
    proposals: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    facts: Sequence[InferredTestFact],
    failures: list[str],
) -> None:
    facts_by_id: dict[str, list[InferredTestFact]] = defaultdict(list)
    for fact in facts:
        facts_by_id[fact.stable_id].append(fact)
    for redirect_id in sorted(redirects):
        _validate_redirect(
            key=redirect_id,
            redirect=redirects[redirect_id],
            proposals=proposals,
            facts_by_id=facts_by_id,
            failures=failures,
        )
    _validate_redirect_graph(redirects, failures)
    _validate_proposal_redirect_cardinality(proposals, redirects, failures)
    _validate_redirect_endpoints(
        root=root,
        redirects=redirects,
        tests=tests,
        facts_by_id=facts_by_id,
        failures=failures,
    )


def resolve_redirect_endpoint(
    identity: dict[str, str] | None,
    test_id: Any,
    redirects: Mapping[str, Mapping[str, Any]],
) -> dict[str, str] | None:
    if identity is None or not isinstance(test_id, str):
        return identity
    by_old: dict[str, Mapping[str, Any]] = {}
    for redirect in redirects.values():
        old = redirect.get("old_identity")
        if (
            redirect.get("test_id") == test_id
            and isinstance(old, Mapping)
            and isinstance(old.get("discovery_id"), str)
        ):
            by_old.setdefault(str(old["discovery_id"]), redirect)
    current = dict(identity)
    seen: set[str] = set()
    while current["discovery_id"] in by_old:
        discovery_id = current["discovery_id"]
        if discovery_id in seen:
            break
        seen.add(discovery_id)
        redirect = by_old[discovery_id]
        if redirect.get("old_identity") != current:
            break
        target = redirect.get("new_identity")
        if not isinstance(target, Mapping) or set(target) != IDENTITY_FIELDS:
            break
        if not all(isinstance(target.get(field), str) for field in IDENTITY_FIELDS):
            break
        current = {field: str(target[field]) for field in IDENTITY_FIELDS}
    return current


def _validate_redirect(
    *,
    key: str,
    redirect: Mapping[str, Any],
    proposals: Mapping[str, Mapping[str, Any]],
    facts_by_id: Mapping[str, list[InferredTestFact]],
    failures: list[str],
) -> None:
    label = f"redirect {key}"
    _check_fields(redirect, REDIRECT_FIELDS, label, failures)
    if redirect.get("id") != key:
        failures.append(f"{label} id does not match record key")
    if not REDIRECT_ID_RE.fullmatch(str(redirect.get("id", ""))):
        failures.append(f"{label} id is invalid")
    if not PROPOSAL_ID_RE.fullmatch(str(redirect.get("proposal_id", ""))):
        failures.append(f"{label} proposal_id is invalid")
    if not TEST_ID_RE.fullmatch(str(redirect.get("test_id", ""))):
        failures.append(f"{label} test_id is invalid")
    if redirect.get("schema_version") != 1:
        failures.append(f"{label} must use schema_version 1")
    if redirect.get("status") != "active":
        failures.append(f"{label} status must equal active")
    if not isinstance(redirect.get("last_reviewed"), str) or not DATE_RE.fullmatch(
        str(redirect.get("last_reviewed", ""))
    ):
        failures.append(f"{label} last_reviewed must be YYYY-MM-DD")
    old = validate_identity(redirect.get("old_identity"), f"{label} old identity", failures)
    new = validate_identity(redirect.get("new_identity"), f"{label} new identity", failures)
    if old == new:
        failures.append(f"{label} old and new identities must differ")
    proposal_id = redirect.get("proposal_id")
    proposal = proposals.get(proposal_id) if isinstance(proposal_id, str) else None
    if proposal is None:
        failures.append(f"{label} references orphan proposal {proposal_id}")
    else:
        if proposal.get("disposition") == "no_refactor_needed":
            failures.append(f"{label} no_refactor_needed proposal cannot authorize a redirect")
        if proposal.get("status") != "validated" or "completed" not in proposal.get(
            "lifecycle", []
        ):
            failures.append(f"{label} proposal must be completed and validated")
        if proposal.get("test_id") != redirect.get("test_id"):
            failures.append(f"{label} test_id does not match proposal")
        if proposal.get("source_identity") != old:
            failures.append(f"{label} old identity does not match proposal")
        if proposal.get("target_identity") != new:
            failures.append(f"{label} new identity does not match proposal")
        for field in PROPOSAL_EVIDENCE_FIELDS:
            if proposal.get(field) != redirect.get(field):
                failures.append(f"{label} {field} does not match proposal")
    if old is not None and facts_by_id.get(old["discovery_id"]):
        failures.append(f"{label} old identity is still live")


def _validate_redirect_graph(
    redirects: Mapping[str, Mapping[str, Any]],
    failures: list[str],
) -> None:
    edges: dict[str, str] = {}
    old_counts: Counter[str] = Counter()
    new_counts: Counter[str] = Counter()
    for redirect in redirects.values():
        old = redirect.get("old_identity")
        new = redirect.get("new_identity")
        if not isinstance(old, Mapping) or not isinstance(new, Mapping):
            continue
        old_id = old.get("discovery_id")
        new_id = new.get("discovery_id")
        if not isinstance(old_id, str) or not isinstance(new_id, str):
            continue
        old_counts[old_id] += 1
        new_counts[new_id] += 1
        edges.setdefault(old_id, new_id)
    for identity, count in old_counts.items():
        if count > 1:
            failures.append(f"redirect fork from {identity} has {count} targets")
    for identity, count in new_counts.items():
        if count > 1:
            failures.append(f"redirect merge into {identity} has {count} sources")
    for start in edges:
        seen: set[str] = set()
        current = start
        while current in edges:
            if current in seen:
                failures.append(f"redirect cycle contains {current}")
                break
            seen.add(current)
            current = edges[current]


def _validate_proposal_redirect_cardinality(
    proposals: Mapping[str, Mapping[str, Any]],
    redirects: Mapping[str, Mapping[str, Any]],
    failures: list[str],
) -> None:
    counts = Counter(
        redirect.get("proposal_id")
        for redirect in redirects.values()
        if isinstance(redirect.get("proposal_id"), str)
    )
    for proposal_id, proposal in proposals.items():
        count = counts[proposal_id]
        disposition = proposal.get("disposition")
        status = proposal.get("status")
        if disposition in {"move", "rename"} and status == "validated":
            if count != 1:
                failures.append(
                    f"proposal {proposal_id} validated refactor requires exactly one redirect; found {count}"
                )
        elif count:
            failures.append(
                f"proposal {proposal_id} lifecycle does not permit {count} redirect record(s)"
            )


def _validate_redirect_endpoints(
    *,
    root: Path,
    redirects: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    facts_by_id: Mapping[str, list[InferredTestFact]],
    failures: list[str],
) -> None:
    by_old: dict[str, Mapping[str, Any]] = {}
    for redirect in redirects.values():
        old = redirect.get("old_identity")
        if isinstance(old, Mapping) and isinstance(old.get("discovery_id"), str):
            by_old.setdefault(str(old["discovery_id"]), redirect)

    for redirect_id, redirect in redirects.items():
        label = f"redirect {redirect_id}"
        new = redirect.get("new_identity")
        if not isinstance(new, Mapping) or not isinstance(new.get("discovery_id"), str):
            continue
        test_id = redirect.get("test_id")
        if not isinstance(test_id, str) or test_id not in tests:
            failures.append(f"{label} references orphan catalog test {test_id!r}")
            continue
        next_redirect = by_old.get(str(new["discovery_id"]))
        if next_redirect is not None:
            failures.append(
                f"{label} redirect chains are blocked until lock evidence IDs are proposal-scoped"
            )
            continue

        catalog = tests[test_id]
        if catalog_identity(catalog) != dict(new):
            failures.append(f"{label} catalog endpoint does not match terminal new identity")
        path = new.get("path")
        if isinstance(path, str):
            validate_live_path(root, path, f"{label} terminal endpoint", failures)
        matches = facts_by_id.get(str(new["discovery_id"]), [])
        if not matches:
            failures.append(
                f"{label} new identity is absent from current scanner facts at terminal endpoint"
            )
        elif len(matches) != 1:
            failures.append(
                f"{label} terminal identity resolves to {len(matches)} scanner facts"
            )
        elif fact_identity(matches[0]) != dict(new):
            failures.append(f"{label} terminal identity fields do not match current scanner fact")


def _check_fields(
    value: Mapping[str, Any],
    expected: set[str],
    label: str,
    failures: list[str],
) -> None:
    for field in sorted(expected - set(value)):
        failures.append(f"{label} missing required field {field}")
    for field in sorted(set(value) - expected):
        failures.append(f"{label} has additional field {field}")
