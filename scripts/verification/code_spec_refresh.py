"""Deterministic quarantine builder for code-to-spec refresh proposals."""

from __future__ import annotations

import hashlib
import json
import re
from collections import defaultdict
from collections.abc import Mapping, Sequence
from typing import Any

from .code_spec_common import sha256_digest, stable_code_id


CODE_ID_RE = re.compile(r"^CODE_[A-F0-9]{64}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
STALE_BODY_FINDINGS = frozenset(
    {
        "stale_spec_review body digest changed",
        "stale_spec_review risk_signals changed",
        "risk acknowledgement body digest changed",
        "risk acknowledgement risk signals changed",
    }
)
STALE_IDENTITY_FINDINGS = frozenset(
    {
        "invented_or_stale_code_identity",
        "area binding names invented code identity",
    }
)
MISSING_FINDINGS = frozenset(
    {
        "missing_spec",
        "missing_spec new fact absent from map and frozen baseline",
    }
)


class CodeSpecRefreshError(ValueError):
    """Raised when a frozen report cannot form an exact proposal partition."""


def build_refresh_proposals(
    *,
    report: Mapping[str, Any],
    source_report_path: str,
    source_report_sha256: str,
    code_map: Mapping[str, Any],
    code_map_sha256: str,
) -> dict[str, Any]:
    """Build observation-only proposals and partition every source finding."""

    facts, findings = _validate_report(report)
    mappings = _index_code_map(code_map)
    _require_digest(source_report_sha256, "source report SHA-256")
    _require_digest(code_map_sha256, "code map SHA-256")
    if not isinstance(source_report_path, str) or not source_report_path:
        raise CodeSpecRefreshError("source report path must be nonempty")

    consumed: set[str] = set()
    missing = _missing_mapping_proposals(facts, findings, consumed)
    rebinds = _digest_rebind_proposals(
        report=report,
        facts=facts,
        findings=findings,
        mappings=mappings,
        consumed=consumed,
    )
    stale = _stale_identity_proposals(
        facts=facts,
        findings=findings,
        mappings=mappings,
        consumed=consumed,
    )
    semantic = _semantic_review_groups(
        findings=findings,
        facts=facts,
        consumed=consumed,
    )
    unclassified = sorted(set(findings) - consumed)
    if unclassified:
        raise CodeSpecRefreshError(
            "unclassified finding(s): " + "; ".join(unclassified)
        )

    payload: dict[str, Any] = {
        "schema_version": 1,
        "generator": "code-spec-refresh-proposals",
        "generator_version": "1",
        "mode": "proposal_only",
        "source_observation_status": "untrusted_until_live_reconstruction",
        "source_commit": str(report["commit"]),
        "source_report": {
            "path": source_report_path,
            "sha256": source_report_sha256,
            "input_digest": str(report["input_digest"]),
        },
        "source_code_map_sha256": code_map_sha256,
        "boundaries": {
            "automatic_writes": False,
            "creates_product_proof": False,
            "closes_spec_gaps": False,
            "promotes_invariants": False,
            "infers_semantic_bindings": False,
            "source_facts_live_reconstructed": False,
        },
        "summary": {
            "source_facts": len(facts),
            "source_findings": len(findings),
            "partitioned_findings": len(consumed),
            "missing_mapping_proposals": len(missing),
            "digest_rebind_proposals": len(rebinds),
            "stale_identity_proposals": len(stale),
            "semantic_review_groups": len(semantic),
            "unclassified_findings": 0,
        },
        "missing_mapping_proposals": missing,
        "digest_rebind_proposals": rebinds,
        "stale_identity_proposals": stale,
        "semantic_review_groups": semantic,
        "unclassified_findings": unclassified,
    }
    payload["operation_set_digest"] = _digest(
        {
            "missing_mapping_proposals": missing,
            "digest_rebind_proposals": rebinds,
            "stale_identity_proposals": stale,
            "semantic_review_groups": semantic,
        }
    )
    return payload


def canonical_json(value: Mapping[str, Any]) -> str:
    """Render stable human-readable JSON for at-rest comparison."""

    return json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def validate_refresh_proposals(
    payload: Mapping[str, Any],
    *,
    source_report: Mapping[str, Any],
    source_report_path: str,
    source_report_sha256: str,
    source_code_map: Mapping[str, Any],
    source_code_map_sha256: str,
) -> list[str]:
    """Validate proposal rows against the exact frozen source report."""

    failures: list[str] = []
    try:
        source_facts, source_findings = _validate_report(source_report)
        _require_digest(source_report_sha256, "source report SHA-256")
        _require_digest(source_code_map_sha256, "source code map SHA-256")
    except CodeSpecRefreshError as error:
        return [f"source report binding is invalid: {error}"]
    missing = payload.get("missing_mapping_proposals")
    rebinds = payload.get("digest_rebind_proposals")
    stale = payload.get("stale_identity_proposals")
    semantic = payload.get("semantic_review_groups")
    unclassified = payload.get("unclassified_findings")
    summary = payload.get("summary")
    if not all(
        isinstance(rows, list)
        for rows in (missing, rebinds, stale, semantic, unclassified)
    ):
        return ["refresh proposal arrays are malformed"]
    if not isinstance(summary, Mapping):
        return ["refresh proposal summary is malformed"]
    malformed_sections = [
        label
        for label, rows in (
            ("missing_mapping_proposals", missing),
            ("digest_rebind_proposals", rebinds),
            ("stale_identity_proposals", stale),
            ("semantic_review_groups", semantic),
        )
        if any(not isinstance(row, Mapping) for row in rows)
    ]
    if malformed_sections:
        return [
            "refresh proposal rows are malformed: "
            + ", ".join(malformed_sections)
        ]

    source_metadata = payload.get("source_report")
    if not isinstance(source_metadata, Mapping):
        failures.append("refresh proposal source_report is malformed")
    else:
        if source_metadata.get("path") != source_report_path:
            failures.append("source report path binding differs")
        if source_metadata.get("sha256") != source_report_sha256:
            failures.append("source report SHA-256 binding differs")
        if source_metadata.get("input_digest") != source_report.get("input_digest"):
            failures.append("source report input_digest binding differs")
    if payload.get("source_commit") != source_report.get("commit"):
        failures.append("source report commit binding differs")
    if summary.get("source_facts") != len(source_facts):
        failures.append("summary source_facts does not match source report")
    if summary.get("source_findings") != len(source_findings):
        failures.append("summary source_findings does not match source report")

    expected_counts = {
        "missing_mapping_proposals": len(missing),
        "digest_rebind_proposals": len(rebinds),
        "stale_identity_proposals": len(stale),
        "semantic_review_groups": len(semantic),
        "unclassified_findings": len(unclassified),
    }
    for field, expected in expected_counts.items():
        if summary.get(field) != expected:
            failures.append(f"summary {field} does not match proposal rows")

    partitioned = (
        sum(len(row.get("findings", [])) for row in missing)
        + sum(len(row.get("findings", [])) for row in rebinds)
        + sum(len(row.get("findings", [])) for row in stale)
        + sum(len(row.get("affected_refs", [])) for row in semantic)
    )
    if summary.get("partitioned_findings") != partitioned:
        failures.append("summary partitioned_findings does not match proposal rows")
    if summary.get("source_findings") != partitioned + len(unclassified):
        failures.append("source findings do not form an exact proposal partition")

    proposed_findings = _proposal_findings(
        missing=missing,
        rebinds=rebinds,
        stale=stale,
        semantic=semantic,
        unclassified=unclassified,
        failures=failures,
    )
    if len(proposed_findings) != len(set(proposed_findings)):
        failures.append("proposal finding partition contains duplicate source findings")
    if sorted(proposed_findings) != list(source_findings):
        failures.append("proposal finding partition differs from exact source report")

    _validate_proposal_rows(
        rows=missing,
        id_field="proposal_id",
        id_prefix="CODEMAP_CAND",
        order_field="code_id",
        failures=failures,
    )
    _validate_proposal_rows(
        rows=rebinds,
        id_field="proposal_id",
        id_prefix="CODEMAP_CAND",
        order_field="code_id",
        failures=failures,
    )
    _validate_proposal_rows(
        rows=stale,
        id_field="proposal_id",
        id_prefix="CODEMAP_CAND",
        order_field="stale_code_id",
        failures=failures,
    )
    _validate_proposal_rows(
        rows=semantic,
        id_field="review_group_id",
        id_prefix="CODEMAP_GROUP",
        order_field="finding",
        failures=failures,
        order_key=lambda row: (str(row.get("kind", "")), str(row.get("finding", ""))),
    )

    for row in missing + rebinds:
        fact = row.get("fact")
        if not isinstance(fact, Mapping) or row.get("code_id") != fact.get("code_id"):
            failures.append(
                f"{row.get('proposal_id', '<unknown>')}: proposal/fact identity differs"
            )
    for row in missing:
        if row.get("required_reviewer_roles") != ["semantic_owner"]:
            failures.append(
                f"{row.get('proposal_id', '<unknown>')}: reviewer roles differ "
                "from missing-mapping policy"
            )
    for row in rebinds:
        findings = row.get("findings", [])
        expected_roles = ["semantic_owner"]
        if isinstance(findings, list) and any(
            str(detail).startswith("risk acknowledgement")
            for detail in findings
        ):
            expected_roles.append("risk_owner")
        if row.get("required_reviewer_roles") != expected_roles:
            failures.append(
                f"{row.get('proposal_id', '<unknown>')}: reviewer roles differ "
                "from digest-rebind policy"
            )
    for row in stale:
        candidates = row.get("exact_successor_code_ids")
        if isinstance(candidates, list) and candidates != sorted(set(candidates)):
            failures.append(
                f"{row.get('proposal_id', '<unknown>')}: successor identities are not canonical"
            )
        expected_roles = ["semantic_owner"]
        if row.get("has_owner_ack") is True:
            expected_roles.append("risk_owner")
        if row.get("required_reviewer_roles") != expected_roles:
            failures.append(
                f"{row.get('proposal_id', '<unknown>')}: reviewer roles differ "
                "from stale-identity policy"
            )
    for row in semantic:
        for field in ("affected_refs", "affected_code_ids", "owners"):
            values = row.get(field)
            if isinstance(values, list) and values != sorted(set(values)):
                failures.append(
                    f"{row.get('review_group_id', '<unknown>')}: {field} is not canonical"
                )
        expected_roles = (
            ["risk_owner", "verification_metadata_owner"]
            if row.get("kind") == "risk_ack_review"
            else ["semantic_owner", "risk_owner"]
            if row.get("kind") == "risk_signal_review" and row.get("owners")
            else ["risk_owner", "verification_metadata_owner"]
            if row.get("kind") == "risk_signal_review"
            else ["semantic_owner"]
            if row.get("owners")
            else ["verification_metadata_owner"]
        )
        if row.get("required_reviewer_roles") != expected_roles:
            failures.append(
                f"{row.get('review_group_id', '<unknown>')}: reviewer roles differ "
                "from semantic-review policy"
            )

    operations = {
        "missing_mapping_proposals": missing,
        "digest_rebind_proposals": rebinds,
        "stale_identity_proposals": stale,
        "semantic_review_groups": semantic,
    }
    if payload.get("operation_set_digest") != _digest(operations):
        failures.append("operation_set_digest does not bind proposal rows")
    try:
        source_metadata = payload.get("source_report")
        expected = build_refresh_proposals(
            report=source_report,
            source_report_path=source_report_path,
            source_report_sha256=source_report_sha256,
            code_map=source_code_map,
            code_map_sha256=source_code_map_sha256,
        )
    except CodeSpecRefreshError as error:
        failures.append(f"exact source reconstruction failed: {error}")
    else:
        if canonical_json(payload) != canonical_json(expected):
            failures.append(
                "proposal payload differs from exact source reconstruction"
            )
    return sorted(set(failures))


def _proposal_findings(
    *,
    missing: Sequence[Any],
    rebinds: Sequence[Any],
    stale: Sequence[Any],
    semantic: Sequence[Any],
    unclassified: Sequence[Any],
    failures: list[str],
) -> list[str]:
    reconstructed: list[str] = []
    for rows, subject_field in (
        (missing, "code_id"),
        (rebinds, "code_id"),
        (stale, "stale_code_id"),
    ):
        for row in rows:
            if not isinstance(row, Mapping):
                continue
            subject = row.get(subject_field)
            findings = row.get("findings")
            if not isinstance(subject, str) or not _is_string_sequence(findings):
                failures.append("proposal row cannot reconstruct source findings")
                continue
            reconstructed.extend(f"{subject}: {detail}" for detail in findings)
    for row in semantic:
        if not isinstance(row, Mapping):
            continue
        detail = row.get("finding")
        affected_refs = row.get("affected_refs")
        if not isinstance(detail, str) or not _is_string_sequence(affected_refs):
            failures.append("semantic review group cannot reconstruct source findings")
            continue
        reconstructed.extend(f"{subject}: {detail}" for subject in affected_refs)
    for finding in unclassified:
        if not isinstance(finding, str):
            failures.append("unclassified finding is not a string")
            continue
        reconstructed.append(finding)
    return reconstructed


def _validate_proposal_rows(
    *,
    rows: Sequence[Any],
    id_field: str,
    id_prefix: str,
    order_field: str,
    failures: list[str],
    order_key: Any | None = None,
) -> None:
    seen: set[str] = set()
    keys = []
    for index, row in enumerate(rows):
        if not isinstance(row, Mapping):
            failures.append(f"{id_prefix} row {index} is not an object")
            continue
        record_id = row.get(id_field)
        body = {key: value for key, value in row.items() if key != id_field}
        expected = _stable_id(id_prefix, body)
        if record_id != expected:
            failures.append(f"{record_id}: generated proposal identity differs")
        if not isinstance(record_id, str) or record_id in seen:
            failures.append(f"{id_prefix} proposal identities are not unique")
        elif isinstance(record_id, str):
            seen.add(record_id)
        keys.append(
            order_key(row)
            if order_key is not None
            else str(row.get(order_field, ""))
        )
    if keys != sorted(keys):
        failures.append(f"{id_prefix} proposal rows are not canonical")


def _validate_report(
    report: Mapping[str, Any],
) -> tuple[dict[str, Mapping[str, Any]], tuple[str, ...]]:
    if report.get("schema_version") != 1:
        raise CodeSpecRefreshError("source report schema_version must equal 1")
    if report.get("generator") != "code-spec-coverage":
        raise CodeSpecRefreshError("source report generator is not code-spec-coverage")
    if report.get("report_status") != "complete":
        raise CodeSpecRefreshError("source report must be complete")
    commit = report.get("commit")
    if not isinstance(commit, str) or COMMIT_RE.fullmatch(commit) is None:
        raise CodeSpecRefreshError("source report commit is invalid")
    _require_digest(report.get("input_digest"), "source report input_digest")
    raw_facts = report.get("facts")
    raw_findings = report.get("findings")
    raw_rebinds = report.get("digest_rebinds")
    raw_input_paths = report.get("input_paths")
    summary = report.get("summary")
    if not _is_record_sequence(raw_facts):
        raise CodeSpecRefreshError("source report facts must be an array of objects")
    if not _is_string_sequence(raw_findings):
        raise CodeSpecRefreshError("source report findings must be an array of strings")
    if not _is_record_sequence(raw_rebinds):
        raise CodeSpecRefreshError("source report digest_rebinds must be an array")
    if not _is_string_sequence(raw_input_paths):
        raise CodeSpecRefreshError("source report input_paths must be an array of strings")
    if list(raw_input_paths) != sorted(set(raw_input_paths)):
        raise CodeSpecRefreshError("source report input_paths must be canonical")
    if not isinstance(summary, Mapping):
        raise CodeSpecRefreshError("source report summary must be an object")
    if summary.get("facts_total") != len(raw_facts):
        raise CodeSpecRefreshError("source report facts_total does not match facts")
    if summary.get("findings_total") != len(raw_findings):
        raise CodeSpecRefreshError(
            "source report findings_total does not match findings"
        )
    if len(raw_findings) != len(set(raw_findings)):
        raise CodeSpecRefreshError("source report findings must be unique")

    facts: dict[str, Mapping[str, Any]] = {}
    finding_set = set(raw_findings)
    for index, fact in enumerate(raw_facts):
        code_id = fact.get("code_id")
        if not isinstance(code_id, str) or CODE_ID_RE.fullmatch(code_id) is None:
            raise CodeSpecRefreshError(f"source fact {index} has invalid code identity")
        if code_id in facts:
            raise CodeSpecRefreshError(f"duplicate live code identity {code_id}")
        if fact.get("path") not in raw_input_paths:
            raise CodeSpecRefreshError(
                f"{code_id}: reported fact path is absent from input closure"
            )
        details = fact.get("findings")
        if not _is_string_sequence(details):
            raise CodeSpecRefreshError(f"{code_id}: fact findings must be strings")
        missing = [
            detail
            for detail in details
            if f"{code_id}: {detail}" not in finding_set
        ]
        if missing:
            raise CodeSpecRefreshError(
                f"{code_id}: fact findings absent from report findings: {missing}"
            )
        _validate_fact_observation(fact, code_id)
        facts[code_id] = fact
    return dict(sorted(facts.items())), tuple(sorted(raw_findings))


def _validate_fact_observation(fact: Mapping[str, Any], code_id: str) -> None:
    for field in (
        "source_kind",
        "path",
        "qualified_symbol",
        "item_kind",
        "signature",
        "owner",
        "area",
        "visibility",
        "disposition",
    ):
        if not isinstance(fact.get(field), str) or not fact[field]:
            raise CodeSpecRefreshError(f"{code_id}: fact {field} must be nonempty")
    for field in ("signature_digest", "body_digest"):
        _require_digest(fact.get(field), f"{code_id} {field}")
    expected_signature_digest = sha256_digest(
        str(fact["signature"]).encode("utf-8")
    )
    if fact.get("signature_digest") != expected_signature_digest:
        raise CodeSpecRefreshError(
            f"{code_id}: signature digest does not bind reported signature"
        )
    expected_code_id = stable_code_id(
        source_kind=str(fact["source_kind"]),
        path=str(fact["path"]),
        qualified_symbol=str(fact["qualified_symbol"]),
        item_kind=str(fact["item_kind"]),
        signature=str(fact["signature"]),
    )
    if code_id != expected_code_id:
        raise CodeSpecRefreshError(
            f"{code_id}: identity does not derive from the reported fact tuple"
        )
    for field in ("area_candidates", "risk_signals"):
        if not _is_string_sequence(fact.get(field)):
            raise CodeSpecRefreshError(f"{code_id}: fact {field} must be strings")
    if not isinstance(fact.get("line"), int) or isinstance(fact.get("line"), bool):
        raise CodeSpecRefreshError(f"{code_id}: fact line must be an integer")


def _index_code_map(code_map: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    if code_map.get("schema_version") != 1 or set(code_map) != {
        "schema_version",
        "mappings",
    }:
        raise CodeSpecRefreshError("code map root fields drift")
    records = code_map.get("mappings")
    if not _is_record_sequence(records):
        raise CodeSpecRefreshError("code map mappings must be an array")
    indexed: dict[str, Mapping[str, Any]] = {}
    for index, mapping in enumerate(records):
        code_id = mapping.get("code_id")
        if not isinstance(code_id, str) or CODE_ID_RE.fullmatch(code_id) is None:
            raise CodeSpecRefreshError(f"code map mapping {index} has invalid code_id")
        if code_id in indexed:
            raise CodeSpecRefreshError(f"code map duplicates {code_id}")
        indexed[code_id] = mapping
    return dict(sorted(indexed.items()))


def _missing_mapping_proposals(
    facts: Mapping[str, Mapping[str, Any]],
    findings: Sequence[str],
    consumed: set[str],
) -> list[dict[str, Any]]:
    finding_set = set(findings)
    proposals = []
    for code_id, fact in facts.items():
        if fact.get("disposition") != "missing_spec":
            continue
        observed = sorted(
            detail
            for detail in MISSING_FINDINGS
            if f"{code_id}: {detail}" in finding_set
        )
        if "missing_spec" not in observed:
            raise CodeSpecRefreshError(f"{code_id}: missing mapping lacks missing_spec")
        consumed.update(f"{code_id}: {detail}" for detail in observed)
        body = {
            "kind": "missing_mapping",
            "code_id": code_id,
            "fact": _fact_observation(fact),
            "findings": observed,
            "review_status": "review_required",
            "required_reviewer_roles": ["semantic_owner"],
        }
        proposals.append(
            {
                "proposal_id": _stable_id("CODEMAP_CAND", body),
                **body,
            }
        )
    return proposals


def _digest_rebind_proposals(
    *,
    report: Mapping[str, Any],
    facts: Mapping[str, Mapping[str, Any]],
    findings: Sequence[str],
    mappings: Mapping[str, Mapping[str, Any]],
    consumed: set[str],
) -> list[dict[str, Any]]:
    finding_set = set(findings)
    proposals = []
    seen: set[str] = set()
    for rebind in sorted(
        report["digest_rebinds"],
        key=lambda row: str(row.get("code_id", "")),
    ):
        code_id = rebind.get("code_id")
        if not isinstance(code_id, str) or code_id not in facts:
            raise CodeSpecRefreshError(f"digest rebind names non-live code identity {code_id}")
        if code_id in seen:
            raise CodeSpecRefreshError(f"digest rebind duplicates {code_id}")
        seen.add(code_id)
        fact = facts[code_id]
        mapping = mappings.get(code_id)
        if mapping is None:
            raise CodeSpecRefreshError(f"{code_id}: digest rebind lacks existing mapping")
        old_digest = rebind.get("old_digest")
        new_digest = rebind.get("new_digest")
        _require_digest(old_digest, f"{code_id} old digest")
        _require_digest(new_digest, f"{code_id} new digest")
        if new_digest != fact.get("body_digest"):
            raise CodeSpecRefreshError(f"{code_id}: digest rebind live digest differs")
        if old_digest != mapping.get("reviewed_body_digest"):
            raise CodeSpecRefreshError(f"{code_id}: digest rebind old digest differs")
        details = sorted(
            detail
            for detail in STALE_BODY_FINDINGS
            if f"{code_id}: {detail}" in finding_set
        )
        if "stale_spec_review body digest changed" not in details:
            raise CodeSpecRefreshError(f"{code_id}: digest rebind lacks stale finding")
        consumed.update(f"{code_id}: {detail}" for detail in details)
        roles = ["semantic_owner"]
        if any(detail.startswith("risk acknowledgement") for detail in details):
            roles.append("risk_owner")
        body = {
            "kind": "body_digest_rebind",
            "code_id": code_id,
            "fact": _fact_observation(fact),
            "existing_mapping_sha256": _digest(mapping),
            "existing_disposition": str(mapping.get("disposition", "")),
            "old_body_digest": old_digest,
            "new_body_digest": new_digest,
            "findings": details,
            "review_status": "review_required",
            "required_reviewer_roles": roles,
        }
        proposals.append(
            {
                "proposal_id": _stable_id("CODEMAP_CAND", body),
                **body,
            }
        )
    return proposals


def _stale_identity_proposals(
    *,
    facts: Mapping[str, Mapping[str, Any]],
    findings: Sequence[str],
    mappings: Mapping[str, Mapping[str, Any]],
    consumed: set[str],
) -> list[dict[str, Any]]:
    by_subject = _findings_by_subject(findings)
    proposals = []
    for code_id in sorted(set(mappings) - set(facts)):
        details = sorted(set(by_subject.get(code_id, ())) & STALE_IDENTITY_FINDINGS)
        if not details:
            continue
        consumed.update(f"{code_id}: {detail}" for detail in details)
        mapping = mappings[code_id]
        candidates = [
            live_id
            for live_id, fact in facts.items()
            if (
                fact.get("path"),
                fact.get("qualified_symbol"),
                fact.get("item_kind"),
            )
            == (
                mapping.get("path"),
                mapping.get("qualified_symbol"),
                mapping.get("item_kind"),
            )
        ]
        roles = ["semantic_owner"]
        if mapping.get("owner_ack") is not None:
            roles.append("risk_owner")
        body = {
            "kind": "identity_successor_review",
            "stale_code_id": code_id,
            "existing_mapping_sha256": _digest(mapping),
            "path": str(mapping.get("path", "")),
            "qualified_symbol": str(mapping.get("qualified_symbol", "")),
            "item_kind": str(mapping.get("item_kind", "")),
            "candidate_match_basis": "exact_path_qualified_symbol_item_kind",
            "exact_successor_code_ids": sorted(candidates),
            "has_owner_ack": mapping.get("owner_ack") is not None,
            "findings": details,
            "review_status": "review_required",
            "required_reviewer_roles": roles,
        }
        proposals.append(
            {
                "proposal_id": _stable_id("CODEMAP_CAND", body),
                **body,
            }
        )
    return proposals


def _semantic_review_groups(
    *,
    findings: Sequence[str],
    facts: Mapping[str, Mapping[str, Any]],
    consumed: set[str],
) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str], list[str]] = defaultdict(list)
    for finding in findings:
        if finding in consumed:
            continue
        subject, detail = _split_finding(finding)
        kind = _semantic_kind(detail)
        if kind is None:
            continue
        grouped[(kind, detail)].append(subject)
        consumed.add(finding)
    rows = []
    for (kind, detail), subjects in sorted(grouped.items()):
        affected_refs = sorted(set(subjects))
        affected_code_ids = sorted(
            subject for subject in affected_refs if subject in facts
        )
        owners = sorted(
            {
                str(facts[code_id]["owner"])
                for code_id in affected_code_ids
            }
        )
        body = {
            "kind": kind,
            "finding": detail,
            "affected_refs": affected_refs,
            "affected_code_ids": affected_code_ids,
            "owners": owners,
            "review_status": "review_required",
            "required_reviewer_roles": (
                ["risk_owner", "verification_metadata_owner"]
                if kind == "risk_ack_review"
                else ["semantic_owner", "risk_owner"]
                if kind == "risk_signal_review" and owners
                else ["risk_owner", "verification_metadata_owner"]
                if kind == "risk_signal_review"
                else ["semantic_owner"]
                if owners
                else ["verification_metadata_owner"]
            ),
        }
        rows.append(
            {
                "review_group_id": _stable_id("CODEMAP_GROUP", body),
                **body,
            }
        )
    return rows


