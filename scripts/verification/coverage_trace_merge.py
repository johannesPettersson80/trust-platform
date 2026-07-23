"""Deterministic merge of disjoint per-source execution trace shards."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from typing import Any

from .coverage_trace import validate_trace_document


def merge_trace_documents(
    documents: Sequence[Mapping[str, Any]],
    *,
    live_facts: Mapping[str, Mapping[str, Any]],
    mode: str,
    require_complete_denominator: bool,
) -> dict[str, Any]:
    if not documents:
        raise ValueError("trace merge requires at least one document")
    if mode not in {"focused_shard", "full_campaign"}:
        raise ValueError(f"unsupported merged trace mode {mode!r}")
    source_commits = {
        _required_string(document, "source_commit", "trace shard")
        for document in documents
    }
    platforms = {
        _required_string(document, "platform", "trace shard")
        for document in documents
    }
    if len(source_commits) != 1:
        raise ValueError(f"trace shard source commits differ: {sorted(source_commits)}")
    if len(platforms) != 1:
        raise ValueError(f"trace shard platforms differ: {sorted(platforms)}")
    inventory_digests: dict[str, str] = {}
    tests: dict[str, Mapping[str, Any]] = {}
    input_digests: list[str] = []
    for index, document in enumerate(documents):
        shard_failures = validate_trace_document(
            document,
            live_facts=live_facts,
            allow_full_campaign=True,
        )
        if shard_failures:
            raise ValueError(
                f"trace shard[{index}] is invalid: {'; '.join(shard_failures)}"
            )
        input_digests.append(
            _required_string(document, "input_digest", f"trace shard[{index}]")
        )
        configured = document.get("code_inventory_digests")
        if not isinstance(configured, Mapping):
            raise ValueError(f"trace shard[{index}] inventory digests must be an object")
        for name, digest in configured.items():
            if not isinstance(name, str) or not name:
                raise ValueError("trace inventory names must be non-empty strings")
            if not isinstance(digest, str) or not digest:
                raise ValueError(f"trace inventory {name} requires a digest")
            previous = inventory_digests.get(name)
            if previous is not None and previous != digest:
                raise ValueError(f"trace inventory digest conflicts for {name}")
            inventory_digests[name] = digest
        shard_tests = document.get("tests")
        if not isinstance(shard_tests, Sequence) or isinstance(
            shard_tests, (str, bytes)
        ):
            raise ValueError(f"trace shard[{index}] tests must be an array")
        for record in shard_tests:
            if not isinstance(record, Mapping):
                raise ValueError(f"trace shard[{index}] contains non-object test")
            discovery_id = _required_string(record, "discovery_id", "trace record")
            if discovery_id in tests:
                raise ValueError(f"trace shards overlap discovery ID {discovery_id}")
            tests[discovery_id] = record
    if require_complete_denominator and set(tests) != set(live_facts):
        raise ValueError(
            "merged trace does not cover the complete live denominator: "
            f"missing={sorted(set(live_facts) - set(tests))}, "
            f"extra={sorted(set(tests) - set(live_facts))}"
        )
    merged = {
        "schema_version": 1,
        "generator": "test-coverage-trace",
        "generator_version": "1",
        "source_commit": next(iter(source_commits)),
        "platform": next(iter(platforms)),
        "scope": {
            "mode": mode,
            "requested_discovery_ids": sorted(tests),
            "source_kinds": sorted(
                {
                    _required_string(
                        record, "discovery_source_kind", discovery_id
                    )
                    for discovery_id, record in tests.items()
                }
            ),
        },
        "input_digest": _merged_input_digest(input_digests),
        "code_inventory_digests": dict(sorted(inventory_digests.items())),
        "tests": [dict(tests[discovery_id]) for discovery_id in sorted(tests)],
    }
    failures = validate_trace_document(
        merged,
        live_facts=live_facts,
        allow_full_campaign=mode == "full_campaign",
    )
    if failures:
        raise ValueError("; ".join(failures))
    return merged


def _merged_input_digest(digests: Sequence[str]) -> str:
    encoded = json.dumps(
        sorted(digests),
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _required_string(
    record: Mapping[str, Any],
    field: str,
    label: str,
) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value
