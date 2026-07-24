#!/usr/bin/env python3
"""Route the unique frozen unmapped bundle-builder fact to its reviewed gap."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from datetime import date
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[6]
sys.path.insert(0, str(REPO_ROOT))

from scripts.verification.code_spec_risk import risk_ack_id  # noqa: E402
from scripts.verification.mapping_transaction import (  # noqa: E402
    apply_mapping_transaction,
)


REPORT_PATH = (
    "target/gate-artifacts/code-spec-coverage-p18-refresh/"
    "code-spec-coverage.json"
)
MAP_PATH = "verification/code-spec-map.toml"
GAPS_PATH = "verification/spec-gaps.toml"
RISK_PATH = "verification/code-spec-risk-acknowledgements.toml"
SELECTION_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "collect-source-paths-gap-route-selection.toml"
)
PROPOSAL_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "collect-source-paths-gap-route-proposal.json"
)
REVIEW_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "collect-source-paths-gap-route-review.md"
)
REVIEW_ANCHOR = "accepted-exact-fact-gap-route"
GENERATOR_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "generate-collect-source-paths-gap-route.py"
)
OUTPUT_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "collect-source-paths-gap-route.json"
)
SELECTION_FIELDS = {
    "schema_version",
    "decision",
    "proposal",
    "proposal_digest",
    "reviewed_by",
    "independent_reviewer",
    "reviewed_on",
    "expires_on",
    "rationale",
}
FACT_FIELDS = {
    "code_id",
    "source_kind",
    "path",
    "qualified_symbol",
    "item_kind",
    "signature_digest",
    "body_digest",
    "visibility",
    "risk_signals",
    "area_candidates",
    "line",
}
SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
CODE_ID_RE = re.compile(r"^CODE_[0-9A-F]{64}$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=str(REPO_ROOT))
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--propose", action="store_true")
    mode.add_argument("--accept-proposal", action="store_true")
    mode.add_argument("--apply", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser.parse_args()


def digest_bytes(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def digest_json(value: Any) -> str:
    return digest_bytes(
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        ).encode()
    )


def load_toml(path: Path) -> dict[str, Any]:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def selection_document(
    root: Path,
    *,
    require_binding: bool = True,
) -> dict[str, Any]:
    selection = load_toml(root / SELECTION_PATH)
    expected_fields = SELECTION_FIELDS
    if not require_binding:
        expected_fields = SELECTION_FIELDS - {"proposal_digest"}
        if "proposal_digest" in selection:
            expected_fields = SELECTION_FIELDS
    if (
        set(selection) != expected_fields
        or selection.get("schema_version") != 1
        or selection.get("decision")
        != "accept_unique_unmapped_same_file_sibling_gap_route"
        or selection.get("proposal") != PROPOSAL_PATH
    ):
        raise ValueError("gap-route semantic selection is malformed")
    for field in ("reviewed_by", "independent_reviewer", "rationale"):
        if not isinstance(selection.get(field), str) or not selection[field].strip():
            raise ValueError(f"gap-route selection {field} is empty")
    try:
        reviewed_on = date.fromisoformat(str(selection["reviewed_on"]))
        expires_on = date.fromisoformat(str(selection["expires_on"]))
    except ValueError as error:
        raise ValueError("gap-route selection dates are malformed") from error
    if reviewed_on > expires_on or expires_on < date.today():
        raise ValueError("gap-route selection expiry is invalid")
    if require_binding:
        proposal_digest = selection.get("proposal_digest")
        if (
            not isinstance(proposal_digest, str)
            or SHA256_RE.fullmatch(proposal_digest) is None
        ):
            raise ValueError("gap-route selection proposal digest is malformed")
    return selection


def bind_selection_content(root: Path, *, proposal_digest: str) -> bytes:
    selection_document(root, require_binding=False)
    content = (root / SELECTION_PATH).read_text(encoding="utf-8")
    rendered = f'proposal_digest = "{proposal_digest}"'
    if re.search(r"(?m)^proposal_digest = .*$", content):
        content = re.sub(
            r"(?m)^proposal_digest = .*$",
            rendered,
            content,
            count=1,
        )
    else:
        content, replacements = re.subn(
            r'(?m)^(proposal = ".*")$',
            rf"\1\n{rendered}",
            content,
            count=1,
        )
        if replacements != 1:
            raise ValueError("gap-route selection lacks one proposal field")
    return content.encode()


def closed_fact(raw: Any) -> dict[str, Any]:
    if not isinstance(raw, dict):
        raise ValueError("unmapped code fact is malformed")
    if set(raw).intersection(FACT_FIELDS) != FACT_FIELDS:
        raise ValueError("unmapped code fact lacks required identity fields")
    fact = {
        "code_id": raw.get("code_id"),
        "source_kind": raw.get("source_kind"),
        "path": raw.get("path"),
        "qualified_symbol": raw.get("qualified_symbol"),
        "item_kind": raw.get("item_kind"),
        "signature_digest": raw.get("signature_digest"),
        "body_digest": raw.get("body_digest"),
        "visibility": raw.get("visibility"),
        "risk_signals": raw.get("risk_signals"),
        "area_candidates": raw.get("area_candidates"),
        "line": raw.get("line"),
    }
    if (
        set(fact) != FACT_FIELDS
        or not isinstance(fact["code_id"], str)
        or CODE_ID_RE.fullmatch(fact["code_id"]) is None
        or not isinstance(fact["signature_digest"], str)
        or SHA256_RE.fullmatch(fact["signature_digest"]) is None
        or not isinstance(fact["body_digest"], str)
        or SHA256_RE.fullmatch(fact["body_digest"]) is None
        or not isinstance(fact["risk_signals"], list)
        or not fact["risk_signals"]
        or any(not isinstance(item, str) for item in fact["risk_signals"])
        or fact["risk_signals"] != sorted(set(fact["risk_signals"]))
        or not isinstance(fact["area_candidates"], list)
        or not fact["area_candidates"]
        or any(not isinstance(item, str) for item in fact["area_candidates"])
        or fact["area_candidates"] != sorted(set(fact["area_candidates"]))
        or any(
            not isinstance(fact[field], str) or not fact[field]
            for field in (
                "source_kind",
                "path",
                "qualified_symbol",
                "item_kind",
                "visibility",
            )
        )
        or not isinstance(fact["line"], int)
        or isinstance(fact["line"], bool)
        or fact["line"] <= 0
    ):
        raise ValueError("unmapped code fact identity fields drift")
    return fact


def report_fact(root: Path) -> tuple[dict[str, Any], dict[str, str]]:
    report_path = root / REPORT_PATH
    report = json.loads(report_path.read_text(encoding="utf-8"))
    facts = report.get("facts")
    findings = report.get("findings")
    summary = report.get("summary")
    if (
        report.get("report_status") != "complete"
        or not isinstance(facts, list)
        or not isinstance(findings, list)
        or any(not isinstance(finding, str) for finding in findings)
        or not isinstance(summary, dict)
    ):
        raise ValueError("frozen code-spec report is incomplete")
    unmapped = [
        fact
        for fact in facts
        if isinstance(fact, dict)
        and isinstance(fact.get("findings"), list)
        and "unmapped" in fact["findings"]
    ]
    if len(unmapped) != 1 or summary.get("unmapped") != len(unmapped):
        raise ValueError("frozen report does not contain one unique unmapped fact")
    fact = closed_fact(unmapped[0])
    if f"{fact['code_id']}: unmapped" not in findings:
        raise ValueError("unmapped fact lacks its top-level frozen finding")
    commit = report.get("commit")
    input_digest = report.get("input_digest")
    if (
        not isinstance(commit, str)
        or COMMIT_RE.fullmatch(commit) is None
        or not isinstance(input_digest, str)
        or SHA256_RE.fullmatch(input_digest) is None
    ):
        raise ValueError("frozen report provenance is malformed")
    return fact, {
        "source_report_sha256": digest_bytes(report_path.read_bytes()),
        "source_report_commit": commit,
        "source_report_input_digest": input_digest,
    }


def verify_source_identity(
    root: Path,
    *,
    fact: dict[str, Any],
    source_commit: str,
) -> str:
    path = str(fact["path"])
    current = (root / path).read_bytes()
    try:
        frozen = subprocess.check_output(
            ["git", "show", f"{source_commit}:{path}"],
            cwd=root,
            stderr=subprocess.PIPE,
        )
    except subprocess.CalledProcessError as error:
        raise ValueError("cannot reconstruct frozen source identity") from error
    if current != frozen:
        raise ValueError("gap-route source changed after the frozen report")
    return digest_bytes(current)


def split_blocks(content: bytes, marker: str) -> tuple[bytes, list[bytes]]:
    starts = [
        match.start()
        for match in re.finditer(
            rf"(?m)^\[\[{re.escape(marker)}\]\]\n".encode(),
            content,
        )
    ]
    if not starts:
        raise ValueError(f"{marker}: document contains no record blocks")
    return content[: starts[0]], [
        content[start : starts[index + 1] if index + 1 < len(starts) else None]
        for index, start in enumerate(starts)
    ]


def block_record(block: bytes, key: str) -> dict[str, Any]:
    records = tomllib.loads(block.decode()).get(key)
    if not isinstance(records, list) or len(records) != 1:
        raise ValueError(f"{key}: malformed record block")
    return dict(records[0])


def toml_value(value: Any) -> str:
    if isinstance(value, int) and not isinstance(value, bool):
        return str(value)
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, list) and all(isinstance(item, str) for item in value):
        return "[" + ", ".join(json.dumps(item) for item in value) + "]"
    raise ValueError(f"unsupported TOML value {value!r}")


def render_block(marker: str, row: dict[str, Any]) -> bytes:
    lines = [f"[[{marker}]]"]
    lines.extend(f"{key} = {toml_value(value)}" for key, value in row.items())
    return ("\n".join(lines) + "\n\n").encode()


def route_from_siblings(
    root: Path,
    *,
    fact: dict[str, Any],
) -> tuple[dict[str, str], list[str], dict[str, Any], bytes]:
    map_content = (root / MAP_PATH).read_bytes()
    map_document = tomllib.loads(map_content.decode())
    rows = map_document.get("mappings")
    if not isinstance(rows, list):
        raise ValueError("code-spec map is malformed")
    mapped_ids = [
        str(row.get("code_id", ""))
        for row in rows
        if isinstance(row, dict)
    ]
    if (
        len(mapped_ids) != len(rows)
        or any(CODE_ID_RE.fullmatch(code_id) is None for code_id in mapped_ids)
        or len(set(mapped_ids)) != len(mapped_ids)
    ):
        raise ValueError("code-spec map identities are missing or duplicated")
    target_id = str(fact["code_id"])
    target_rows = [
        row for row in rows if isinstance(row, dict) and row.get("code_id") == target_id
    ]
    if len(target_rows) > 1:
        raise ValueError("target code fact is duplicated in the map")
    siblings = [
        row
        for row in rows
        if isinstance(row, dict)
        and row.get("path") == fact["path"]
        and row.get("code_id") != target_id
    ]
    if not siblings:
        raise ValueError("gap-route fact has no mapped same-file siblings")
    routes = {
        (
            row.get("area"),
            row.get("owner"),
            row.get("disposition"),
            row.get("spec_gap_ref"),
        )
        for row in siblings
    }
    if len(routes) != 1:
        raise ValueError("same-file sibling route is not unanimous")
    area, owner, disposition, gap_id = next(iter(routes))
    if (
        not isinstance(area, str)
        or area not in fact["area_candidates"]
        or not isinstance(owner, str)
        or disposition != "spec_gap"
        or not isinstance(gap_id, str)
        or not gap_id
    ):
        raise ValueError("same-file sibling route is not an eligible gap route")
    route = {"area": area, "owner": owner, "gap_id": gap_id}
    sibling_ids = sorted(str(row["code_id"]) for row in siblings)
    return route, sibling_ids, map_document, map_content


def validate_gap(
    root: Path,
    *,
    route: dict[str, str],
    sibling_ids: list[str],
    target_id: str,
) -> tuple[dict[str, Any], bytes]:
    content = (root / GAPS_PATH).read_bytes()
    document = tomllib.loads(content.decode())
    gaps = document.get("spec_gaps")
    if not isinstance(gaps, list):
        raise ValueError("spec-gap registry is malformed")
    target_memberships = []
    for candidate in gaps:
        if not isinstance(candidate, dict):
            raise ValueError("spec-gap registry row is malformed")
        affected_ids = candidate.get("affected_code_ids", [])
        if not isinstance(affected_ids, list) or any(
            not isinstance(code_id, str) for code_id in affected_ids
        ):
            raise ValueError("spec-gap affected_code_ids are malformed")
        if target_id in affected_ids:
            target_memberships.append(candidate.get("id"))
    if (
        len(target_memberships) > 1
        or target_memberships
        and target_memberships != [route["gap_id"]]
    ):
        raise ValueError("target code fact belongs to an unauthorized gap")
    matches = [
        gap
        for gap in gaps
        if isinstance(gap, dict) and gap.get("id") == route["gap_id"]
    ]
    if len(matches) != 1:
        raise ValueError("unanimous sibling gap is absent or duplicated")
    gap = matches[0]
    affected = gap.get("affected_code_ids")
    if (
        gap.get("status") != "spec_gap"
        or gap.get("area") != route["area"]
        or gap.get("owner") != route["owner"]
        or not isinstance(affected, list)
        or any(not isinstance(code_id, str) for code_id in affected)
        or affected != sorted(set(affected))
        or not set(sibling_ids).issubset(set(affected))
    ):
        raise ValueError("target historical gap authority drift")
    return document, content


def validate_risk_ledger(
    content: bytes,
    *,
    code_id: str,
    acknowledgement_id: str,
) -> None:
    document = tomllib.loads(content.decode())
    rows = document.get("acknowledgements")
    if not isinstance(rows, list) or any(not isinstance(row, dict) for row in rows):
        raise ValueError("risk acknowledgement ledger is malformed")
    ids = [row.get("id") for row in rows]
    code_ids = [row.get("code_id") for row in rows]
    if (
        any(not isinstance(value, str) for value in ids)
        or any(not isinstance(value, str) for value in code_ids)
        or len(set(ids)) != len(ids)
        or len(set(code_ids)) != len(code_ids)
    ):
        raise ValueError("risk acknowledgement identities are duplicated")
    claimed = [
        row
        for row in rows
        if row.get("id") == acknowledgement_id
        or row.get("code_id") == code_id
    ]
    if any(
        row.get("id") != acknowledgement_id
        or row.get("code_id") != code_id
        for row in claimed
    ):
        raise ValueError("generated risk acknowledgement identity is claimed")


def fresh_proposal(root: Path) -> dict[str, Any]:
    fact, provenance = report_fact(root)
    source_blob_sha256 = verify_source_identity(
        root,
        fact=fact,
        source_commit=provenance["source_report_commit"],
    )
    route, sibling_ids, _, _ = route_from_siblings(root, fact=fact)
    validate_gap(
        root,
        route=route,
        sibling_ids=sibling_ids,
        target_id=str(fact["code_id"]),
    )
    semantic = {
        "schema_version": 1,
        "generator": GENERATOR_PATH,
        "generator_sha256": digest_bytes((root / GENERATOR_PATH).read_bytes()),
        "source_report": REPORT_PATH,
        **provenance,
        "source_blob_sha256": source_blob_sha256,
        "fact": fact,
        "route": route,
        "sibling_code_ids": sibling_ids,
    }
    return {**semantic, "proposal_digest": digest_json(semantic)}


def load_reviewed_proposal(
    root: Path,
    *,
    selection: dict[str, Any] | None = None,
) -> dict[str, Any]:
    proposal = json.loads((root / PROPOSAL_PATH).read_text(encoding="utf-8"))
    expected_fields = {
        "schema_version",
        "generator",
        "generator_sha256",
        "source_report",
        "source_report_sha256",
        "source_report_commit",
        "source_report_input_digest",
        "source_blob_sha256",
        "fact",
        "route",
        "sibling_code_ids",
        "proposal_digest",
    }
    if (
        not isinstance(proposal, dict)
        or set(proposal) != expected_fields
        or proposal.get("schema_version") != 1
        or proposal.get("generator") != GENERATOR_PATH
        or proposal.get("generator_sha256")
        != digest_bytes((root / GENERATOR_PATH).read_bytes())
        or proposal.get("source_report") != REPORT_PATH
    ):
        raise ValueError("reviewed gap-route proposal fields drift")
    fact = proposal.get("fact")
    if not isinstance(fact, dict) or set(fact) != FACT_FIELDS:
        raise ValueError("reviewed gap-route proposal fact fields drift")
    proposal["fact"] = closed_fact(fact)
    for field in (
        "generator_sha256",
        "source_report_sha256",
        "source_report_input_digest",
        "source_blob_sha256",
        "proposal_digest",
    ):
        value = proposal.get(field)
        if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
            raise ValueError(f"reviewed gap-route proposal {field} is malformed")
    commit = proposal.get("source_report_commit")
    if not isinstance(commit, str) or COMMIT_RE.fullmatch(commit) is None:
        raise ValueError("reviewed gap-route proposal commit is malformed")
    route = proposal.get("route")
    if (
        not isinstance(route, dict)
        or set(route) != {"area", "owner", "gap_id"}
        or any(not isinstance(value, str) or not value for value in route.values())
    ):
        raise ValueError("reviewed gap-route proposal route is malformed")
    sibling_ids = proposal.get("sibling_code_ids")
    if (
        not isinstance(sibling_ids, list)
        or not sibling_ids
        or any(
            not isinstance(code_id, str)
            or CODE_ID_RE.fullmatch(code_id) is None
            for code_id in sibling_ids
        )
        or sibling_ids != sorted(set(sibling_ids))
    ):
        raise ValueError("reviewed gap-route proposal siblings are malformed")
    semantic = {
        key: value for key, value in proposal.items() if key != "proposal_digest"
    }
    if proposal["proposal_digest"] != digest_json(semantic):
        raise ValueError("reviewed gap-route proposal digest changed")
    if (
        selection is not None
        and selection.get("proposal_digest") != proposal["proposal_digest"]
    ):
        raise ValueError("gap-route selection does not bind this proposal")
    return proposal


def generated_mapping(
    *,
    fact: dict[str, Any],
    route: dict[str, str],
    selection: dict[str, Any],
) -> dict[str, Any]:
    code_id = str(fact["code_id"])
    return {
        "schema_version": 1,
        "code_id": code_id,
        "source_kind": fact["source_kind"],
        "path": fact["path"],
        "qualified_symbol": fact["qualified_symbol"],
        "item_kind": fact["item_kind"],
        "signature_digest": fact["signature_digest"],
        "reviewed_body_digest": fact["body_digest"],
        "owner": route["owner"],
        "area": route["area"],
        "visibility": fact["visibility"],
        "risk_signals": fact["risk_signals"],
        "owner_ack": risk_ack_id(code_id),
        "owner_ack_expires_on": selection["expires_on"],
        "disposition": "spec_gap",
        "spec_gap_ref": route["gap_id"],
        "last_reviewed": selection["reviewed_on"],
    }


def generated_acknowledgement(
    *,
    fact: dict[str, Any],
    route: dict[str, str],
    selection: dict[str, Any],
) -> dict[str, Any]:
    code_id = str(fact["code_id"])
    return {
        "schema_version": 1,
        "id": risk_ack_id(code_id),
        "code_id": code_id,
        "owner": route["owner"],
        "area": route["area"],
        "body_digest": fact["body_digest"],
        "risk_signals": fact["risk_signals"],
        "review_ref": f"{REVIEW_PATH}#{REVIEW_ANCHOR}",
        "reviewed_by": route["owner"],
        "reviewed_on": selection["reviewed_on"],
        "expires_on": selection["expires_on"],
        "rationale": (
            "Acknowledges the scanner-derived risk signals for this exact "
            "reviewed body while the fact remains bound to an explicit open "
            "historical code-spec gap; it neither specifies behavior nor "
            "suppresses any risk signal."
        ),
    }


def replace_record(
    content: bytes,
    *,
    marker: str,
    key: str,
    identity_field: str,
    identity: str,
    generated: dict[str, Any],
) -> bytes:
    header, blocks = split_blocks(content, marker)
    matches = [
        block
        for block in blocks
        if block_record(block, key).get(identity_field) == identity
    ]
    if len(matches) > 1:
        raise ValueError(f"{marker}: target record is duplicated")
    kept = [block for block in blocks if block not in matches]
    preserved = (header + b"".join(kept)).rstrip(b"\n")
    output = preserved + b"\n\n" + render_block(marker, generated)
    return output.rstrip(b"\n") + b"\n"


def update_gap_membership(
    content: bytes,
    *,
    gap_id: str,
    code_id: str,
) -> bytes:
    header, blocks = split_blocks(content, "spec_gaps")
    updated_blocks = []
    matched = 0
    for block in blocks:
        record = block_record(block, "spec_gaps")
        if record.get("id") != gap_id:
            updated_blocks.append(block)
            continue
        matched += 1
        affected = record.get("affected_code_ids")
        if not isinstance(affected, list) or any(
            not isinstance(item, str) for item in affected
        ):
            raise ValueError("target gap affected_code_ids are malformed")
        if affected != sorted(set(affected)):
            raise ValueError("target gap affected_code_ids are not canonical")
        updated_ids = sorted(set(affected) | {code_id})
        rendered = "affected_code_ids = [\n"
        rendered += "".join(f"  {json.dumps(item)},\n" for item in updated_ids)
        rendered += "]\n"
        updated, count = re.subn(
            r"(?ms)^affected_code_ids = \[\n.*?^\]\n",
            rendered,
            block.decode(),
            count=1,
        )
        if count != 1:
            raise ValueError("target gap lacks one affected_code_ids block")
        parsed = block_record(updated.encode(), "spec_gaps")
        if parsed.get("affected_code_ids") != updated_ids:
            raise ValueError("target gap membership update failed")
        before = dict(record)
        after = dict(parsed)
        before.pop("affected_code_ids", None)
        after.pop("affected_code_ids", None)
        if before != after:
            raise ValueError("gap generator changed unrelated fields")
        updated_blocks.append(updated.encode())
    if matched != 1:
        raise ValueError("target gap match is not unique")
    return header + b"".join(updated_blocks)


def review_markdown(
    *,
    fact: dict[str, Any],
    route: dict[str, str],
    sibling_ids: list[str],
    selection: dict[str, Any],
    provenance: dict[str, str],
    source_blob_sha256: str,
) -> bytes:
    lines = [
        "# Collect source paths gap-route review",
        "",
        f"## {REVIEW_ANCHOR.replace('-', ' ').title()}",
        "",
        f"- Decision: `{selection['decision']}`",
        f"- Code identity: `{fact['code_id']}`",
        f"- Qualified symbol: `{fact['qualified_symbol']}`",
        f"- Signature digest: `{fact['signature_digest']}`",
        f"- Body digest: `{fact['body_digest']}`",
        f"- Source blob digest: `{source_blob_sha256}`",
        f"- Area: `{route['area']}`",
        f"- Owner: `{route['owner']}`",
        f"- Open gap: `{route['gap_id']}`",
        f"- Risk signals: `{json.dumps(fact['risk_signals'])}`",
        f"- Reviewed by: `{selection['reviewed_by']}`",
        f"- Independent reviewer: `{selection['independent_reviewer']}`",
        f"- Reviewed on: `{selection['reviewed_on']}`",
        f"- Acknowledgement expires on: `{selection['expires_on']}`",
        f"- Frozen report commit: `{provenance['source_report_commit']}`",
        f"- Frozen report SHA-256: `{provenance['source_report_sha256']}`",
        f"- Same-file mapped siblings: `{len(sibling_ids)}`",
        "",
        selection["rationale"],
        "",
        "This accepted decision is metadata routing only. It creates no "
        "behavior specification, test mapping, proof promotion, gap closure, "
        "or IEC decision/deviation.",
        "",
    ]
    return "\n".join(lines).encode()


def build_outputs(root: Path, *, applying: bool) -> dict[str, bytes]:
    selection = selection_document(root)
    proposal = load_reviewed_proposal(root, selection=selection)
    if applying:
        if not (root / REPORT_PATH).is_file():
            raise ValueError("gap-route apply requires the frozen report")
        if fresh_proposal(root) != proposal:
            raise ValueError(
                "frozen report or live route differs from reviewed proposal"
            )
    fact = dict(proposal["fact"])
    provenance = {
        "source_report_sha256": proposal["source_report_sha256"],
        "source_report_commit": proposal["source_report_commit"],
        "source_report_input_digest": proposal[
            "source_report_input_digest"
        ],
    }
    source_blob_sha256 = verify_source_identity(
        root,
        fact=fact,
        source_commit=provenance["source_report_commit"],
    )
    if source_blob_sha256 != proposal["source_blob_sha256"]:
        raise ValueError("reviewed proposal source blob changed")
    route, sibling_ids, _, map_content = route_from_siblings(
        root,
        fact=fact,
    )
    if route != proposal["route"] or sibling_ids != proposal["sibling_code_ids"]:
        raise ValueError("live same-file route differs from reviewed proposal")
    if selection["reviewed_by"] != route["owner"]:
        raise ValueError("gap-route decision was not reviewed by the owner")
    _, gaps_content = validate_gap(
        root,
        route=route,
        sibling_ids=sibling_ids,
        target_id=str(fact["code_id"]),
    )
    mapping = generated_mapping(
        fact=fact,
        route=route,
        selection=selection,
    )
    acknowledgement = generated_acknowledgement(
        fact=fact,
        route=route,
        selection=selection,
    )
    target_id = str(fact["code_id"])
    risk_content = (root / RISK_PATH).read_bytes()
    validate_risk_ledger(
        risk_content,
        code_id=target_id,
        acknowledgement_id=str(acknowledgement["id"]),
    )
    outputs = {
        MAP_PATH: replace_record(
            map_content,
            marker="mappings",
            key="mappings",
            identity_field="code_id",
            identity=target_id,
            generated=mapping,
        ),
        RISK_PATH: replace_record(
            risk_content,
            marker="acknowledgements",
            key="acknowledgements",
            identity_field="code_id",
            identity=target_id,
            generated=acknowledgement,
        ),
        GAPS_PATH: update_gap_membership(
            gaps_content,
            gap_id=route["gap_id"],
            code_id=target_id,
        ),
    }
    outputs[REVIEW_PATH] = review_markdown(
        fact=fact,
        route=route,
        sibling_ids=sibling_ids,
        selection=selection,
        provenance=provenance,
        source_blob_sha256=source_blob_sha256,
    )
    manifest = {
        "schema_version": 1,
        "batch_id": "COLLECT_SOURCE_PATHS_GAP_ROUTE_20260724",
        "generator": GENERATOR_PATH,
        "generator_sha256": digest_bytes((root / GENERATOR_PATH).read_bytes()),
        "command": f"python3 {GENERATOR_PATH} --root . --apply",
        "check_command": f"python3 {GENERATOR_PATH} --root . --check",
        "selection": SELECTION_PATH,
        "selection_sha256": digest_bytes((root / SELECTION_PATH).read_bytes()),
        "proposal": PROPOSAL_PATH,
        "proposal_sha256": digest_bytes((root / PROPOSAL_PATH).read_bytes()),
        "proposal_digest": proposal["proposal_digest"],
        "source_report": REPORT_PATH,
        **provenance,
        "source_blob_sha256": source_blob_sha256,
        "fact": fact,
        "route": route,
        "sibling_code_ids": sibling_ids,
        "generated_mapping": mapping,
        "generated_risk_acknowledgement": acknowledgement,
        "output_digests": {
            path: digest_bytes(content)
            for path, content in sorted(outputs.items())
        },
    }
    outputs[OUTPUT_PATH] = (
        json.dumps(manifest, indent=2, sort_keys=True) + "\n"
    ).encode()
    return outputs


def validate_outputs(root: Path, outputs: dict[str, bytes]) -> list[str]:
    return [
        f"{path}: generated bytes differ"
        for path, expected in outputs.items()
        if not (root / path).is_file() or (root / path).read_bytes() != expected
    ]


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve()
    try:
        if args.propose:
            if not (root / REPORT_PATH).is_file():
                raise ValueError("gap-route proposal requires the frozen report")
            proposal = fresh_proposal(root)
            content = (
                json.dumps(proposal, indent=2, sort_keys=True) + "\n"
            ).encode()
            apply_mapping_transaction(
                root=root,
                replacements={PROPOSAL_PATH: content},
                validate=lambda: (
                    []
                    if (root / PROPOSAL_PATH).is_file()
                    and (root / PROPOSAL_PATH).read_bytes() == content
                    else ["generated proposal bytes differ"]
                ),
            )
            print("collect_source_paths gap-route proposal generated")
            return 0
        if args.accept_proposal:
            proposal = load_reviewed_proposal(root)
            content = bind_selection_content(
                root,
                proposal_digest=str(proposal["proposal_digest"]),
            )
            apply_mapping_transaction(
                root=root,
                replacements={SELECTION_PATH: content},
                validate=lambda: (
                    []
                    if selection_document(root).get("proposal_digest")
                    == proposal["proposal_digest"]
                    else ["selection does not bind the reviewed proposal"]
                ),
            )
            print("collect_source_paths gap-route proposal accepted")
            return 0
        outputs = build_outputs(root, applying=args.apply)
        if args.check:
            failures = validate_outputs(root, outputs)
            if failures:
                raise ValueError("; ".join(failures))
            manifest = json.loads(outputs[OUTPUT_PATH])
            print(
                "collect_source_paths gap route is reproducible: "
                f"siblings={len(manifest['sibling_code_ids'])}"
            )
            return 0
        apply_mapping_transaction(
            root=root,
            replacements=outputs,
            validate=lambda: validate_outputs(root, outputs),
        )
    except (
        OSError,
        ValueError,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
        subprocess.SubprocessError,
    ) as error:
        print(f"collect_source_paths gap route failed: {error}", file=sys.stderr)
        return 1
    print("collect_source_paths gap route generated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
