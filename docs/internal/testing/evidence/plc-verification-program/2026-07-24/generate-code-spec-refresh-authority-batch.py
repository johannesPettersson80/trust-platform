#!/usr/bin/env python3
"""Reproduce the reviewed code-spec refresh mapping/risk bootstrap batch."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import tempfile
import tomllib
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[6]
sys.path.insert(0, str(REPO_ROOT))

from scripts.verification.code_spec_common import (  # noqa: E402
    RISK_SIGNALS,
    CodeFact,
    sha256_digest,
    stable_code_id,
)
from scripts.verification.code_spec_source_adapter import (  # noqa: E402
    candidate_area_owners,
    governed_source_bytes,
    select_area_owner,
)
from scripts.verification.code_spec_tooling import (  # noqa: E402
    discover_script_sources,
    extract_python_source,
    extract_shell_source,
)


MAP_PATH = "verification/code-spec-map.toml"
RISK_PATH = "verification/code-spec-risk-acknowledgements.toml"
MATRIX_PATH = "verification/matrix.toml"
OUTPUT_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "code-spec-refresh-authority-batch.json"
)
GENERATOR_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "generate-code-spec-refresh-authority-batch.py"
)
REVIEW_REF = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "code-spec-refresh-quarantine-risk-review.md"
    "#accepted-path-coherent-risk-batch"
)
PYTHON_PATHS = (
    "scripts/generate_code_spec_refresh_proposals.py",
    "scripts/verification/code_spec_refresh.py",
    "scripts/verification/code_spec_refresh_cli.py",
)
SHELL_PATH = "scripts/verification_metadata_gate.sh"
SELECTED_PATHS = (*PYTHON_PATHS, SHELL_PATH)
SPEC_REF = (
    "SPEC_VERIFICATION_PROGRAM_CONTRACT_001"
    "#code-to-spec-refresh-proposal-quarantine"
)
INVARIANT_ID = "VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001"
TEST_ID = "TEST_VERIFICATION_METADATA_GATE_001"
SHELL_GAP = "SPEC_GAP_P17_HISTORICAL_CODE_VERIFICATION_001"
REVIEWED_ON = "2026-07-24"
EXPIRES_ON = "2026-10-24"
RISK_RATIONALE = (
    "Accepts the scanner-derived risk signals for the exact code-spec refresh "
    "quarantine fact; the reviewed path batch forbids accepted-map writes, "
    "semantic inference, gap closure, and proof promotion."
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=str(REPO_ROOT))
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--apply", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser.parse_args()


def digest_bytes(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def digest_json(value: Any) -> str:
    encoded = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return digest_bytes(encoded)


def load_toml(root: Path, relative: str) -> dict[str, Any]:
    return tomllib.loads((root / relative).read_text(encoding="utf-8"))


def selected_facts(root: Path, mappings: list[dict[str, Any]]) -> dict[str, CodeFact]:
    matrix = load_toml(root, MATRIX_PATH)
    sources = {
        record["path"]: record
        for record in discover_script_sources(root)
        if record["path"] in SELECTED_PATHS
    }
    if set(sources) != set(SELECTED_PATHS):
        raise ValueError(
            f"selected source census drift: missing={sorted(set(SELECTED_PATHS) - set(sources))}"
        )
    area_bindings = {
        str(row["code_id"]): str(row["area"])
        for row in mappings
        if isinstance(row.get("code_id"), str) and isinstance(row.get("area"), str)
    }
    facts: dict[str, CodeFact] = {}
    for path in SELECTED_PATHS:
        source_record = sources[path]
        normalized_path, source = governed_source_bytes(root, path)
        if source_record["source_kind"] == "python":
            extracted = extract_python_source(source_record["module"], source)
        else:
            extracted = extract_shell_source(source_record["module"], source)
        candidates = candidate_area_owners(matrix, normalized_path)
        for record in extracted:
            signature = str(record["signature"])
            code_id = stable_code_id(
                source_kind=source_record["source_kind"],
                path=normalized_path,
                qualified_symbol=str(record["qualified_symbol"]),
                item_kind=str(record["item_kind"]),
                signature=signature,
            )
            area, owner = select_area_owner(code_id, candidates, area_bindings)
            if (area, owner) == ("unmapped", "unmapped"):
                raise ValueError(f"{code_id}: selected fact has ambiguous area routing")
            risks = tuple(sorted(set(record["risk_signals"])))
            unknown_risks = sorted(set(risks) - RISK_SIGNALS)
            if unknown_risks:
                raise ValueError(f"{code_id}: unknown risks {unknown_risks}")
            body_start = int(record["body_start"])
            body_end = int(record["body_end"])
            facts[code_id] = CodeFact(
                code_id=code_id,
                source_kind=source_record["source_kind"],
                path=normalized_path,
                qualified_symbol=str(record["qualified_symbol"]),
                item_kind=str(record["item_kind"]),
                signature=signature,
                signature_digest=sha256_digest(signature.encode()),
                body_digest=sha256_digest(source[body_start:body_end]),
                owner=owner,
                area=area,
                area_candidates=tuple(sorted(candidates)),
                visibility=str(record["visibility"]),
                risk_signals=risks,
                line=int(record["line"]),
            )
    return dict(sorted(facts.items()))


def generated_mapping(fact: CodeFact) -> dict[str, Any]:
    row: dict[str, Any] = {
        "schema_version": 1,
        "code_id": fact.code_id,
        "source_kind": fact.source_kind,
        "path": fact.path,
        "qualified_symbol": fact.qualified_symbol,
        "item_kind": fact.item_kind,
        "signature_digest": fact.signature_digest,
        "reviewed_body_digest": fact.body_digest,
        "owner": fact.owner,
        "area": fact.area,
        "visibility": fact.visibility,
        "risk_signals": list(fact.risk_signals),
    }
    if fact.risk_signals:
        row["owner_ack"] = fact.code_id.replace("CODE_", "RISK_ACK_", 1)
        row["owner_ack_expires_on"] = EXPIRES_ON
    if fact.path == SHELL_PATH:
        row["disposition"] = "spec_gap"
        row["spec_gap_ref"] = SHELL_GAP
    else:
        row["disposition"] = "specified"
        row["spec_ref"] = SPEC_REF
        row["invariant_ids"] = [INVARIANT_ID]
        row["test_ids"] = [TEST_ID]
    row["last_reviewed"] = REVIEWED_ON
    return row


def generated_acknowledgement(fact: CodeFact) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "id": fact.code_id.replace("CODE_", "RISK_ACK_", 1),
        "code_id": fact.code_id,
        "owner": fact.owner,
        "area": fact.area,
        "body_digest": fact.body_digest,
        "risk_signals": list(fact.risk_signals),
        "review_ref": REVIEW_REF,
        "reviewed_by": "verification",
        "reviewed_on": REVIEWED_ON,
        "expires_on": EXPIRES_ON,
        "rationale": RISK_RATIONALE,
    }


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
    blocks = [
        content[start : starts[index + 1] if index + 1 < len(starts) else None]
        for index, start in enumerate(starts)
    ]
    return content[: starts[0]], blocks


def record_from_block(block: bytes, key: str) -> dict[str, Any]:
    document = tomllib.loads(block.decode())
    records = document.get(key)
    if not isinstance(records, list) or len(records) != 1:
        raise ValueError(f"{key}: malformed record block")
    return dict(records[0])


def toml_value(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
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


def replace_selected_blocks(
    content: bytes,
    *,
    marker: str,
    record_key: str,
    selected_ids: set[str],
    generated: list[dict[str, Any]],
) -> tuple[bytes, str]:
    header, blocks = split_blocks(content, marker)
    kept = [
        block
        for block in blocks
        if str(record_from_block(block, record_key).get("code_id")) not in selected_ids
    ]
    preserved_digest = digest_bytes(b"".join(kept))
    rendered = b"".join(render_block(marker, row) for row in generated)
    output = header + b"".join(kept) + rendered
    return output.rstrip(b"\n") + b"\n", preserved_digest


def atomic_write(path: Path, content: bytes) -> None:
    descriptor, temporary = tempfile.mkstemp(
        prefix=f".{path.name}.",
        suffix=".staged",
        dir=path.parent,
    )
    temporary_path = Path(temporary)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary_path, path)
    finally:
        if temporary_path.exists():
            temporary_path.unlink()


def build(root: Path) -> tuple[bytes, bytes, bytes]:
    map_path = root / MAP_PATH
    risk_path = root / RISK_PATH
    map_before = map_path.read_bytes()
    risk_before = risk_path.read_bytes()
    map_document = tomllib.loads(map_before.decode())
    mappings = map_document.get("mappings")
    if not isinstance(mappings, list):
        raise ValueError("code-spec map mappings are malformed")
    old_selected_ids = {
        str(row["code_id"])
        for row in mappings
        if row.get("path") in SELECTED_PATHS
    }
    facts = selected_facts(root, mappings)
    selected_ids = old_selected_ids | set(facts)
    generated_mappings = [generated_mapping(fact) for fact in facts.values()]
    generated_acks = [
        generated_acknowledgement(fact)
        for fact in facts.values()
        if fact.risk_signals
    ]
    map_after, preserved_map_digest = replace_selected_blocks(
        map_before,
        marker="mappings",
        record_key="mappings",
        selected_ids=selected_ids,
        generated=generated_mappings,
    )
    risk_after, preserved_risk_digest = replace_selected_blocks(
        risk_before,
        marker="acknowledgements",
        record_key="acknowledgements",
        selected_ids=selected_ids,
        generated=generated_acks,
    )
    manifest = {
        "schema_version": 1,
        "batch_id": "CODE_SPEC_AUTHORITY_BATCH_REFRESH_QUARANTINE_20260724",
        "generator": GENERATOR_PATH,
        "generator_sha256": digest_bytes((root / GENERATOR_PATH).read_bytes()),
        "command": (
            "python3 "
            + GENERATOR_PATH
            + " --root . --apply"
        ),
        "check_command": (
            "python3 "
            + GENERATOR_PATH
            + " --root . --check"
        ),
        "selected_paths": list(SELECTED_PATHS),
        "source_digests": {
            path: digest_bytes((root / path).read_bytes())
            for path in SELECTED_PATHS
        },
        "authority": {
            "spec_ref": SPEC_REF,
            "invariant_id": INVARIANT_ID,
            "test_id": TEST_ID,
            "shell_spec_gap_ref": SHELL_GAP,
            "review_ref": REVIEW_REF,
        },
        "facts_total": len(facts),
        "risk_acknowledgements_total": len(generated_acks),
        "fact_set_digest": digest_json(
            {
                code_id: {
                    key: value
                    for key, value in vars(fact).items()
                }
                for code_id, fact in facts.items()
            }
        ),
        "generated_mapping_rows_digest": digest_json(generated_mappings),
        "generated_risk_rows_digest": digest_json(generated_acks),
        "preserved_unselected_map_blocks_sha256": preserved_map_digest,
        "preserved_unselected_risk_blocks_sha256": preserved_risk_digest,
        "output_code_map_sha256": digest_bytes(map_after),
        "output_risk_acknowledgements_sha256": digest_bytes(risk_after),
    }
    manifest_bytes = (
        json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode()
    return map_after, risk_after, manifest_bytes


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve()
    expected_map, expected_risk, expected_manifest = build(root)
    outputs = {
        root / MAP_PATH: expected_map,
        root / RISK_PATH: expected_risk,
        root / OUTPUT_PATH: expected_manifest,
    }
    if args.check:
        drift = [
            path.relative_to(root).as_posix()
            for path, expected in outputs.items()
            if not path.is_file() or path.read_bytes() != expected
        ]
        if drift:
            print("authority batch drift: " + ", ".join(drift), file=sys.stderr)
            return 1
        print("code-spec refresh authority batch is reproducible")
        return 0
    for path, content in outputs.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        atomic_write(path, content)
    print(
        "generated code-spec refresh authority batch: "
        f"map={digest_bytes(expected_map)} "
        f"risk={digest_bytes(expected_risk)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
