"""Adapt reviewed exact authority routes into quarantined mapping inputs."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping, Sequence
from typing import Any


_RUST_SOURCE_KINDS = frozenset({"rust_unit_test", "rust_integration_test"})
_ELIGIBLE_INVARIANT_STATUSES = frozenset(
    {"planned", "mapped", "specified", "implemented", "validated"}
)
_DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")


def build_authority_route_mapping_inputs(
    *,
    routes: Sequence[Mapping[str, Any]],
    facts: Sequence[Mapping[str, Any]],
    denominator_reviews: Sequence[Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    execution_documents: Sequence[Mapping[str, Any]],
    mapping_demands: Sequence[Mapping[str, Any]] = (),
    authenticated_inputs: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Build deterministic proposal candidates after exact focused execution."""

    facts_by_id = _unique_rows(facts, "stable_id", "scanner fact")
    reviews_by_id = _unique_rows(
        denominator_reviews,
        "discovery_id",
        "denominator review",
    )
    routes_by_id = _unique_rows(routes, "discovery_id", "authority route")
    authenticated = (
        _authenticate_route_inputs(
            routes=routes,
            mapping_demands=mapping_demands,
            authenticated_inputs=authenticated_inputs,
        )
        if authenticated_inputs is not None
        else None
    )
    executions = (
        {} if authenticated is not None else _passing_executions(execution_documents)
    )

    candidates: list[dict[str, Any]] = []
    trace_records: list[dict[str, Any]] = []
    for discovery_id in sorted(routes_by_id):
        route = routes_by_id[discovery_id]
        fact = _required_row(facts_by_id, discovery_id, "scanner fact")
        review = _required_row(reviews_by_id, discovery_id, "denominator review")
        path = _required_string(route, "path", f"route {discovery_id}")
        name = _required_string(route, "name", f"route {discovery_id}")

        source_kind = _required_string(
            fact,
            "source_kind",
            f"scanner fact {discovery_id}",
        )
        if source_kind not in _RUST_SOURCE_KINDS:
            raise ValueError(
                f"{discovery_id} has unsupported scanner source kind {source_kind}"
            )
        _require_identity(fact, discovery_id, path, name, source_kind, "scanner fact")
        if review.get("disposition") != "reviewed_nonmapping":
            raise ValueError(
                f"{discovery_id} denominator disposition must be reviewed_nonmapping"
            )
        _require_identity(
            review,
            discovery_id,
            path,
            name,
            source_kind,
            "denominator review",
        )

        invariant_id = _required_string(
            route,
            "invariant_id",
            f"route {discovery_id}",
        )
        invariant = invariants.get(invariant_id)
        if invariant is None:
            raise ValueError(f"{discovery_id} references unknown invariant {invariant_id}")
        if authenticated is not None:
            _require_specified_invariant(invariant_id, invariant)
        behavior_refs = _resolve_behavior_refs(route, invariant_id, invariant)

        if authenticated is None:
            matching_executions = executions.get((path, name), [])
            if len(matching_executions) != 1:
                raise ValueError(
                    f"{discovery_id} lacks one passing focused execution"
                )
            command, duration_ms = matching_executions[0]
            command_authority = "verified_synthesized"
            covered_code_ids: list[str] = []
            spec_gap_code_ids: list[str] = []
            spec_gap_refs: list[str] = []
            candidate_tier = "C1"
            concentration_ppm: dict[str, int] | None = None
            authority_route_digest: str | None = None
        else:
            _required_string(route, "route_source", f"route {discovery_id}")
            demand = _required_row(
                authenticated["demands"], discovery_id, "mapping demand"
            )
            trace_record = _required_row(
                authenticated["trace_records"], discovery_id, "focused trace record"
            )
            _require_identity(
                trace_record,
                discovery_id,
                path,
                name,
                source_kind,
                "focused trace record",
            )
            if trace_record.get("trace_status") != "observed" or trace_record.get(
                "exit_code"
            ) != 0:
                raise ValueError(
                    f"{discovery_id} requires one successful observed focused trace"
                )
            if demand.get("candidate_tier") != "C4":
                raise ValueError(f"{discovery_id} authority route requires a C4 demand")
            unmapped_code_ids = _string_array(
                demand.get("unmapped_code_ids"),
                f"{discovery_id} unmapped code IDs",
            )
            if unmapped_code_ids:
                raise ValueError(
                    f"{discovery_id} authority route refuses unmapped code demand"
                )
            covered_code_ids = _string_array(
                demand.get("covered_code_ids"),
                f"{discovery_id} demand covered code IDs",
            )
            trace_code_ids = _string_array(
                trace_record.get("covered_code_ids"),
                f"{discovery_id} trace covered code IDs",
            )
            if trace_code_ids != covered_code_ids:
                raise ValueError(
                    f"{discovery_id} focused trace covered code IDs differ from demand"
                )
            spec_gap_code_ids = covered_code_ids
            spec_gap_refs = _string_array(
                demand.get("spec_gap_refs"),
                f"{discovery_id} demand spec-gap refs",
            )
            if not covered_code_ids or not spec_gap_refs:
                raise ValueError(
                    f"{discovery_id} authority route requires non-empty spec-gap state"
                )
            command = _required_string(trace_record, "command", discovery_id)
            command_authority = _required_string(
                trace_record, "command_authority", discovery_id
            )
            if command_authority not in {"exact", "verified_synthesized"}:
                raise ValueError(f"{discovery_id} has invalid command authority")
            duration_ms = _nonnegative_int(
                trace_record.get("duration_ms"), f"{discovery_id} duration"
            )
            candidate_tier = "C2"
            concentration_ppm = {invariant_id: 0}
            authority_route_digest = str(authenticated["authority_route_digest"])

        candidate = {
            "candidate_invariant_ids": [invariant_id],
            "candidate_tier": candidate_tier,
            "covered_code_ids": covered_code_ids,
            "discovery_id": discovery_id,
            "exact_behavior_refs": behavior_refs,
            "mutation_kill_ids": [],
            "spec_gap_code_ids": spec_gap_code_ids,
            "spec_gap_refs": spec_gap_refs,
            "specified_code_ids": [],
        }
        if concentration_ppm is not None:
            candidate["concentration_ppm"] = concentration_ppm
        if authority_route_digest is not None:
            candidate["authority_route_digest"] = authority_route_digest
            candidate["unmapped_code_ids"] = []
        candidates.append(candidate)
        trace_records.append(
            {
                "command": command,
                "command_authority": command_authority,
                "discovery_id": discovery_id,
                "duration_ms": duration_ms,
                "trace_status": "observed",
            }
        )

    return {
        "candidates": candidates,
        "trace_records": trace_records,
        "summary": {
            "candidates": len(candidates),
            "execution_documents": len(execution_documents),
            "routes": len(routes),
        },
    }


