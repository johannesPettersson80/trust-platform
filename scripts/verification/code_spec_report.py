"""Canonical Phase 17 code-to-spec report model."""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from dataclasses import dataclass
from typing import Any, Mapping

from .code_spec_common import CodeFact


@dataclass(frozen=True)
class CodeSpecProvenance:
    commit: str
    timestamp: str
    platform: str
    input_paths: tuple[str, ...]
    output_json: str
    output_markdown: str


@dataclass(frozen=True)
class CodeSpecDenominator:
    source_counts: Mapping[str, int]
    cargo_target_counts: Mapping[str, int]


def build_report_payload(
    *,
    facts: Mapping[str, CodeFact],
    mappings: Mapping[str, Mapping[str, Any]],
    failures: tuple[str, ...],
    provenance: CodeSpecProvenance,
    input_digest: str,
    debt_baseline: Mapping[str, Any] | None = None,
    cargo_bindings: Mapping[str, tuple[str, ...]] | None = None,
    denominator: CodeSpecDenominator | None = None,
    inventory_bindings: Mapping[str, tuple[str, ...]] | None = None,
    enforcement_mode: str = "report",
    blocking_findings: tuple[str, ...] = (),
) -> dict[str, Any]:
    debt_baseline = debt_baseline or {"entries": []}
    cargo_bindings = cargo_bindings or {}
    denominator = denominator or CodeSpecDenominator({}, {})
    inventory_bindings = inventory_bindings or {}
    baseline_entries = debt_baseline.get("entries", [])
    baseline_by_id = {
        str(entry["code_id"]): entry
        for entry in baseline_entries
        if isinstance(entry, Mapping) and isinstance(entry.get("code_id"), str)
    }
    findings_by_code: dict[str, list[str]] = {}
    finding_kinds: Counter[str] = Counter()
    for failure in sorted(set(failures)):
        code_id, separator, detail = failure.partition(": ")
        if not separator:
            code_id, detail = "global", failure
        findings_by_code.setdefault(code_id, []).append(detail)
        finding_kinds[detail.split(maxsplit=1)[0]] += 1
    rows = []
    digest_rebinds = []
    crate_names: list[str] = []
    cargo_target_names: list[str] = []
    for code_id, code_fact in sorted(facts.items()):
        mapping = mappings.get(code_id)
        disposition = mapping.get("disposition") if mapping else "missing_spec"
        fact_bindings = tuple(sorted(set(cargo_bindings.get(code_id, ()))))
        crate_names.extend(
            sorted({binding.split(":", 1)[0] for binding in fact_bindings})
        )
        cargo_target_names.extend(fact_bindings)
        rows.append(
            {
                "code_id": code_id,
                "source_kind": code_fact.source_kind,
                "path": code_fact.path,
                "qualified_symbol": code_fact.qualified_symbol,
                "item_kind": code_fact.item_kind,
                "signature": code_fact.signature,
                "signature_digest": code_fact.signature_digest,
                "body_digest": code_fact.body_digest,
                "owner": code_fact.owner,
                "area": code_fact.area,
                "visibility": code_fact.visibility,
                "risk_signals": list(code_fact.risk_signals),
                "cargo_bindings": list(fact_bindings),
                "inventory_bindings": list(
                    sorted(set(inventory_bindings.get(code_id, ())))
                ),
                "line": code_fact.line,
                "disposition": disposition,
                "disposition_ref": _disposition_ref(mapping),
                "debt_status": "historical_debt" if code_id in baseline_by_id else "none",
                "findings": findings_by_code.get(code_id, []),
            }
        )
        reviewed = mapping.get("reviewed_body_digest") if mapping else None
        if isinstance(reviewed, str) and reviewed != code_fact.body_digest:
            digest_rebinds.append(
                {
                    "code_id": code_id,
                    "old_digest": reviewed,
                    "new_digest": code_fact.body_digest,
                    "owner": code_fact.owner,
                    "area": code_fact.area,
                    "owner_ack": mapping.get("owner_ack"),
                }
            )
    return {
        "schema_version": 1,
        "generator": "code-spec-coverage",
        "generator_version": 1,
        "report_status": "complete",
        "commit": provenance.commit,
        "timestamp": provenance.timestamp,
        "platform": provenance.platform,
        "input_paths": list(provenance.input_paths),
        "input_digest": input_digest,
        "output_paths": {
            "json": provenance.output_json,
            "markdown": provenance.output_markdown,
        },
        "boundaries": {
            "proof_kind": "none",
            "creates_product_proof": False,
            "closes_spec_gaps": False,
            "promotes_invariants": False,
            "infers_expected_behavior": False,
        },
        "enforcement": {
            "mode": enforcement_mode,
            "blocking_total": len(set(blocking_findings)),
            "blocking_findings": sorted(set(blocking_findings)),
        },
        "denominator": {
            "source_counts": _counts_from_mapping(denominator.source_counts),
            "cargo_target_counts": _counts_from_mapping(
                denominator.cargo_target_counts
            ),
        },
        "summary": {
            "facts_total": len(rows),
            "mapped_total": sum(row["disposition"] != "missing_spec" for row in rows),
            "missing_spec": sum(row["disposition"] == "missing_spec" for row in rows),
            "historical_debt": len(baseline_by_id),
            "new_missing_spec": sum(
                row["disposition"] == "missing_spec" and row["debt_status"] == "none"
                for row in rows
            ),
            "stale_spec_review": finding_kinds["stale_spec_review"],
            "spec_gap": finding_kinds["spec_gap"],
            "missing_tests": finding_kinds["missing_tests"],
            "unmapped": finding_kinds["unmapped"],
            "findings_total": sum(finding_kinds.values()),
            "by_area": _counts(row["area"] for row in rows),
            "by_owner": _counts(row["owner"] for row in rows),
            "by_disposition": _counts(row["disposition"] for row in rows),
            "by_risk_signal": _counts(signal for row in rows for signal in row["risk_signals"]),
            "by_crate": _counts(crate_names),
            "by_cargo_target": _counts(cargo_target_names),
        },
        "digest_rebinds": digest_rebinds,
        "debt_baseline": list(baseline_entries),
        "findings": list(sorted(set(failures))),
        "facts": rows,
        "limitations": [
            "This report maps code to reviewed metadata and emits proof_kind none.",
            "Missing or ambiguous expected behavior remains a finding; implementation text is never an oracle.",
            "Static mapping does not close a specification gap or promote an invariant.",
        ],
    }


