"""Proof and TOML-document helpers for case-backed behavior mapping."""

from __future__ import annotations

import json
import re
import tomllib
from collections import defaultdict
from collections.abc import Mapping, Sequence
from typing import Any

from .proof_contract import proof_contract_digest


LEGACY_CURRENT_PROOF_CONTRACT_VERSION = "execution_contract_v2"


def replace_table_record_fields(
    source: str,
    *,
    table: str,
    updates: Mapping[str, Mapping[str, Any]],
) -> str:
    """Replace scalar or string-array fields without reformatting whole records."""

    if not updates:
        return source
    document = tomllib.loads(source)
    records = document.get(table)
    if not isinstance(records, list) or any(
        not isinstance(record, Mapping) for record in records
    ):
        raise ValueError(f"source must contain [[{table}]] records")
    indices: dict[str, list[int]] = defaultdict(list)
    for index, record in enumerate(records):
        record_id = record.get("id")
        if isinstance(record_id, str):
            indices[record_id].append(index)
    for record_id in updates:
        if len(indices.get(record_id, [])) != 1:
            raise ValueError(
                f"[[{table}]] must contain exactly one {record_id}"
            )

    header = re.compile(rf"(?m)^\[\[{re.escape(table)}\]\]\s*$")
    matches = list(header.finditer(source))
    if len(matches) != len(records):
        raise ValueError(f"[[{table}]] source/header count drifted")
    result = source
    update_indices = sorted(
        (indices[record_id][0] for record_id in updates),
        reverse=True,
    )
    record_id_by_index = {
        indices[record_id][0]: record_id for record_id in updates
    }
    for index in update_indices:
        start = matches[index].start()
        end = matches[index + 1].start() if index + 1 < len(matches) else len(result)
        block = result[start:end]
        record_id = record_id_by_index[index]
        for field, value in updates[record_id].items():
            rendered = f"{field} = {_toml_value(value)}"
            pattern = re.compile(rf"(?m)^{re.escape(field)}\s*=.*$")
            field_matches = list(pattern.finditer(block))
            if len(field_matches) > 1:
                raise ValueError(
                    f"{record_id} duplicates field {field}"
                )
            if field_matches:
                block = pattern.sub(rendered, block, count=1)
                continue
            anchor = re.search(r"(?m)^last_reviewed\s*=.*$", block)
            if anchor is None:
                block = block.rstrip() + "\n" + rendered + "\n\n"
            else:
                block = (
                    block[: anchor.start()]
                    + rendered
                    + "\n"
                    + block[anchor.start() :]
                )
        result = result[:start] + block + result[end:]

    parsed = tomllib.loads(result)
    updated_records = parsed.get(table)
    assert isinstance(updated_records, list)
    for record_id, field_updates in updates.items():
        index = indices[record_id][0]
        expected = dict(records[index])
        expected.update(field_updates)
        if updated_records[index] != expected:
            raise ValueError(
                f"[[{table}]] {record_id} update failed round-trip validation"
            )
    for index, record in enumerate(records):
        if index not in update_indices and updated_records[index] != record:
            raise ValueError(f"[[{table}]] unrelated record changed")
    return result


def select_reused_evidence(
    *,
    evidence_records: Sequence[Mapping[str, Any]],
    assignment_by_test: Mapping[str, Mapping[str, Any]],
    cases_by_test: Mapping[str, set[str]],
) -> list[dict[str, Any]]:
    """Select one current-case passing proof record for each existing runner."""

    selected: list[dict[str, Any]] = []
    for test_id in sorted(assignment_by_test):
        assignment = assignment_by_test[test_id]
        required_cases = cases_by_test[test_id]
        candidates = []
        for evidence in evidence_records:
            if test_id not in evidence.get("linked_tests", []):
                continue
            summaries = _case_summary(evidence.get("per_case_summary", []))
            if (
                evidence.get("command_exit_status") == 0
                and evidence.get("case_file_digest")
                == assignment["case_file_digest"]
                and all(
                    summaries.get(case_id) == "passed"
                    for case_id in required_cases
                )
            ):
                candidates.append((evidence, summaries))
        if not candidates:
            raise ValueError(
                f"{test_id} lacks current passing evidence for every assigned case"
            )
        evidence, summaries = sorted(
            candidates,
            key=lambda item: (
                _evidence_priority(item[0]),
                str(item[0].get("id", "")),
            ),
        )[0]
        selected.append(
            {
                "catalog_test_id": test_id,
                "evidence_id": _string(evidence, "id", "execution evidence"),
                "proof_kind": evidence.get("proof_kind"),
                "command": evidence.get("command"),
                "command_exit_status": evidence.get("command_exit_status"),
                "commit": evidence.get("commit"),
                "platform": evidence.get("platform"),
                "case_file_digest": evidence.get("case_file_digest"),
                "passed_case_ids": sorted(
                    case_id
                    for case_id, result in summaries.items()
                    if result == "passed"
                ),
            }
        )
    return selected


def proof_digest_updates(
    *,
    evidence_records: Sequence[Mapping[str, Any]],
    selected_test_ids: set[str],
    tests_by_id: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
) -> list[dict[str, str]]:
    """Compute current-binding proof digests after catalog behavior mapping."""

    updates: list[dict[str, str]] = []
    for evidence in evidence_records:
        binding = evidence.get("proof_contract_binding")
        if binding not in (None, "current"):
            continue
        if binding is None and (
            evidence.get("proof_contract_version")
            != LEGACY_CURRENT_PROOF_CONTRACT_VERSION
            or not isinstance(evidence.get("proof_contract_digest"), str)
        ):
            continue
        selected = selected_test_ids.intersection(
            _strings(evidence.get("linked_tests", []), "evidence linked tests")
        )
        if not selected:
            continue
        if len(selected) != 1:
            raise ValueError(
                f"{evidence.get('id')} links multiple selected proof contracts"
            )
        test_id = next(iter(selected))
        digest = proof_contract_digest(
            test=tests_by_id[test_id],
            invariants=invariants,
        )
        updates.append(
            {
                "evidence_id": _string(evidence, "id", "evidence record"),
                "catalog_test_id": test_id,
                "proof_contract_digest": digest,
            }
        )
    updates.sort(key=lambda row: row["evidence_id"])
    return updates


def _evidence_priority(evidence: Mapping[str, Any]) -> int:
    proof_kind = evidence.get("proof_kind")
    if proof_kind == "lock_compare":
        return 0
    if proof_kind == "green":
        return 1
    if proof_kind == "lock_baseline":
        return 2
    return 3


def _case_summary(value: Any) -> dict[str, str]:
    rows = _strings(value, "per-case summary")
    result: dict[str, str] = {}
    for row in rows:
        case_id, separator, status = row.rpartition(":")
        if not separator or not case_id or not status or case_id in result:
            raise ValueError("per-case summary is invalid")
        result[case_id] = status
    return result


def _strings(value: Any, label: str) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ValueError(f"{label} must be a string array")
    result = [item for item in value if isinstance(item, str) and item]
    if len(result) != len(value) or len(result) != len(set(result)):
        raise ValueError(f"{label} must contain distinct non-empty strings")
    return result


def _string(record: Mapping[str, Any], field: str, label: str) -> str:
    value = record.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value


def _toml_value(value: Any) -> str:
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        strings = _strings(value, "generated TOML array")
        return "[" + ", ".join(
            json.dumps(item, ensure_ascii=False) for item in strings
        ) + "]"
    raise ValueError("record field updates support strings and string arrays")