def _authenticate_route_inputs(
    *,
    routes: Sequence[Mapping[str, Any]],
    mapping_demands: Sequence[Mapping[str, Any]],
    authenticated_inputs: Mapping[str, Any],
) -> dict[str, Any]:
    authority_document = authenticated_inputs.get("authority_document")
    focused_trace = authenticated_inputs.get("focused_trace")
    if not isinstance(authority_document, Mapping):
        raise ValueError("authenticated authority document must be an object")
    if not isinstance(focused_trace, Mapping):
        raise ValueError("authenticated focused trace must be an object")
    authority_route_digest = _required_string(
        authenticated_inputs,
        "authority_route_digest",
        "authenticated authority inputs",
    )
    focused_trace_digest = _required_string(
        authenticated_inputs,
        "focused_trace_digest",
        "authenticated authority inputs",
    )
    if _DIGEST_RE.fullmatch(authority_route_digest) is None:
        raise ValueError("authority route digest must be tagged SHA-256")
    if _DIGEST_RE.fullmatch(focused_trace_digest) is None:
        raise ValueError("focused trace digest must be tagged SHA-256")
    if _canonical_digest(authority_document) != authority_route_digest:
        raise ValueError("authority route digest mismatch")
    if _canonical_digest(focused_trace) != focused_trace_digest:
        raise ValueError("focused trace digest mismatch")
    if authority_document.get("schema_version") != 1 or authority_document.get(
        "kind"
    ) != "reviewed_test_invariant_authority_routes":
        raise ValueError("authority route document kind is unsupported")
    source_commit = _required_string(
        authority_document, "source_commit", "authority route document"
    )
    if _COMMIT_RE.fullmatch(source_commit) is None:
        raise ValueError("authority route source commit is invalid")
    if focused_trace.get("source_commit") != source_commit:
        raise ValueError("authority route source commit differs from focused trace")
    if authority_document.get("trace_digest") != focused_trace_digest:
        raise ValueError("authority route trace digest mismatch")
    document_routes = authority_document.get("routes")
    if not isinstance(document_routes, list) or any(
        not isinstance(route, Mapping) for route in document_routes
    ):
        raise ValueError("authority route document routes must be an object array")
    if list(document_routes) != list(routes):
        raise ValueError("authority route rows differ from authenticated document")

    demands_by_id = _unique_rows(
        mapping_demands, "discovery_id", "mapping demand"
    )
    routes_by_id = _unique_rows(routes, "discovery_id", "authority route")
    trace_tests = focused_trace.get("tests")
    if not isinstance(trace_tests, list) or any(
        not isinstance(record, Mapping) for record in trace_tests
    ):
        raise ValueError("focused trace tests must be an object array")
    trace_by_id = _unique_rows(trace_tests, "discovery_id", "focused trace record")
    route_ids = set(routes_by_id)
    if route_ids != set(demands_by_id) or not route_ids.issubset(trace_by_id):
        raise ValueError("authority route discovery set mismatch")
    return {
        "authority_route_digest": authority_route_digest,
        "demands": demands_by_id,
        "trace_records": trace_by_id,
    }