def canonical_json(payload: Mapping[str, Any]) -> str:
    return json.dumps(payload, indent=2, sort_keys=True) + "\n"


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    summary = payload["summary"]
    lines = [
        "# Production Code-to-Spec Coverage",
        "",
        f"Source revision: `{payload['commit']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input digest: `{payload['input_digest']}`",
        "",
        "This report emits `proof_kind = none`; it does not infer expected behavior, close gaps, or promote invariants.",
        "",
        "## Summary",
        "",
        f"- Live facts: {summary['facts_total']}",
        f"- Mapped facts: {summary['mapped_total']}",
        f"- Missing specification: {summary['missing_spec']}",
        f"- Frozen historical debt: {summary['historical_debt']}",
        f"- New missing specification: {summary['new_missing_spec']}",
        f"- Stale reviews: {summary['stale_spec_review']}",
        f"- Unmapped ownership/area: {summary['unmapped']}",
        f"- Total findings: {summary['findings_total']}",
        f"- Enforcement mode: {payload['enforcement']['mode']}",
        f"- Blocking findings: {payload['enforcement']['blocking_total']}",
        "",
        "## Denominator Census",
        "",
        "### Sources",
        "",
    ]
    for row in payload["denominator"]["source_counts"]:
        lines.append(f"- `{row['name']}`: {row['count']}")
    lines.extend(["", "### Cargo targets", ""])
    for row in payload["denominator"]["cargo_target_counts"]:
        lines.append(f"- `{row['name']}`: {row['count']}")
    lines.extend(
        [
            "",
            "## Digest Rebinds",
            "",
            "| Code ID | Owner | Area | Reviewed digest | Live digest | Owner ack |",
            "| --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["digest_rebinds"]:
        lines.append(
            f"| `{row['code_id']}` | `{row['owner']}` | `{row['area']}` | "
            f"`{row['old_digest']}` | `{row['new_digest']}` | `{row['owner_ack'] or 'none'}` |"
        )
    lines.extend(
        [
            "",
            "## Raw Facts",
            "",
            "| Code ID | Source | Symbol | Kind | Area | Owner | Disposition | Risks | Findings |",
            "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
    )
    for row in payload["facts"]:
        lines.append(
            f"| `{_cell(row['code_id'])}` | `{_cell(row['path'])}:{row['line']}` | "
            f"`{_cell(row['qualified_symbol'])}` | `{_cell(row['item_kind'])}` | "
            f"`{_cell(row['area'])}` | `{_cell(row['owner'])}` | "
            f"`{_cell(row['disposition'])}/{_cell(row['debt_status'])}` | "
            f"`{_cell(', '.join(row['risk_signals']) or 'none')}` | "
            f"`{_cell('; '.join(row['findings']) or 'none')}` |"
        )
    return "\n".join(lines) + "\n"


def validate_report_bytes(json_bytes: bytes, markdown: str) -> list[str]:
    failures: list[str] = []
    try:
        payload = json.loads(json_bytes)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"report JSON is invalid: {error}"]
    canonical = canonical_json(payload).encode("utf-8")
    if canonical != json_bytes:
        failures.append("report is not canonical JSON serialization")
    digest = hashlib.sha256(canonical).hexdigest()
    expected_markdown = render_markdown(payload, json_digest=digest)
    if markdown != expected_markdown:
        failures.append("report Markdown does not reconstruct from canonical JSON")
    return failures


def _counts(values: Any) -> list[dict[str, Any]]:
    return [
        {"name": name, "count": count}
        for name, count in sorted(Counter(values).items())
    ]


def _counts_from_mapping(values: Mapping[str, int]) -> list[dict[str, Any]]:
    return [
        {"name": name, "count": count}
        for name, count in sorted(values.items())
    ]


def _disposition_ref(mapping: Mapping[str, Any] | None) -> str | None:
    if not mapping:
        return None
    for field in (
        "spec_ref",
        "spec_gap_ref",
        "parent_code_id",
        "generator_code_id",
        "boundary_spec_ref",
        "non_behavioral_reason",
    ):
        value = mapping.get(field)
        if isinstance(value, str) and value:
            return value
    return None


def _cell(value: str) -> str:
    return value.replace("|", "\\|").replace("`", "'").replace("\n", " ")
