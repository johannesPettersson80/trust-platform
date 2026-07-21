"""Thin CLI boundary for the Phase 18 zero-debt substrate."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import tomllib
from pathlib import Path

from ..test_catalog_json_schema import validate_json_schema_instance
from .artifacts import write_artifacts
from .orchestrator import GateResult, evaluate_bundle


CONTRACTS = (
    ("verification/p18-zero-debt-ledger.toml", "verification/schemas/p18-zero-debt-ledger.schema.json"),
    ("verification/p18-proof-budgets.toml", "verification/schemas/p18-proof-budgets.schema.json"),
    ("verification/p18-claim-matrix.toml", "verification/schemas/p18-claim-matrix.schema.json"),
)
EXPECTED_SOURCE_SHA = "a74efe093d4a9fbb60e3d4cf3fd89dbaf8c2ce35"


def resolve_producer_source_sha(root: Path, override: str | None) -> tuple[str, list[str]]:
    """Resolve an exact clean producer commit or validate an explicit test override."""

    if override is not None:
        if re.fullmatch(r"[0-9a-f]{40}", override):
            return override, []
        return "unavailable", ["producer source SHA override must be 40 lower-case hex"]
    head = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        check=False,
        capture_output=True,
        text=True,
    )
    if head.returncode != 0 or not re.fullmatch(r"[0-9a-f]{40}", head.stdout.strip()):
        return "unavailable", ["producer git HEAD is unavailable"]
    status = subprocess.run(
        ["git", "-C", str(root), "status", "--porcelain", "--untracked-files=normal"],
        check=False,
        capture_output=True,
        text=True,
    )
    if status.returncode != 0:
        return "unavailable", ["producer git status is unavailable"]
    if status.stdout.strip():
        return head.stdout.strip(), ["producer working tree is dirty"]
    return head.stdout.strip(), []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--expected-source-sha", default=EXPECTED_SOURCE_SHA)
    parser.add_argument("--producer-source-sha")
    parser.add_argument("--mode", choices=("report", "strict"), default="report")
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("target/gate-artifacts/p18-zero-debt/report-only"),
    )
    args = parser.parse_args(argv)
    root = args.root.resolve()
    producer_source_sha, producer_failures = resolve_producer_source_sha(
        root, args.producer_source_sha
    )
    documents = []
    schema_failures: list[str] = list(producer_failures)
    input_paths: list[Path] = []
    try:
        for document_path, schema_path in CONTRACTS:
            document_file = root / document_path
            schema_file = root / schema_path
            document = tomllib.loads(document_file.read_text())
            schema = json.loads(schema_file.read_text())
            documents.append(document)
            input_paths.extend((document_file, schema_file))
            schema_failures.extend(
                f"{document_path}: {failure}"
                for failure in validate_json_schema_instance(document, schema)
            )
    except (OSError, ValueError, tomllib.TOMLDecodeError, json.JSONDecodeError) as error:
        schema_failures.append(f"contract load failed: {error}")
    source_sha = args.expected_source_sha
    if len(documents) == 3:
        for row in documents[0].get("proof_classes", []):
            raw_path = row.get("raw_artifact") if isinstance(row, dict) else None
            if isinstance(raw_path, str):
                relative = Path(raw_path)
                candidate = root / relative
                if not relative.is_absolute() and ".." not in relative.parts and candidate.is_file():
                    input_paths.append(candidate)
    if schema_failures or len(documents) != 3:
        result = GateResult(
            exit_code=2,
            report={
                "schema_version": 1,
                "status": "contract_error",
                "source_sha": source_sha,
                "summary": {},
                "proof_classes": [],
                "findings": sorted(set(schema_failures)),
            },
        )
    else:
        result = evaluate_bundle(
            documents[0],
            documents[1],
            documents[2],
            artifact_root=root,
            expected_source_sha=source_sha,
            mode=args.mode,
        )
    result.report["contract_source_sha"] = source_sha
    result.report["source_sha"] = producer_source_sha
    write_artifacts(
        args.out_dir,
        result.report,
        input_paths=input_paths,
        source_sha=source_sha,
        exit_status=result.exit_code,
        display_root=root,
    )
    print(
        f"Phase 18 zero-debt {args.mode}: {result.report['status']}; "
        f"{len(result.report.get('findings', []))} contract findings"
    )
    return result.exit_code