def _require_specified_invariant(
    invariant_id: str,
    invariant: Mapping[str, Any],
) -> None:
    if invariant.get("status") not in _ELIGIBLE_INVARIANT_STATUSES:
        raise ValueError(f"{invariant_id} lacks eligible invariant status")
    spec = invariant.get("spec")
    if not isinstance(spec, Mapping) or spec.get("status") != "specified":
        raise ValueError(f"{invariant_id} lacks specified authority")
    oracle = invariant.get("oracle")
    if (
        not isinstance(oracle, Mapping)
        or not isinstance(oracle.get("ref"), str)
        or not oracle["ref"]
    ):
        raise ValueError(f"{invariant_id} lacks an oracle reference")


def _unique_rows(
    rows: Sequence[Mapping[str, Any]],
    id_field: str,
    label: str,
) -> dict[str, Mapping[str, Any]]:
    indexed: dict[str, Mapping[str, Any]] = {}
    for index, row in enumerate(rows):
        identifier = _required_string(row, id_field, f"{label} row {index}")
        if identifier in indexed:
            raise ValueError(f"duplicate {label} identity {identifier}")
        indexed[identifier] = row
    return indexed


def _required_row(
    rows: Mapping[str, Mapping[str, Any]],
    discovery_id: str,
    label: str,
) -> Mapping[str, Any]:
    row = rows.get(discovery_id)
    if row is None:
        raise ValueError(f"{discovery_id} lacks one {label}")
    return row


def _required_string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires nonempty {field}")
    return value