def _semantic_kind(detail: str) -> str | None:
    if detail == "specified spec_ref fragment does not exist":
        return "spec_fragment_rebind"
    if detail == "unmapped":
        return "unmapped_fact"
    if detail in {
        "stale_spec_review risk_signals changed",
        "risk acknowledgement risk signals changed",
    }:
        return "risk_signal_review"
    if re.fullmatch(r"invariant [A-Z][A-Z0-9_]+ crosses area without decision", detail):
        return "cross_area_decision"
    if re.fullmatch(
        (
            r"invariant [A-Z][A-Z0-9_]+ does not link "
            r"(?:any mapped test|mapped test [A-Z][A-Z0-9_]+)"
        ),
        detail,
    ):
        return "invariant_test_relation"
    if re.fullmatch(
        r"test [A-Z][A-Z0-9_]+ does not link a mapped invariant",
        detail,
    ):
        return "test_invariant_relation"
    if detail in {
        "risk acknowledgement body digest changed",
        "risk acknowledgement names invented or stale code identity",
        "acknowledgement names a non-risk code fact",
    }:
        return "risk_ack_review"
    return None


def _fact_observation(fact: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "code_id": str(fact["code_id"]),
        "source_kind": str(fact["source_kind"]),
        "path": str(fact["path"]),
        "qualified_symbol": str(fact["qualified_symbol"]),
        "item_kind": str(fact["item_kind"]),
        "signature_digest": str(fact["signature_digest"]),
        "body_digest": str(fact["body_digest"]),
        "owner": str(fact["owner"]),
        "area": str(fact["area"]),
        "area_candidates": sorted(set(fact["area_candidates"])),
        "visibility": str(fact["visibility"]),
        "risk_signals": sorted(set(fact["risk_signals"])),
        "line": int(fact["line"]),
        "observed_disposition": str(fact["disposition"]),
    }


