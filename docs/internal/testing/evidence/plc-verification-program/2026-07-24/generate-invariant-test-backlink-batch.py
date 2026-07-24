#!/usr/bin/env python3
"""Generate reciprocal invariant test backlinks from existing exact authority."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tomllib
from collections import defaultdict
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[6]
sys.path.insert(0, str(REPO_ROOT))

from scripts.verification.mapping_transaction import (  # noqa: E402
    apply_mapping_transaction,
)


REPORT_PATH = (
    "target/gate-artifacts/code-spec-coverage-p18-refresh/"
    "code-spec-coverage.json"
)
CATALOG_PATH = "verification/test-catalog.toml"
CODE_MAP_PATH = "verification/code-spec-map.toml"
INVARIANT_ROOT = "verification/invariants"
GENERATOR_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "generate-invariant-test-backlink-batch.py"
)
OUTPUT_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "invariant-test-backlink-batch.json"
)
FINDING_RE = re.compile(
    r"^(CODE_[0-9A-F]+): invariant ([A-Z0-9_]+) "
    r"does not link mapped test ([A-Z0-9_]+)$"
)
TESTS_BLOCK_RE = re.compile(r"(?ms)^tests = \[\n.*?^\]\n")
SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
RELATIONSHIP_FIELDS = {
    "invariant_id",
    "invariant_path",
    "test_id",
    "catalog_discovery_id",
    "area",
    "oracle_ref",
    "finding_count",
    "code_ids",
    "findings",
}


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


def validate_provenance(provenance: dict[str, Any]) -> None:
    if (
        not SHA256_RE.fullmatch(
            str(provenance.get("source_report_sha256", ""))
        )
        or not COMMIT_RE.fullmatch(
            str(provenance.get("source_report_commit", ""))
        )
        or not SHA256_RE.fullmatch(
            str(provenance.get("source_report_input_digest", ""))
        )
    ):
        raise ValueError("frozen backlink report provenance is malformed")


def invariant_documents(
    root: Path,
) -> dict[str, tuple[str, dict[str, Any], bytes]]:
    documents: dict[str, tuple[str, dict[str, Any], bytes]] = {}
    for path in sorted((root / INVARIANT_ROOT).rglob("*.toml")):
        content = path.read_bytes()
        document = tomllib.loads(content.decode())
        invariant_id = document.get("id")
        if not isinstance(invariant_id, str):
            continue
        if invariant_id in documents:
            raise ValueError(f"duplicate invariant ID {invariant_id}")
        documents[invariant_id] = (
            path.relative_to(root).as_posix(),
            document,
            content,
        )
    return documents


def exact_authorized_relationships(
    root: Path,
) -> tuple[
    dict[str, Any],
    list[dict[str, Any]],
    dict[str, tuple[str, dict[str, Any], bytes]],
]:
    report_path = root / REPORT_PATH
    report = json.loads(report_path.read_text(encoding="utf-8"))
    findings = report.get("findings")
    if not isinstance(findings, list) or any(
        not isinstance(finding, str) for finding in findings
    ):
        raise ValueError("frozen code-spec report findings are malformed")
    finding_groups: dict[tuple[str, str], list[tuple[str, str]]] = defaultdict(list)
    for finding in findings:
        match = FINDING_RE.fullmatch(finding)
        if match is None:
            continue
        code_id, invariant_id, test_id = match.groups()
        finding_groups[(invariant_id, test_id)].append((code_id, finding))

    catalog = load_toml(root / CATALOG_PATH)
    catalog_rows = catalog.get("tests")
    if not isinstance(catalog_rows, list):
        raise ValueError("test catalog rows are malformed")
    tests = {
        str(row["id"]): row
        for row in catalog_rows
        if isinstance(row, dict) and isinstance(row.get("id"), str)
    }
    if len(tests) != len(catalog_rows):
        raise ValueError("test catalog IDs are missing or duplicated")

    code_map = load_toml(root / CODE_MAP_PATH)
    mapping_rows = code_map.get("mappings")
    if not isinstance(mapping_rows, list):
        raise ValueError("code-spec mappings are malformed")
    mappings = {
        str(row["code_id"]): row
        for row in mapping_rows
        if isinstance(row, dict) and isinstance(row.get("code_id"), str)
    }
    if len(mappings) != len(mapping_rows):
        raise ValueError("code-spec mapping IDs are missing or duplicated")

    invariants = invariant_documents(root)
    relationships: list[dict[str, Any]] = []
    for (invariant_id, test_id), grouped_findings in sorted(
        finding_groups.items()
    ):
        test = tests.get(test_id)
        invariant_record = invariants.get(invariant_id)
        if test is None or invariant_record is None:
            continue
        invariant_path, invariant, _ = invariant_record
        test_invariants = test.get("invariants")
        if (
            not isinstance(test_invariants, list)
            or invariant_id not in test_invariants
        ):
            continue
        oracle = invariant.get("oracle")
        if (
            test.get("status") != "mapped"
            or test.get("area") != invariant.get("area")
            or not isinstance(oracle, dict)
            or test.get("oracle_ref") != oracle.get("ref")
        ):
            raise ValueError(
                f"{invariant_id}/{test_id}: explicit catalog authority "
                "does not match invariant area and oracle"
            )
        code_ids = sorted(code_id for code_id, _ in grouped_findings)
        if len(set(code_ids)) != len(code_ids):
            raise ValueError(
                f"{invariant_id}/{test_id}: duplicate frozen findings"
            )
        for code_id in code_ids:
            mapping = mappings.get(code_id)
            mapping_invariants = (
                mapping.get("invariant_ids")
                if isinstance(mapping, dict)
                else None
            )
            mapping_tests = (
                mapping.get("test_ids")
                if isinstance(mapping, dict)
                else None
            )
            if (
                mapping is None
                or not isinstance(mapping_invariants, list)
                or invariant_id not in mapping_invariants
                or not isinstance(mapping_tests, list)
                or test_id not in mapping_tests
            ):
                raise ValueError(
                    f"{invariant_id}/{test_id}: frozen finding lacks exact "
                    f"code-map authority at {code_id}"
                )
        relationships.append(
            {
                "invariant_id": invariant_id,
                "invariant_path": invariant_path,
                "test_id": test_id,
                "catalog_discovery_id": test.get("discovery_id"),
                "area": test.get("area"),
                "oracle_ref": test.get("oracle_ref"),
                "finding_count": len(grouped_findings),
                "code_ids": code_ids,
                "findings": sorted(finding for _, finding in grouped_findings),
            }
        )
    if not relationships:
        raise ValueError("frozen report contains no exact authorized backlinks")
    return report, relationships, invariants


def validate_frozen_relationships(
    root: Path,
    relationships: list[dict[str, Any]],
    invariants: dict[str, tuple[str, dict[str, Any], bytes]],
) -> None:
    if not relationships:
        raise ValueError("frozen backlink manifest contains no relationships")
    catalog = load_toml(root / CATALOG_PATH)
    catalog_rows = catalog.get("tests")
    if not isinstance(catalog_rows, list):
        raise ValueError("test catalog rows are malformed")
    tests = {
        str(row["id"]): row
        for row in catalog_rows
        if isinstance(row, dict) and isinstance(row.get("id"), str)
    }
    if len(tests) != len(catalog_rows):
        raise ValueError("test catalog IDs are missing or duplicated")
    code_map = load_toml(root / CODE_MAP_PATH)
    mapping_rows = code_map.get("mappings")
    if not isinstance(mapping_rows, list):
        raise ValueError("code-spec mappings are malformed")
    mappings = {
        str(row["code_id"]): row
        for row in mapping_rows
        if isinstance(row, dict) and isinstance(row.get("code_id"), str)
    }
    if len(mappings) != len(mapping_rows):
        raise ValueError("code-spec mapping IDs are missing or duplicated")
    expected_order = sorted(
        relationships,
        key=lambda row: (
            str(row.get("invariant_id", "")),
            str(row.get("test_id", "")),
        ),
    )
    if relationships != expected_order:
        raise ValueError("frozen backlink relationships are not canonical")
    seen_pairs: set[tuple[str, str]] = set()
    for relationship in relationships:
        if set(relationship) != RELATIONSHIP_FIELDS:
            raise ValueError("frozen backlink relationship fields drift")
        invariant_id = relationship.get("invariant_id")
        test_id = relationship.get("test_id")
        if not isinstance(invariant_id, str) or not isinstance(test_id, str):
            raise ValueError("frozen backlink relationship IDs are malformed")
        pair = (invariant_id, test_id)
        if pair in seen_pairs:
            raise ValueError("frozen backlink relationship is duplicated")
        seen_pairs.add(pair)
        test = tests.get(test_id)
        invariant_record = invariants.get(invariant_id)
        if test is None or invariant_record is None:
            raise ValueError(f"{invariant_id}/{test_id}: authority is absent")
        invariant_path, invariant, _ = invariant_record
        oracle = invariant.get("oracle")
        test_invariants = test.get("invariants")
        if (
            relationship.get("invariant_path") != invariant_path
            or test.get("status") != "mapped"
            or not isinstance(test_invariants, list)
            or invariant_id not in test_invariants
            or test.get("area") != invariant.get("area")
            or not isinstance(oracle, dict)
            or test.get("oracle_ref") != oracle.get("ref")
            or relationship.get("catalog_discovery_id")
            != test.get("discovery_id")
            or relationship.get("area") != test.get("area")
            or relationship.get("oracle_ref") != test.get("oracle_ref")
        ):
            raise ValueError(
                f"{invariant_id}/{test_id}: tracked authority drift"
            )
        code_ids = relationship.get("code_ids")
        findings = relationship.get("findings")
        finding_count = relationship.get("finding_count")
        if (
            not isinstance(code_ids, list)
            or any(not isinstance(code_id, str) for code_id in code_ids)
            or code_ids != sorted(set(code_ids))
            or not isinstance(findings, list)
            or any(not isinstance(finding, str) for finding in findings)
            or findings != sorted(set(findings))
            or not isinstance(finding_count, int)
            or isinstance(finding_count, bool)
            or finding_count != len(code_ids)
            or finding_count != len(findings)
        ):
            raise ValueError(
                f"{invariant_id}/{test_id}: frozen finding set drift"
            )
        parsed_code_ids = []
        for finding in findings:
            match = FINDING_RE.fullmatch(finding)
            if (
                match is None
                or match.group(2) != invariant_id
                or match.group(3) != test_id
            ):
                raise ValueError(
                    f"{invariant_id}/{test_id}: frozen finding identity drift"
                )
            parsed_code_ids.append(match.group(1))
        if sorted(parsed_code_ids) != code_ids:
            raise ValueError(
                f"{invariant_id}/{test_id}: finding/code identity drift"
            )
        for code_id in code_ids:
            mapping = mappings.get(code_id)
            mapping_invariants = (
                mapping.get("invariant_ids")
                if isinstance(mapping, dict)
                else None
            )
            mapping_tests = (
                mapping.get("test_ids")
                if isinstance(mapping, dict)
                else None
            )
            if (
                mapping is None
                or not isinstance(mapping_invariants, list)
                or invariant_id not in mapping_invariants
                or not isinstance(mapping_tests, list)
                or test_id not in mapping_tests
            ):
                raise ValueError(
                    f"{invariant_id}/{test_id}: tracked code-map authority "
                    f"drift at {code_id}"
                )


def render_tests_backlinks(
    content: bytes,
    *,
    test_ids: list[str],
) -> bytes:
    source = content.decode()
    document = tomllib.loads(source)
    current = document.get("tests")
    if not isinstance(current, list) or any(
        not isinstance(test_id, str) for test_id in current
    ):
        raise ValueError("invariant tests are malformed")
    additions = sorted(set(test_ids) - set(current))
    updated_tests = [*current, *additions]
    rendered = "tests = [\n"
    rendered += "".join(f"  {json.dumps(test_id)},\n" for test_id in updated_tests)
    rendered += "]\n"
    updated, count = TESTS_BLOCK_RE.subn(rendered, source, count=1)
    if count != 1:
        raise ValueError("invariant lacks one multiline top-level tests block")
    updated_document = tomllib.loads(updated)
    if updated_document.get("tests") != updated_tests:
        raise ValueError("generated invariant tests failed round-trip validation")
    before_without_tests = dict(document)
    after_without_tests = dict(updated_document)
    before_without_tests.pop("tests", None)
    after_without_tests.pop("tests", None)
    if before_without_tests != after_without_tests:
        raise ValueError("generator changed invariant fields other than tests")
    return updated.encode()


def build_outputs(root: Path, *, applying: bool) -> dict[str, bytes]:
    manifest_path = root / OUTPUT_PATH
    report_path = root / REPORT_PATH
    prior_manifest = (
        json.loads(manifest_path.read_text(encoding="utf-8"))
        if manifest_path.is_file()
        else None
    )
    if applying:
        if not report_path.is_file():
            raise ValueError(
                "backlink apply requires the frozen code-spec report"
            )
        report, relationships, invariants = exact_authorized_relationships(root)
        provenance = {
            "source_report_sha256": digest_bytes(report_path.read_bytes()),
            "source_report_commit": report.get("commit"),
            "source_report_input_digest": report.get("input_digest"),
        }
    elif isinstance(prior_manifest, dict):
        raw_relationships = prior_manifest.get("relationships")
        if not isinstance(raw_relationships, list) or any(
            not isinstance(relationship, dict)
            for relationship in raw_relationships
        ):
            raise ValueError("committed backlink manifest is malformed")
        relationships = [
            dict(relationship) for relationship in raw_relationships
        ]
        invariants = invariant_documents(root)
        provenance = {
            "source_report_sha256": prior_manifest.get(
                "source_report_sha256"
            ),
            "source_report_commit": prior_manifest.get(
                "source_report_commit"
            ),
            "source_report_input_digest": prior_manifest.get(
                "source_report_input_digest"
            ),
        }
    elif report_path.is_file():
        report, relationships, invariants = exact_authorized_relationships(root)
        provenance = {
            "source_report_sha256": digest_bytes(report_path.read_bytes()),
            "source_report_commit": report.get("commit"),
            "source_report_input_digest": report.get("input_digest"),
        }
    else:
        raise ValueError(
            "backlink generation needs either the frozen report or "
            "the committed minimal manifest"
        )
    validate_provenance(provenance)
    validate_frozen_relationships(root, relationships, invariants)
    tests_by_path: dict[str, list[str]] = defaultdict(list)
    for relationship in relationships:
        tests_by_path[str(relationship["invariant_path"])].append(
            str(relationship["test_id"])
        )
    outputs: dict[str, bytes] = {}
    for path, test_ids in sorted(tests_by_path.items()):
        invariant_id = next(
            invariant_id
            for invariant_id, (candidate_path, _, _) in invariants.items()
            if candidate_path == path
        )
        _, _, content = invariants[invariant_id]
        outputs[path] = render_tests_backlinks(content, test_ids=test_ids)
    relationship_digest = digest_json(relationships)
    manifest = {
        "schema_version": 1,
        "batch_id": "INVARIANT_TEST_BACKLINK_BATCH_20260724",
        "generator": GENERATOR_PATH,
        "generator_sha256": digest_bytes((root / GENERATOR_PATH).read_bytes()),
        "command": f"python3 {GENERATOR_PATH} --root . --apply",
        "check_command": f"python3 {GENERATOR_PATH} --root . --check",
        "source_report": REPORT_PATH,
        **provenance,
        "authority_input_digests": {
            CATALOG_PATH: digest_bytes((root / CATALOG_PATH).read_bytes()),
            CODE_MAP_PATH: digest_bytes((root / CODE_MAP_PATH).read_bytes()),
        },
        "relationship_digest": relationship_digest,
        "relationship_count": len(relationships),
        "finding_count": sum(
            int(relationship["finding_count"])
            for relationship in relationships
        ),
        "relationships": relationships,
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
    failures = []
    for path, expected in outputs.items():
        target = root / path
        if not target.is_file() or target.read_bytes() != expected:
            failures.append(f"{path}: generated bytes differ")
    return failures


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve()
    try:
        outputs = build_outputs(root, applying=args.apply)
        if args.check:
            failures = validate_outputs(root, outputs)
            if failures:
                raise ValueError("; ".join(failures))
            manifest = json.loads(outputs[OUTPUT_PATH])
            print(
                "invariant test backlink batch is reproducible: "
                f"relationships={manifest['relationship_count']} "
                f"findings={manifest['finding_count']}"
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
        StopIteration,
        json.JSONDecodeError,
        tomllib.TOMLDecodeError,
    ) as error:
        print(
            f"invariant test backlink generation failed: {error}",
            file=sys.stderr,
        )
        return 1
    manifest = json.loads((root / OUTPUT_PATH).read_text())
    print(
        "invariant test backlink batch generated: "
        f"relationships={manifest['relationship_count']} "
        f"findings={manifest['finding_count']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