def _string_array(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or any(
        not isinstance(item, str) or not item for item in value
    ):
        raise ValueError(f"{label} must be a string array")
    if value != sorted(set(value)):
        raise ValueError(f"{label} must be sorted and distinct")
    return list(value)


def _nonnegative_int(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError(f"{label} must be a nonnegative integer")
    return value


def _canonical_digest(document: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        document,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _require_identity(
    row: Mapping[str, Any],
    discovery_id: str,
    path: str,
    name: str,
    source_kind: str,
    label: str,
) -> None:
    row_id = row.get("stable_id", row.get("discovery_id"))
    row_source_kind = row.get("source_kind", row.get("discovery_source_kind"))
    expected = {
        "discovery_id": discovery_id,
        "path": path,
        "name": name,
        "source_kind": source_kind,
    }
    actual = {
        "discovery_id": row_id,
        "path": row.get("path"),
        "name": row.get("name"),
        "source_kind": row_source_kind,
    }
    if actual != expected:
        raise ValueError(f"{discovery_id} {label} identity does not match authority route")


def _resolve_behavior_refs(
    route: Mapping[str, Any],
    invariant_id: str,
    invariant: Mapping[str, Any],
) -> list[str]:
    partitions = route.get("behavior_partitions")
    if (
        not isinstance(partitions, list)
        or not partitions
        or any(not isinstance(value, str) or not value for value in partitions)
    ):
        raise ValueError(f"{route.get('discovery_id')} requires behavior_partitions")
    if len(set(partitions)) != len(partitions):
        raise ValueError(f"{route.get('discovery_id')} repeats a behavior partition")
    if "behavior_partition" not in route:
        raise ValueError(
            f"route {route.get('discovery_id')} requires behavior_partition"
        )
    primary = route["behavior_partition"]
    if primary is not None:
        if not isinstance(primary, str) or not primary:
            raise ValueError(
                f"route {route.get('discovery_id')} requires nonempty "
                "behavior_partition when provided"
            )
        if primary not in partitions:
            raise ValueError(f"{primary} is absent from behavior_partitions")

    behavior = invariant.get("behavior")
    if not isinstance(behavior, list):
        raise ValueError(f"{invariant_id} requires a behavior array")
    refs: list[str] = []
    for partition in partitions:
        matches = [
            index
            for index, row in enumerate(behavior)
            if isinstance(row, Mapping)
            and isinstance(row.get("partition"), Mapping)
            and _behavior_partition_label(row["partition"]) == partition
        ]
        if len(matches) != 1:
            raise ValueError(
                f"{partition} must resolve to exactly one behavior row"
            )
        refs.append(f"{invariant_id}#behavior:{matches[0]}")
    return refs


def _behavior_partition_label(partition: Mapping[str, Any]) -> str | None:
    if len(partition) != 1:
        return None
    predicate, value = next(iter(partition.items()))
    if not isinstance(predicate, str) or not predicate:
        return None
    if not isinstance(value, str) or not value:
        return None
    if predicate == "equals":
        return value
    return f"{predicate}_{value}"


def _passing_executions(
    documents: Sequence[Mapping[str, Any]],
) -> dict[tuple[str, str], list[tuple[str, int]]]:
    indexed: dict[tuple[str, str], list[tuple[str, int]]] = {}
    for document_index, document in enumerate(documents):
        execution = document.get("execution")
        if not isinstance(execution, Mapping):
            raise ValueError(
                f"execution document {document_index} requires execution metadata"
            )
        path = _required_string(
            execution,
            "source_path",
            f"execution document {document_index}",
        )
        command = _required_string(
            execution,
            "batch_command",
            f"execution document {document_index}",
        )
        tests = document.get("tests")
        if not isinstance(tests, list):
            raise ValueError(f"execution document {document_index} requires tests")
        for test_index, test in enumerate(tests):
            if not isinstance(test, Mapping):
                raise ValueError(
                    f"execution document {document_index} test {test_index} must be a table"
                )
            if test.get("state") != "passed":
                continue
            name = _required_string(
                test,
                "title",
                f"execution document {document_index} test {test_index}",
            )
            duration = test.get("duration")
            if (
                not isinstance(duration, int)
                or isinstance(duration, bool)
                or duration < 0
            ):
                raise ValueError(f"{path}::{name} requires nonnegative duration")
            indexed.setdefault((path, name), []).append((command, duration))
    return indexed
