"""Focused per-test execution trace contracts and orchestration helpers."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any


def validate_trace_document(
    document: Mapping[str, Any],
    *,
    live_facts: Mapping[str, Mapping[str, Any]],
    allow_full_campaign: bool = False,
) -> list[str]:
    """Validate trace semantics beyond the closed JSON schema."""

    failures: list[str] = []
    scope = document.get("scope")
    if not isinstance(scope, Mapping):
        return ["trace requires scope object"]
    mode = scope.get("mode")
    if mode == "full_campaign" and not allow_full_campaign:
        failures.append("full campaign is not authorized before the frozen final candidate")
    requested = _string_list(scope.get("requested_discovery_ids"))
    code_inventory_digests = document.get("code_inventory_digests")
    if not isinstance(code_inventory_digests, Mapping):
        failures.append("trace requires code_inventory_digests object")
    unknown_requested = sorted(set(requested) - set(live_facts))
    if unknown_requested:
        failures.append(
            f"requested discovery IDs are absent from live facts: {unknown_requested}"
        )
    tests = document.get("tests")
    if not isinstance(tests, Sequence) or isinstance(tests, (str, bytes)):
        return sorted(set([*failures, "trace tests must be an array"]))
    seen: set[str] = set()
    for index, record in enumerate(tests):
        if not isinstance(record, Mapping):
            failures.append(f"trace test[{index}] must be an object")
            continue
        discovery_id = record.get("discovery_id")
        label = (
            discovery_id
            if isinstance(discovery_id, str) and discovery_id
            else f"trace test[{index}]"
        )
        if not isinstance(discovery_id, str) or not discovery_id:
            failures.append(f"{label} requires discovery_id")
            continue
        if discovery_id in seen:
            failures.append(f"trace duplicates discovery ID {discovery_id}")
        seen.add(discovery_id)
        fact = live_facts.get(discovery_id)
        if fact is None:
            failures.append(f"{label} is absent from live facts")
        else:
            for field in ("discovery_source_kind", "path", "name"):
                if record.get(field) != fact.get(field):
                    failures.append(f"{label} {field} does not match live fact")
        authority = record.get("command_authority")
        status = record.get("trace_status")
        covered_code_ids = _string_list(record.get("covered_code_ids"))
        if status == "observed":
            if authority not in {"exact", "verified_synthesized"}:
                failures.append(f"{label} requires an exact or verified command")
            if record.get("exit_code") != 0:
                failures.append(f"{label} observed record requires exit_code 0")
            for field in ("attempt_id", "profile_digest"):
                value = record.get(field)
                if not isinstance(value, str) or not value:
                    failures.append(f"{label} observed record requires {field}")
            if record.get("attribution_kind") in {"unavailable", "structural_only"}:
                failures.append(
                    f"{label} observed record requires execution attribution kind"
                )
            if (
                record.get("attribution_kind") == "llvm_source_coverage"
                and (
                    not isinstance(code_inventory_digests, Mapping)
                    or not any(
                        key in code_inventory_digests
                        for key in (
                            "rust_product",
                            "rust_focused_projection",
                            "rust_observed_paths",
                        )
                    )
                )
            ):
                failures.append(
                    f"{label} LLVM attribution requires Rust inventory digest"
                )
            if (
                record.get("attribution_kind") == "istanbul_source_coverage"
                and (
                    not isinstance(code_inventory_digests, Mapping)
                    or "typescript_product" not in code_inventory_digests
                )
            ):
                failures.append(
                    f"{label} Istanbul attribution requires typescript_product inventory digest"
                )
        elif status == "failed":
            if authority not in {"exact", "verified_synthesized"}:
                failures.append(
                    f"{label} failed record requires an exact or verified command"
                )
            exit_code = record.get("exit_code")
            if (
                not isinstance(exit_code, int)
                or isinstance(exit_code, bool)
                or exit_code <= 0
            ):
                failures.append(
                    f"{label} failed record requires positive nonzero exit_code"
                )
            for field in ("attempt_id", "profile_digest"):
                value = record.get(field)
                if not isinstance(value, str) or not value:
                    failures.append(f"{label} failed record requires {field}")
            duration_ms = record.get("duration_ms")
            if (
                not isinstance(duration_ms, int)
                or isinstance(duration_ms, bool)
                or duration_ms < 0
            ):
                failures.append(
                    f"{label} failed record requires nonnegative duration_ms"
                )
            if record.get("attribution_kind") != "unavailable":
                failures.append(
                    f"{label} failed record requires unavailable attribution"
                )
            if covered_code_ids:
                failures.append(
                    f"{label} failed record cannot claim covered code"
                )
        else:
            if authority not in {
                "exact",
                "verified_synthesized",
                "unavailable",
            }:
                failures.append(f"{label} has invalid non-observed command authority")
            if covered_code_ids:
                failures.append(f"{label} non-observed record cannot claim covered code")
    missing_records = sorted(set(requested) - seen)
    extra_records = sorted(seen - set(requested))
    if missing_records or extra_records:
        failures.append(
            "requested discovery IDs do not exactly match trace records: "
            f"missing={missing_records}, extra={extra_records}"
        )
    return sorted(set(failures))


def _string_list(value: Any) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        return []
    return [item for item in value if isinstance(item, str) and item]