def _findings_by_subject(findings: Sequence[str]) -> dict[str, list[str]]:
    grouped: dict[str, list[str]] = defaultdict(list)
    for finding in findings:
        subject, detail = _split_finding(finding)
        grouped[subject].append(detail)
    return grouped


def _split_finding(finding: str) -> tuple[str, str]:
    subject, separator, detail = finding.partition(": ")
    if not separator or not subject or not detail:
        raise CodeSpecRefreshError(f"malformed source finding {finding!r}")
    return subject, detail


def _stable_id(prefix: str, value: Mapping[str, Any]) -> str:
    return f"{prefix}_{_digest_hex(value).upper()}"


def _digest(value: Mapping[str, Any]) -> str:
    return f"sha256:{_digest_hex(value)}"


def _digest_hex(value: Mapping[str, Any]) -> str:
    encoded = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _require_digest(value: Any, label: str) -> None:
    if not isinstance(value, str) or DIGEST_RE.fullmatch(value) is None:
        raise CodeSpecRefreshError(f"{label} must be a SHA-256 digest")


def _is_record_sequence(value: Any) -> bool:
    return (
        isinstance(value, Sequence)
        and not isinstance(value, (str, bytes))
        and all(isinstance(item, Mapping) for item in value)
    )


def _is_string_sequence(value: Any) -> bool:
    return (
        isinstance(value, Sequence)
        and not isinstance(value, (str, bytes))
        and all(isinstance(item, str) and item for item in value)
    )
