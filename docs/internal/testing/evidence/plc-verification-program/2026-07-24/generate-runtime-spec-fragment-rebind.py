#!/usr/bin/env python3
"""Rebind two renamed runtime-spec fragments and their generated review IDs."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import sys
import tomllib
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[6]
sys.path.insert(0, str(REPO_ROOT))

from scripts.verification.mapping_review import stable_decision_id  # noqa: E402
from scripts.verification.mapping_toml import render_decisions_toml  # noqa: E402
from scripts.verification.mapping_transaction import (  # noqa: E402
    apply_mapping_transaction,
)


DECISIONS_PATH = "verification/test-catalog-mapping-decisions.toml"
GENERATOR_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "generate-runtime-spec-fragment-rebind.py"
)
OUTPUT_PATH = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-24/"
    "runtime-spec-fragment-rebind.json"
)
TARGET_PATHS = (
    "verification/code-spec-map.toml",
    "verification/code-spec-risk-acknowledgements.toml",
    "verification/invariants/runtime_safety/RT_ADS_CLI_AUTHORING_001.toml",
    "verification/invariants/runtime_safety/RT_ADS_CLI_FAIL_CLOSED_001.toml",
    "verification/invariants/runtime_safety/RT_ADS_CLI_ROUTE_SECURITY_001.toml",
    "verification/invariants/runtime_safety/RT_OFFLINE_COMM_AUTHORING_CLI_001.toml",
    "verification/malformed-input-taxonomies/runtime_offline_comm_authoring.md",
    "verification/malformed-input-taxonomies/runtime_offline_comm_authoring.toml",
    "verification/test-catalog-mapping-decisions.toml",
    "verification/test-catalog.toml",
)
REBINDINGS = {
    (
        "SPEC_RUNTIME_ENGINE_001"
        "#6-8-1a-ads-cli-authoring-and-server-control-contract"
    ): (
        "SPEC_RUNTIME_ENGINE_001"
        "#6-8-1b-ads-cli-authoring-and-server-control-contract"
    ),
    (
        "SPEC_RUNTIME_ENGINE_001"
        "#offline-communication-authoring-client"
    ): (
        "SPEC_RUNTIME_ENGINE_001"
        "#6-8-1a-offline-communication-authoring-client"
    ),
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


def digest_record(value: dict[str, Any]) -> str:
    encoded = json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()
    return digest_bytes(encoded)


def replace_exact_refs(value: Any) -> Any:
    if isinstance(value, str):
        return REBINDINGS.get(value, value)
    if isinstance(value, list):
        return [replace_exact_refs(item) for item in value]
    if isinstance(value, dict):
        return {key: replace_exact_refs(item) for key, item in value.items()}
    return value


def update_decision_document(content: bytes) -> tuple[bytes, int, int]:
    document = tomllib.loads(content.decode())
    updated = copy.deepcopy(document)
    snapshots = updated.get("proposal_snapshots")
    decisions = updated.get("decisions")
    if not isinstance(snapshots, list) or not isinstance(decisions, list):
        raise ValueError("mapping decision document is malformed")
    changed_snapshots: dict[str, dict[str, str]] = {}
    for snapshot in snapshots:
        if not isinstance(snapshot, dict):
            raise ValueError("proposal snapshot is not an object")
        old_id = str(snapshot.get("id", ""))
        rebound = replace_exact_refs(snapshot)
        if rebound == snapshot:
            continue
        semantic = {
            key: value
            for key, value in rebound.items()
            if key not in {"id", "proposal_digest"}
        }
        proposal_digest = digest_record(semantic)
        proposal_id = (
            f"MAP_CAND_{semantic['discovery_id'].removeprefix('DISC_')}_"
            f"{proposal_digest.removeprefix('sha256:')[:12].upper()}"
        )
        snapshot.clear()
        snapshot.update(
            {
                **semantic,
                "id": proposal_id,
                "proposal_digest": proposal_digest,
            }
        )
        changed_snapshots[old_id] = {
            "id": proposal_id,
            "proposal_digest": proposal_digest,
        }
    changed_decisions = 0
    for decision in decisions:
        if not isinstance(decision, dict):
            raise ValueError("mapping decision is not an object")
        replacement = changed_snapshots.get(str(decision.get("proposal_id", "")))
        if replacement is None:
            continue
        decision["proposal_id"] = replacement["id"]
        decision["proposal_digest"] = replacement["proposal_digest"]
        decision["id"] = stable_decision_id(decision)
        changed_decisions += 1
    if changed_decisions != len(changed_snapshots):
        raise ValueError(
            "proposal snapshot/decision rebind is not one-to-one: "
            f"snapshots={len(changed_snapshots)} decisions={changed_decisions}"
        )
    return (
        render_decisions_toml(updated).encode(),
        len(changed_snapshots),
        changed_decisions,
    )


def target_mapping_rows(
    decision_content: bytes,
) -> tuple[set[str], dict[str, tuple[dict[str, Any], dict[str, Any]]]]:
    document = tomllib.loads(decision_content.decode())
    snapshots = document.get("proposal_snapshots")
    decisions = document.get("decisions")
    if not isinstance(snapshots, list) or not isinstance(decisions, list):
        raise ValueError("mapping decision document is malformed")
    new_refs = set(REBINDINGS.values())
    target_snapshots = [
        snapshot
        for snapshot in snapshots
        if isinstance(snapshot, dict)
        and new_refs.intersection(snapshot.get("oracle_refs", []))
    ]
    if not target_snapshots:
        raise ValueError("runtime fragment rebind found no target snapshots")
    snapshots_by_id = {
        str(snapshot["id"]): snapshot for snapshot in target_snapshots
    }
    target_discovery_ids = {
        str(snapshot["discovery_id"]) for snapshot in target_snapshots
    }
    if (
        len(snapshots_by_id) != len(target_snapshots)
        or len(target_discovery_ids) != len(target_snapshots)
    ):
        raise ValueError("runtime fragment target snapshots are not unique")
    target_decisions = [
        decision
        for decision in decisions
        if isinstance(decision, dict)
        and decision.get("discovery_id") in target_discovery_ids
    ]
    decisions_by_discovery: dict[str, dict[str, Any]] = {}
    decision_proposal_ids: set[str] = set()
    for decision in target_decisions:
        discovery_id = str(decision.get("discovery_id", ""))
        proposal_id = str(decision.get("proposal_id", ""))
        if discovery_id in decisions_by_discovery:
            raise ValueError(
                f"runtime fragment decision discovery is duplicated: "
                f"{discovery_id}"
            )
        snapshot = snapshots_by_id.get(proposal_id)
        if snapshot is None:
            raise ValueError("runtime fragment decision lacks its snapshot")
        if snapshot.get("discovery_id") != discovery_id:
            raise ValueError(
                "runtime fragment decision/snapshot discovery mismatch"
            )
        decisions_by_discovery[discovery_id] = decision
        decision_proposal_ids.add(proposal_id)
    if (
        set(decisions_by_discovery) != target_discovery_ids
        or decision_proposal_ids != set(snapshots_by_id)
        or len(target_decisions) != len(target_snapshots)
    ):
        raise ValueError("runtime fragment decisions are not one-to-one")
    accepted: dict[str, tuple[dict[str, Any], dict[str, Any]]] = {}
    for decision in target_decisions:
        proposal_id = str(decision.get("proposal_id", ""))
        snapshot = snapshots_by_id.get(proposal_id)
        assert snapshot is not None
        if decision.get("outcome") == "accepted":
            accepted[str(decision["discovery_id"])] = (snapshot, decision)
    return target_discovery_ids, accepted


def validate_pairing_self_check(decision_content: bytes) -> None:
    document = tomllib.loads(decision_content.decode())
    target_ids, _ = target_mapping_rows(decision_content)
    decisions = document.get("decisions")
    if not isinstance(decisions, list):
        raise ValueError("mapping decision document is malformed")
    indexes = [
        index
        for index, decision in enumerate(decisions)
        if isinstance(decision, dict)
        and decision.get("discovery_id") in target_ids
    ]
    if len(indexes) < 2:
        raise ValueError("pairing self-check requires two target decisions")
    malformed_documents = []
    duplicate = copy.deepcopy(document)
    duplicate["decisions"][indexes[1]] = copy.deepcopy(
        duplicate["decisions"][indexes[0]]
    )
    malformed_documents.append(duplicate)
    mismatched = copy.deepcopy(document)
    first = mismatched["decisions"][indexes[0]]
    second = mismatched["decisions"][indexes[1]]
    first["proposal_id"], second["proposal_id"] = (
        second["proposal_id"],
        first["proposal_id"],
    )
    malformed_documents.append(mismatched)
    for malformed in malformed_documents:
        try:
            target_mapping_rows(render_decisions_toml(malformed).encode())
        except ValueError:
            continue
        raise ValueError(
            "runtime fragment decision pairing self-check accepted "
            "a malformed graph"
        )


def update_catalog_bindings(
    content: bytes,
    *,
    target_discovery_ids: set[str],
    accepted: dict[str, tuple[dict[str, Any], dict[str, Any]]],
) -> bytes:
    source = content.decode()
    marker = "\n[[tests]]\n"
    parts = source.split(marker)
    updated_parts = [parts[0]]
    seen_targets: set[str] = set()
    for body in parts[1:]:
        parsed = tomllib.loads("[[tests]]\n" + body)
        rows = parsed.get("tests")
        if not isinstance(rows, list) or len(rows) != 1:
            raise ValueError("catalog test block is malformed")
        row = rows[0]
        discovery_id = str(row.get("discovery_id", ""))
        if discovery_id in target_discovery_ids:
            seen_targets.add(discovery_id)
            binding = accepted.get(discovery_id)
            if binding is None:
                raise ValueError(
                    f"rejected runtime fragment proposal {discovery_id} "
                    "unexpectedly has a catalog row"
                )
            snapshot, decision = binding
            expected = {
                "mapping_proposal_id": str(snapshot["id"]),
                "mapping_proposal_digest": str(snapshot["proposal_digest"]),
                "mapping_decision_id": str(decision["id"]),
            }
            for field, value in expected.items():
                pattern = rf'(?m)^{field} = "[^"]*"$'
                body, count = re.subn(
                    pattern,
                    f"{field} = {json.dumps(value)}",
                    body,
                    count=1,
                )
                if count != 1:
                    raise ValueError(
                        f"catalog row {discovery_id} lacks {field}"
                    )
        updated_parts.append(body)
    if seen_targets != set(accepted):
        raise ValueError(
            "accepted runtime fragment catalog binding drift: "
            f"seen={sorted(seen_targets)} expected={sorted(accepted)}"
        )
    updated = marker.join(updated_parts).encode()
    parsed_updated = tomllib.loads(updated.decode())
    rows_by_discovery = {
        str(row.get("discovery_id", "")): row
        for row in parsed_updated.get("tests", [])
        if isinstance(row, dict)
    }
    for discovery_id, (snapshot, decision) in accepted.items():
        row = rows_by_discovery.get(discovery_id)
        if row is None:
            raise ValueError(f"catalog lost {discovery_id}")
        if (
            row.get("mapping_proposal_id") != snapshot.get("id")
            or row.get("mapping_proposal_digest")
            != snapshot.get("proposal_digest")
            or row.get("mapping_decision_id") != decision.get("id")
        ):
            raise ValueError(f"catalog binding did not update for {discovery_id}")
    return updated


def prior_replacement_counts(root: Path) -> dict[str, int]:
    manifest_path = root / OUTPUT_PATH
    if not manifest_path.is_file():
        raise ValueError("already-rebound input lacks its generated manifest")
    manifest = json.loads(manifest_path.read_text())
    rows = manifest.get("rebindings")
    if not isinstance(rows, list):
        raise ValueError("runtime fragment manifest lacks rebindings")
    counts: dict[str, int] = {}
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError("runtime fragment manifest rebind is malformed")
        old = row.get("old")
        new = row.get("new")
        count = row.get("replacements")
        if (
            not isinstance(old, str)
            or REBINDINGS.get(old) != new
            or not isinstance(count, int)
            or isinstance(count, bool)
            or count <= 0
        ):
            raise ValueError("runtime fragment manifest rebind drift")
        counts[old] = count
    if set(counts) != set(REBINDINGS):
        raise ValueError("runtime fragment manifest rebind set drift")
    return counts


def build_outputs(root: Path, *, applying: bool) -> dict[str, bytes]:
    source = {path: (root / path).read_bytes() for path in TARGET_PATHS}
    observed_counts = {
        old: sum(content.count(old.encode()) for content in source.values())
        for old in REBINDINGS
    }
    has_old_refs = any(observed_counts.values())
    if has_old_refs and (
        not applying or any(count <= 0 for count in observed_counts.values())
    ):
        raise ValueError(
            f"runtime spec fragment occurrence drift: "
            f"observed={observed_counts}"
        )
    replacement_counts = (
        observed_counts if has_old_refs else prior_replacement_counts(root)
    )
    outputs: dict[str, bytes] = {}
    for path, content in source.items():
        if path == DECISIONS_PATH:
            (
                outputs[path],
                _,
                _,
            ) = update_decision_document(content)
            continue
        updated = content
        for old, new in REBINDINGS.items():
            updated = updated.replace(old.encode(), new.encode())
        outputs[path] = updated
    target_ids, accepted = target_mapping_rows(outputs[DECISIONS_PATH])
    outputs["verification/test-catalog.toml"] = update_catalog_bindings(
        outputs["verification/test-catalog.toml"],
        target_discovery_ids=target_ids,
        accepted=accepted,
    )
    manifest = {
        "schema_version": 1,
        "batch_id": "RUNTIME_SPEC_FRAGMENT_REBIND_20260724",
        "generator": GENERATOR_PATH,
        "generator_sha256": digest_bytes((root / GENERATOR_PATH).read_bytes()),
        "command": f"python3 {GENERATOR_PATH} --root . --apply",
        "check_command": f"python3 {GENERATOR_PATH} --root . --check",
        "rebindings": [
            {
                "old": old,
                "new": new,
                "replacements": replacement_counts[old],
            }
            for old, new in REBINDINGS.items()
        ],
        "target_paths": list(TARGET_PATHS),
        "target_discovery_ids": sorted(target_ids),
        "accepted_catalog_discovery_ids": sorted(accepted),
        "changed_proposal_snapshots": len(target_ids),
        "changed_decisions": len(target_ids),
        "changed_catalog_bindings": len(accepted),
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
    for old in REBINDINGS:
        for path in TARGET_PATHS:
            if old.encode() in (root / path).read_bytes():
                failures.append(f"{path}: stale fragment remains")
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
            validate_pairing_self_check(outputs[DECISIONS_PATH])
            print("runtime spec fragment rebind is reproducible")
            return 0
        apply_mapping_transaction(
            root=root,
            replacements=outputs,
            validate=lambda: validate_outputs(root, outputs),
        )
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"runtime spec fragment rebind failed: {error}", file=sys.stderr)
        return 1
    print(
        "runtime spec fragment rebind generated: "
        f"references={sum(prior_replacement_counts(root).values())}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
