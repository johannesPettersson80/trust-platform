"""Build review-only candidates for reusing existing code-spec authority."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from collections import defaultdict
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path
from typing import Any

from .code_spec_authority_contract import ORACLE_AUTHORITIES
from .code_spec_fragments import source_fragment_exists
from .gate_inventory import (
    INVENTORY_PATH as GATE_INVENTORY_PATH,
    load_gate_inventory,
    validate_gate_inventory,
)
from .gate_surface_authority_reuse import (
    INVARIANT_ID as GATE_SURFACE_INVARIANT_ID,
    build_gate_surface_authority_routes,
)
from .quarantine_paths import quarantine_path
from .test_catalog_surfaces import scan_gate_scripts


CODE_MAP_PATH = "verification/code-spec-map.toml"
SPEC_GAPS_PATH = "verification/spec-gaps.toml"
SPEC_SOURCES_PATH = "verification/spec-sources.toml"
TEST_CATALOG_PATH = "verification/test-catalog.toml"
INVARIANT_ROOT = "verification/invariants"
SPEC_REF_RE = re.compile(r"^([A-Z0-9_]+)#([A-Za-z0-9._:-]+)$")


def build_authority_reuse_proposal(
    *,
    report_path: str,
    report_bytes: bytes,
    code_map_bytes: bytes,
    spec_gaps_bytes: bytes,
    spec_sources: Mapping[str, Mapping[str, Any]],
    invariants: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
    source_bindings: Mapping[str, str],
    gate_routes: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    """Derive deterministic exact-path candidates without accepting them."""

    report = _json_object(report_bytes, "code-spec report")
    _validate_report(report)
    mappings = _toml_index(
        code_map_bytes, "mappings", "code_id", "code-spec map"
    )
    gaps = _toml_index(
        spec_gaps_bytes, "spec_gaps", "id", "spec-gap ledger"
    )
    report_facts = _report_facts(report)
    if set(report_facts) != set(mappings):
        raise ValueError("code-spec report and map identity sets differ")

    eligible: dict[str, dict[str, Any]] = {}
    for invariant_id, invariant in sorted(invariants.items()):
        candidate = _candidate_invariant(
            invariant_id=invariant_id,
            invariant=invariant,
            spec_sources=spec_sources,
            tests=tests,
        )
        if candidate is not None:
            eligible[invariant_id] = candidate
    by_path: dict[tuple[str, str], list[str]] = defaultdict(list)
    for invariant_id, invariant in eligible.items():
        for source_ref in invariant["source_refs"]:
            if _is_code_path(source_ref):
                by_path[(str(invariant["area"]), source_ref)].append(
                    invariant_id
                )

    grouped: dict[
        tuple[str, str, tuple[str, ...]], list[dict[str, Any]]
    ] = defaultdict(list)
    no_candidate: list[str] = []
    gap_facts = 0
    for code_id, mapping in sorted(mappings.items()):
        fact = report_facts[code_id]
        _validate_report_fact(mapping, fact)
        if mapping.get("disposition") != "spec_gap":
            continue
        gap_facts += 1
        gap_id = _required_string(mapping, "spec_gap_ref", code_id)
        gap = gaps.get(gap_id)
        if gap is None or gap.get("resolution_status") not in {
            "open",
            "registered_backlog",
        }:
            raise ValueError(f"{code_id}: unresolved owning gap is absent")
        area = _required_string(mapping, "area", code_id)
        path = _required_string(mapping, "path", code_id)
        route = gate_routes.get(code_id)
        route_invariant = (
            route.get("invariant_id") if isinstance(route, Mapping) else None
        )
        if route is not None and route_invariant not in eligible:
            raise ValueError(f"{code_id}: routed candidate invariant is ineligible")
        candidate_ids = tuple(
            sorted(
                set(by_path.get((area, path), []))
                | ({str(route_invariant)} if route is not None else set())
            )
        )
        if not candidate_ids:
            no_candidate.append(code_id)
            continue
        fact_row = {
                "code_id": code_id,
                "qualified_symbol": _required_string(
                    mapping, "qualified_symbol", code_id
                ),
                "item_kind": _required_string(
                    mapping, "item_kind", code_id
                ),
                "owner": _required_string(mapping, "owner", code_id),
                "risk_signals": _string_array(
                    mapping.get("risk_signals"),
                    f"{code_id} risk signals",
                    allow_empty=True,
                ),
                "spec_gap_ref": gap_id,
            }
        if route is not None:
            fact_row["authority_route"] = dict(route)
        grouped[(area, path, candidate_ids)].append(fact_row)

    candidate_groups = []
    used_invariants: set[str] = set()
    for area, path, candidate_ids in sorted(grouped):
        facts = sorted(
            grouped[(area, path, candidate_ids)],
            key=lambda row: str(row["code_id"]),
        )
        group_seed = {
            "area": area,
            "path": path,
            "candidate_invariant_ids": list(candidate_ids),
            "code_ids": [row["code_id"] for row in facts],
        }
        candidate_groups.append(
            {
                "candidate_group_id": (
                    "CODEAUTH_GROUP_"
                    + hashlib.sha256(_canonical_json(group_seed)).hexdigest().upper()
                ),
                "classification": (
                    "singular" if len(candidate_ids) == 1 else "ambiguous"
                ),
                "area": area,
                "path": path,
                "candidate_invariant_ids": list(candidate_ids),
                "facts": facts,
                "review_status": "review_required",
                "required_reviewer_roles": ["semantic_owner"],
            }
        )
        used_invariants.update(candidate_ids)

    singular_facts = sum(
        len(group["facts"])
        for group in candidate_groups
        if group["classification"] == "singular"
    )
    ambiguous_facts = sum(
        len(group["facts"])
        for group in candidate_groups
        if group["classification"] == "ambiguous"
    )
    candidate_invariants = [
        eligible[invariant_id] for invariant_id in sorted(used_invariants)
    ]
    bindings = [
        {"path": path, "sha256": digest}
        for path, digest in sorted(source_bindings.items())
    ]
    operation_plane = {
        "candidate_groups": candidate_groups,
        "candidate_invariants": candidate_invariants,
        "no_candidate_code_ids": no_candidate,
    }
    return {
        "schema_version": 1,
        "kind": "p18_code_spec_authority_reuse_proposal",
        "generator": "code-spec-authority-reuse",
        "generator_version": 1,
        "mode": "proposal_only",
        "source_commit": report["commit"],
        "source_report": {
            "path": report_path,
            "sha256": _digest(report_bytes),
            "input_digest": report["input_digest"],
        },
        "source_code_map_sha256": _digest(code_map_bytes),
        "source_spec_gaps_sha256": _digest(spec_gaps_bytes),
        "source_bindings": bindings,
        "boundaries": {
            "automatic_writes": False,
            "creates_product_proof": False,
            "closes_spec_gaps": False,
            "infers_expected_behavior": False,
            "requires_complete_semantic_review": True,
            "requires_live_reconstruction": True,
        },
        "summary": {
            "source_facts": len(mappings),
            "source_findings": len(report["findings"]),
            "spec_gap_facts": gap_facts,
            "candidate_groups": len(candidate_groups),
            "candidate_facts": singular_facts + ambiguous_facts,
            "singular_candidate_facts": singular_facts,
            "ambiguous_candidate_facts": ambiguous_facts,
            "no_candidate_facts": len(no_candidate),
        },
        "candidate_invariants": candidate_invariants,
        "candidate_groups": candidate_groups,
        "no_candidate_code_ids": no_candidate,
        "operation_set_digest": _object_digest(operation_plane),
    }


def write_authority_reuse_proposal(
    *,
    root: Path,
    report_path: Path,
    output_path: Path,
) -> dict[str, Any]:
    """Load canonical metadata, validate oracle fragments, and write a proposal."""

    root = root.resolve()
    report_relative, report_resolved = quarantine_path(
        root, report_path, label="authority-reuse source report", must_exist=True
    )
    output_relative, output_resolved = quarantine_path(
        root,
        output_path,
        label="authority-reuse proposal output",
        must_exist=False,
    )
    if report_resolved == output_resolved:
        raise ValueError("authority-reuse report and output must differ")
    proposal = _build_from_root(
        root=root,
        report_relative=report_relative,
        report_resolved=report_resolved,
    )
    encoded = _canonical_json(proposal) + b"\n"
    expected = output_resolved.read_bytes() if output_resolved.is_file() else None
    if expected is not None and expected != encoded:
        raise ValueError("authority-reuse output differs from existing artifact")
    output_resolved.parent.mkdir(parents=True, exist_ok=True)
    output_resolved.write_bytes(encoded)
    return proposal


def reconstruct_live_authority_reuse_proposal(
    root: Path, proposal_bytes: bytes
) -> dict[str, Any]:
    """Rebuild a proposal from its bound report and current canonical inputs."""

    root = root.resolve()
    proposal = _json_object(proposal_bytes, "authority-reuse proposal")
    source_report = proposal.get("source_report")
    if not isinstance(source_report, Mapping):
        raise ValueError("authority-reuse source report binding is malformed")
    report_path = _required_string(
        source_report, "path", "authority-reuse source report"
    )
    report_relative, report_resolved = quarantine_path(
        root,
        root / report_path,
        label="authority-reuse source report",
        must_exist=True,
    )
    rebuilt = _build_from_root(
        root=root,
        report_relative=report_relative,
        report_resolved=report_resolved,
    )
    if proposal != rebuilt:
        raise ValueError(
            "authority-reuse proposal differs from live reconstructed inputs"
        )
    return {
        "source_commit": rebuilt["source_commit"],
        "source_report_sha256": rebuilt["source_report"]["sha256"],
        "operation_set_digest": rebuilt["operation_set_digest"],
        "candidate_groups": rebuilt["summary"]["candidate_groups"],
        "singular_candidate_facts": rebuilt["summary"][
            "singular_candidate_facts"
        ],
        "ambiguous_candidate_facts": rebuilt["summary"][
            "ambiguous_candidate_facts"
        ],
    }


def _build_from_root(
    *,
    root: Path,
    report_relative: str,
    report_resolved: Path,
) -> dict[str, Any]:
    _require_clean_tracked_worktree(root)
    report_bytes = report_resolved.read_bytes()
    report = _json_object(report_bytes, "code-spec report")
    if report.get("commit") != _git_head(root):
        raise ValueError("authority-reuse report commit is stale")

    code_map_bytes = (root / CODE_MAP_PATH).read_bytes()
    spec_gaps_bytes = (root / SPEC_GAPS_PATH).read_bytes()
    spec_source_bytes = (root / SPEC_SOURCES_PATH).read_bytes()
    test_catalog_bytes = (root / TEST_CATALOG_PATH).read_bytes()
    spec_sources = _toml_index(
        spec_source_bytes,
        "spec_sources",
        "id",
        "spec source registry",
    )
    tests = _toml_index(
        test_catalog_bytes, "tests", "id", "test catalog"
    )
    invariants: dict[str, dict[str, Any]] = {}
    source_bindings = {
        SPEC_SOURCES_PATH: _digest(spec_source_bytes),
        TEST_CATALOG_PATH: _digest(test_catalog_bytes),
    }
    for path in sorted((root / INVARIANT_ROOT).rglob("*.toml")):
        content = path.read_bytes()
        document = tomllib.loads(content.decode())
        invariant_id = _required_string(
            document, "id", path.relative_to(root).as_posix()
        )
        if invariant_id in invariants:
            raise ValueError(f"duplicate invariant {invariant_id}")
        oracle = document.get("oracle")
        ref = oracle.get("ref") if isinstance(oracle, Mapping) else None
        match = SPEC_REF_RE.fullmatch(ref) if isinstance(ref, str) else None
        source = spec_sources.get(match.group(1)) if match is not None else None
        if source is not None and not source_fragment_exists(
            source, match.group(2), root=root
        ):
            continue
        invariants[invariant_id] = document
        source_bindings[path.relative_to(root).as_posix()] = _digest(content)

    gate_inventory = load_gate_inventory(root)
    gate_failures = validate_gate_inventory(root, gate_inventory)
    if gate_failures:
        raise ValueError(
            "gate inventory is not live-canonical: " + "; ".join(gate_failures)
        )
    gate_batch = scan_gate_scripts(root)
    gate_errors = [
        diagnostic
        for diagnostic in gate_batch.diagnostics
        if diagnostic.severity == "error"
    ]
    if gate_errors:
        raise ValueError("live gate-script discovery failed")
    gate_candidate = _candidate_invariant(
        invariant_id=GATE_SURFACE_INVARIANT_ID,
        invariant=invariants.get(GATE_SURFACE_INVARIANT_ID, {}),
        spec_sources=spec_sources,
        tests=tests,
    )
    mappings = _toml_index(
        code_map_bytes, "mappings", "code_id", "code-spec map"
    )
    gate_routes = (
        build_gate_surface_authority_routes(
            candidate_invariant=gate_candidate,
            tests=tests,
            inventory_records=gate_inventory,
            live_gate_facts=gate_batch.facts,
            mappings=mappings,
        )
        if gate_candidate is not None
        else {}
    )
    gate_inventory_bytes = (root / GATE_INVENTORY_PATH).read_bytes()
    source_bindings[GATE_INVENTORY_PATH.as_posix()] = _digest(
        gate_inventory_bytes
    )

    return build_authority_reuse_proposal(
        report_path=report_relative,
        report_bytes=report_bytes,
        code_map_bytes=code_map_bytes,
        spec_gaps_bytes=spec_gaps_bytes,
        spec_sources=spec_sources,
        invariants=invariants,
        tests=tests,
        source_bindings=source_bindings,
        gate_routes=gate_routes,
    )


def _candidate_invariant(
    *,
    invariant_id: str,
    invariant: Mapping[str, Any],
    spec_sources: Mapping[str, Mapping[str, Any]],
    tests: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any] | None:
    if invariant.get("id") != invariant_id or invariant.get("status") not in {
        "mapped",
        "implemented",
    }:
        return None
    area = invariant.get("area")
    spec = invariant.get("spec")
    oracle = invariant.get("oracle")
    spec_ref = oracle.get("ref") if isinstance(oracle, Mapping) else None
    match = SPEC_REF_RE.fullmatch(spec_ref) if isinstance(spec_ref, str) else None
    if (
        not isinstance(area, str)
        or not isinstance(spec, Mapping)
        or spec.get("status") != "specified"
        or match is None
    ):
        return None
    source_id = match.group(1)
    source_refs = spec.get("source_refs")
    source = spec_sources.get(source_id)
    if (
        not isinstance(source_refs, list)
        or source_id not in source_refs
        or not _oracle_eligible(source)
        or source.get("area") != area
    ):
        return None
    invariant_tests = _string_array(
        invariant.get("tests"), f"{invariant_id} tests", allow_empty=True
    )
    if not invariant_tests:
        return None
    for test_id in invariant_tests:
        test = tests.get(test_id)
        linked = test.get("invariants") if isinstance(test, Mapping) else None
        if (
            not isinstance(test, Mapping)
            or test.get("status") != "mapped"
            or test.get("area") != area
            or not isinstance(linked, list)
            or invariant_id not in linked
        ):
            return None
    source_paths = _string_array(
        invariant.get("source_refs"),
        f"{invariant_id} source_refs",
        allow_empty=True,
    )
    return {
        "invariant_id": invariant_id,
        "title": _required_string(invariant, "title", invariant_id),
        "claim": _required_string(invariant, "claim", invariant_id),
        "area": area,
        "risk": _required_string(invariant, "risk", invariant_id),
        "owner": _required_string(invariant, "owner", invariant_id),
        "spec_ref": spec_ref,
        "source_id": source_id,
        "source_refs": source_paths,
        "test_ids": invariant_tests,
        "behavior_summaries": list(invariant.get("behavior", [])),
    }


def _oracle_eligible(source: Mapping[str, Any] | None) -> bool:
    return bool(
        source
        and source.get("source_status") == "active"
        and source.get("oracle_eligible") is True
        and source.get("authority") in ORACLE_AUTHORITIES
    )


def _validate_report(report: Mapping[str, Any]) -> None:
    if (
        report.get("schema_version") != 1
        or report.get("generator") != "code-spec-coverage"
        or report.get("report_status") != "complete"
    ):
        raise ValueError("authority-reuse source report contract differs")
    _required_string(report, "commit", "code-spec report")
    _required_string(report, "input_digest", "code-spec report")
    findings = report.get("findings")
    if not isinstance(findings, list) or findings:
        raise ValueError("authority-reuse requires a zero-finding source report")
    summary = report.get("summary")
    facts = report.get("facts")
    if (
        not isinstance(summary, Mapping)
        or not isinstance(facts, list)
        or summary.get("facts_total") != len(facts)
        or summary.get("mapped_total") != len(facts)
        or summary.get("findings_total") != 0
    ):
        raise ValueError("authority-reuse source report counts differ")


def _report_facts(
    report: Mapping[str, Any],
) -> dict[str, Mapping[str, Any]]:
    result: dict[str, Mapping[str, Any]] = {}
    for fact in report["facts"]:
        if not isinstance(fact, Mapping):
            raise ValueError("code-spec report fact is malformed")
        code_id = _required_string(fact, "code_id", "code-spec report fact")
        if code_id in result:
            raise ValueError(f"duplicate report fact {code_id}")
        result[code_id] = fact
    return result


def _validate_report_fact(
    mapping: Mapping[str, Any], fact: Mapping[str, Any]
) -> None:
    fields = (
        ("source_kind", "source_kind"),
        ("path", "path"),
        ("qualified_symbol", "qualified_symbol"),
        ("item_kind", "item_kind"),
        ("signature_digest", "signature_digest"),
        ("reviewed_body_digest", "body_digest"),
        ("owner", "owner"),
        ("area", "area"),
        ("visibility", "visibility"),
        ("disposition", "disposition"),
    )
    code_id = str(mapping.get("code_id"))
    for mapping_field, fact_field in fields:
        if mapping.get(mapping_field) != fact.get(fact_field):
            raise ValueError(
                f"{code_id}: report fact {fact_field} differs from map"
            )
    if sorted(mapping.get("risk_signals", [])) != sorted(
        fact.get("risk_signals", [])
    ):
        raise ValueError(f"{code_id}: report risk signals differ from map")
    if mapping.get("disposition") == "spec_gap" and mapping.get(
        "spec_gap_ref"
    ) != fact.get("disposition_ref"):
        raise ValueError(f"{code_id}: report gap reference differs from map")


def _toml_index(
    content: bytes, array: str, field: str, label: str
) -> dict[str, dict[str, Any]]:
    document = tomllib.loads(content.decode())
    rows = document.get(array)
    if not isinstance(rows, list):
        raise ValueError(f"{label} {array} array is absent")
    result: dict[str, dict[str, Any]] = {}
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError(f"{label} row is malformed")
        identifier = _required_string(row, field, label)
        if identifier in result:
            raise ValueError(f"{label} duplicates {identifier}")
        result[identifier] = row
    return result


def _is_code_path(value: str) -> bool:
    return value.startswith(
        ("crates/", "editors/", "scripts/", "xtask/", ".github/", ".codex/")
    )


def _json_object(content: bytes, label: str) -> dict[str, Any]:
    value = json.loads(content)
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a JSON object")
    return value


def _required_string(
    row: Mapping[str, Any], field: str, label: str
) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires non-empty {field}")
    return value


def _string_array(
    value: Any, label: str, *, allow_empty: bool = False
) -> list[str]:
    if (
        not isinstance(value, list)
        or (not value and not allow_empty)
        or any(not isinstance(item, str) or not item for item in value)
        or len(value) != len(set(value))
    ):
        raise ValueError(f"{label} must be a unique string array")
    return list(value)


def _digest(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


def _canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()


def _object_digest(value: Any) -> str:
    return _digest(_canonical_json(value))


def _git_head(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def _require_clean_tracked_worktree(root: Path) -> None:
    completed = subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=no"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    if completed.stdout:
        raise ValueError(
            "authority-reuse live reconstruction requires a clean tracked "
            "worktree"
        )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Generate review-only existing-authority reuse candidates."
    )
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        proposal = write_authority_reuse_proposal(
            root=args.root,
            report_path=args.report,
            output_path=args.output,
        )
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(
            f"code-spec authority-reuse proposal failed: {error}",
            file=sys.stderr,
        )
        return 1
    summary = proposal["summary"]
    print(
        "code-spec authority-reuse proposal generated: "
        f"gap_facts={summary['spec_gap_facts']} "
        f"singular={summary['singular_candidate_facts']} "
        f"ambiguous={summary['ambiguous_candidate_facts']} "
        f"no_candidate={summary['no_candidate_facts']} "
        f"groups={summary['candidate_groups']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
