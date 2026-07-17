"""Explicit, report-only alignment analysis for the conformance corpus."""

from __future__ import annotations

import fnmatch
import hashlib
import json
import re
import subprocess
import tomllib
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .conformance_semantic_oracles import (
    catalog_semantic_oracle,
    unresolved_v2_oracle_gaps,
)
from .test_catalog_surfaces import scan_conformance
from .test_catalog_rust import sanitize_rust


V1_CATEGORIES = (
    "timers",
    "edges",
    "scan_cycle",
    "init_reset",
    "arithmetic",
    "memory_map",
)
V2_CATEGORIES = (
    "strings",
    "arrays",
    "structs",
    "enums",
    "nested_values",
    "oop_dispatch",
    "references",
    "retain_matrix",
    "scheduler",
    "comms_determinism",
)
ALL_CATEGORIES = V1_CATEGORIES + V2_CATEGORIES

EXPECTED_CASE_COUNTS = {
    "timers": 3,
    "edges": 1,
    "scan_cycle": 1,
    "init_reset": 2,
    "arithmetic": 2,
    "memory_map": 2,
    **{category: 1 for category in V2_CATEGORIES},
}

CONTRACT_SOURCE_ID = "SPEC_CONFORMANCE_CONTRACT_001"
CONTRACT_PATH = "conformance/contract.md"
CONTRACT_COVERS = (
    "conformance_categories",
    "summary_profiles",
    "result_classification",
    "deterministic_ordering",
    "generated_report_artifact_policy",
)
PUBLIC_PAGE_PATH = "docs/public/reference/conformance.md"
WORKFLOW_PATH = ".github/workflows/ci.yml"
EXECUTION_PATH = "crates/trust-runtime/src/bin/trust-runtime/conformance/execution.rs"
RUNNER_REVIEWED_SOURCE_PATHS = (
    "crates/trust-runtime/src/bin/trust-runtime/conformance.rs",
    "crates/trust-runtime/src/bin/trust-runtime/conformance/discovery.rs",
    EXECUTION_PATH,
    "crates/trust-runtime/src/bin/trust-runtime/conformance/models.rs",
    "crates/trust-runtime/src/bin/trust-runtime/conformance/runner.rs",
    "crates/trust-runtime/src/bin/trust-runtime/conformance/series_values.rs",
    "crates/trust-runtime/src/bin/trust-runtime/conformance/tests.rs",
    "crates/trust-runtime/src/bin/trust-runtime/conformance/time_utils.rs",
)
RUNNER_REVIEWED_BEHAVIORS = (
    "category_profile_order",
    "default_program_source",
    "case_id_order",
    "expected_artifact_comparison",
    "case_status_classification",
    "summary_emission",
)
COMMS_REVIEWED_SOURCE_PATHS = (
    RUNNER_REVIEWED_SOURCE_PATHS[0],
    EXECUTION_PATH,
    "crates/trust-runtime/src/connectors/mapping.rs",
)
REPORT_KEEP_PATH = "conformance/reports/.gitkeep"
GITIGNORE_PATH = ".gitignore"
PUBLIC_PAGE_REVIEWED_DIGEST = (
    "sha256:c01090a72714efc747060c0a9564dfeae4d5ceca6e1e73c737f325ff331803c3"
)
RUNNER_REVIEWED_SOURCE_DIGEST = (
    "sha256:e792eef817eb9f0ed5b43e56e4525b22415295e5c12bd2ad2e0e5399bf598051"
)
COMMS_REVIEWED_SOURCE_DIGEST = (
    "sha256:3af615cb312d64ce9152ab0fbd35b48f77f9fd13b03ffafe88ed8d8a2dd5d5ca"
)
CI_JOB_REVIEWED_DIGEST = (
    "sha256:687db5f20cd1eef8f6e6cecc1b923a23c5ec5e14b92d9d1c3e90b4cfdf4ac96f"
)

_COMMON_MANIFEST_FIELDS = {"id", "category", "description", "kind"}
_RUNTIME_MANIFEST_FIELDS = {
    "cycles",
    "sources",
    "watch_globals",
    "watch_direct",
    "advance_ms",
    "input_series",
    "direct_input_series",
    "restarts",
}
_CONNECTOR_MANIFEST_FIELDS = {"connector_status_steps"}
_NETWORK_FIELD_PARTS = ("host", "port", "endpoint", "broker", "socket", "network")
_NETWORK_SOURCE_TOKENS = (
    "std::net",
    "tokio::net",
    "TcpListener",
    "TcpStream",
    "UdpSocket",
    "socket2",
    "reqwest",
    "tonic::transport",
    ".connect(",
)
_REVIEWED_COMMS_CALL_PATH = (
    "execute_case",
    "execute_connector_status_trace_case",
    "project_connector_status_step",
)


def analyze_conformance_alignment(
    root: Path,
    *,
    tests: Mapping[str, Mapping[str, Any]],
    spec_sources: Mapping[str, Mapping[str, Any]],
    tracked_report_paths: Sequence[str],
) -> dict[str, Any]:
    """Return only explicit catalog mappings and mechanically checked corpus facts."""

    root = root.resolve()
    contract = _analyze_contract(root, spec_sources)
    publication = _analyze_publication(root, tracked_report_paths)

    batch = scan_conformance(root)
    diagnostic_messages = [
        f"{item.kind} at {item.path}:{item.line}: {item.message}"
        for item in batch.diagnostics
    ]
    if diagnostic_messages:
        raise ValueError("conformance scanner diagnostics: " + "; ".join(diagnostic_messages))

    facts = sorted(batch.facts, key=lambda fact: fact.name)
    if len(facts) != 21:
        raise ValueError(f"conformance corpus must contain exactly 21 cases, found {len(facts)}")

    catalog_by_discovery_id = _catalog_join(tests)
    expected_artifacts = _load_expected_artifacts(root)
    cases: list[dict[str, Any]] = []
    manifests: dict[str, Mapping[str, Any]] = {}
    for fact in facts:
        row, manifest = _case_row(
            root,
            fact=fact,
            catalog_record=catalog_by_discovery_id.get(fact.stable_id),
            expected_artifacts=expected_artifacts,
            spec_sources=spec_sources,
        )
        cases.append(row)
        manifests[row["case_id"]] = manifest

    case_ids = {row["case_id"] for row in cases}
    orphan_expected = sorted(set(expected_artifacts) - case_ids)
    if orphan_expected:
        raise ValueError(f"orphan expected artifacts: {', '.join(orphan_expected)}")

    counts = Counter(row["category"] for row in cases)
    if dict(counts) != EXPECTED_CASE_COUNTS:
        raise ValueError(
            "conformance category census drift: "
            f"expected {EXPECTED_CASE_COUNTS}, found {dict(counts)}"
        )

    categories = [_category_row(category, cases) for category in ALL_CATEGORIES]
    gaps = unresolved_v2_oracle_gaps(V2_CATEGORIES, cases)
    linked = [row for row in cases if row["invariant_ids"]]
    unlinked = [row["case_id"] for row in cases if not row["invariant_ids"]]
    kind_counts = Counter(row["kind"] for row in cases)

    return {
        "summary": {
            "categories": len(categories),
            "v1_categories": len(V1_CATEGORIES),
            "v2_categories": len(V2_CATEGORIES),
            "cases": len(cases),
            "v1_cases": sum(row["profile"] == "v1" for row in cases),
            "v2_cases": sum(row["profile"] == "v2" for row in cases),
            "runtime_cases": kind_counts["runtime"],
            "compile_error_cases": kind_counts["compile_error"],
            "connector_status_trace_cases": kind_counts["connector_status_trace"],
            "program_sources": sum(row["program_path"] is not None for row in cases),
            "expected_artifacts": len(expected_artifacts),
            "missing_expected_artifacts": 0,
            "orphan_expected_artifacts": 0,
            "explicitly_linked_cases": len(linked),
            "unlinked_cases": len(unlinked),
            "coverage_gaps": len(gaps),
        },
        "categories": categories,
        "cases": cases,
        "coverage_gaps": gaps,
        "unlinked_case_ids": unlinked,
        "contract": contract,
        "comms_determinism": _analyze_comms(root, cases, manifests),
        "publication": publication,
    }


def _case_row(
    root: Path,
    *,
    fact: Any,
    catalog_record: Mapping[str, Any] | None,
    expected_artifacts: Mapping[str, tuple[Path, Mapping[str, Any]]],
    spec_sources: Mapping[str, Mapping[str, Any]],
) -> tuple[dict[str, Any], Mapping[str, Any]]:
    if fact.source_kind != "conformance_case" or fact.package != "trust-runtime":
        raise ValueError(f"unexpected conformance scanner fact for {fact.name}")
    case_id = _required_string(fact.name, "scanner case name")
    manifest_path = _required_string(fact.path, f"{case_id} scanner path")
    manifest_parts = Path(manifest_path).parts
    if len(manifest_parts) != 5 or manifest_parts[:2] != ("conformance", "cases"):
        raise ValueError(f"{case_id} manifest path has invalid shape: {manifest_path}")
    path_category, path_case_id, filename = manifest_parts[2:]
    if filename != "manifest.toml" or path_case_id != case_id:
        raise ValueError(f"{case_id} manifest path does not match its case id")

    manifest_file = _contained_file(root, manifest_path)
    try:
        manifest = tomllib.loads(manifest_file.read_text())
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as exc:
        raise ValueError(f"cannot parse {manifest_path}: {exc}") from exc
    if manifest.get("id") != case_id:
        raise ValueError(f"manifest id does not match path for {case_id}")
    category = _required_string(manifest.get("category"), f"{case_id} category")
    if category not in ALL_CATEGORIES:
        raise ValueError(f"{case_id} uses unknown conformance category {category}")
    if category != path_category:
        raise ValueError(f"{case_id} manifest category does not match path category")

    kind = manifest.get("kind", "runtime")
    if kind not in {"runtime", "compile_error", "connector_status_trace"}:
        raise ValueError(f"{case_id} uses unknown conformance kind {kind!r}")
    _validate_manifest_fields(case_id, kind, manifest)

    program_path = f"conformance/cases/{category}/{case_id}/program.st"
    program_file = root / program_path
    if kind == "connector_status_trace":
        if program_file.exists():
            raise ValueError(f"{case_id} connector status trace forbids a program source")
        program_path_value: str | None = None
        program_digest: str | None = None
    else:
        sources = manifest.get("sources", ["program.st"])
        if sources != ["program.st"]:
            raise ValueError(f"{case_id} source list must resolve only program.st")
        program_file = _contained_file(root, program_path)
        program_path_value = program_path
        program_digest = _digest(program_file)

    expected_entry = expected_artifacts.get(case_id)
    if expected_entry is None:
        raise ValueError(f"missing expected artifact for {case_id}")
    expected_file, expected = expected_entry
    expected_path = expected_file.relative_to(root).as_posix()
    required_expected_path = f"conformance/expected/{category}/{case_id}.json"
    if expected_path != required_expected_path:
        raise ValueError(f"expected artifact path mismatch for {case_id}")
    _validate_expected(case_id, category, kind, manifest, expected)

    catalog_test_id: str | None = None
    invariant_ids: list[str] = []
    if catalog_record is not None:
        catalog_test_id = _required_string(catalog_record.get("id"), "catalog test id")
        expected_identity = {
            "discovery_source_kind": fact.source_kind,
            "path": fact.path,
            "name": fact.name,
        }
        for field, expected in expected_identity.items():
            if catalog_record.get(field) != expected:
                raise ValueError(
                    f"{catalog_test_id} catalog identity {field} must equal {expected!r}"
                )
        raw_invariants = catalog_record.get("invariants", [])
        if not isinstance(raw_invariants, list) or not all(
            isinstance(item, str) and item for item in raw_invariants
        ):
            raise ValueError(f"{catalog_test_id} invariants must be a string array")
        if len(raw_invariants) != len(set(raw_invariants)):
            raise ValueError(f"{catalog_test_id} invariants contain duplicates")
        invariant_ids = list(raw_invariants)

    profile = "v1" if category in V1_CATEGORIES else "v2"
    oracle_ref, expected_result = catalog_semantic_oracle(
        case_id=case_id,
        profile=profile,
        catalog_record=catalog_record,
        spec_sources=spec_sources,
    )

    return (
        {
            "discovery_id": fact.stable_id,
            "case_id": case_id,
            "category": category,
            "profile": profile,
            "kind": kind,
            "manifest_path": manifest_path,
            "manifest_digest": _digest(manifest_file),
            "program_path": program_path_value,
            "program_digest": program_digest,
            "expected_artifact_path": expected_path,
            "expected_artifact_digest": _digest(expected_file),
            "catalog_test_id": catalog_test_id,
            "invariant_ids": invariant_ids,
            "oracle_ref": oracle_ref,
            "expected_result": expected_result,
        },
        manifest,
    )


def _catalog_join(
    tests: Mapping[str, Mapping[str, Any]],
) -> dict[str, Mapping[str, Any]]:
    joined: dict[str, Mapping[str, Any]] = {}
    for index_id, record in tests.items():
        if not isinstance(record, Mapping):
            raise ValueError(f"catalog record {index_id} must be a table")
        if record.get("subject_kind") != "generated_test":
            continue
        discovery_id = record.get("discovery_id")
        if not isinstance(discovery_id, str) or not discovery_id:
            continue
        if discovery_id in joined:
            raise ValueError(f"duplicate catalog discovery_id {discovery_id}")
        joined[discovery_id] = record
    return joined


def _load_expected_artifacts(
    root: Path,
) -> dict[str, tuple[Path, Mapping[str, Any]]]:
    expected_root = root / "conformance/expected"
    if not expected_root.is_dir():
        raise ValueError("required conformance expected-artifact root is missing")
    artifacts: dict[str, tuple[Path, Mapping[str, Any]]] = {}
    for path in sorted(expected_root.rglob("*.json")):
        _require_contained(root, path)
        if path.is_symlink():
            raise ValueError(f"conformance expected artifact must not be a symlink: {path}")
        try:
            payload = json.loads(path.read_text())
        except (OSError, UnicodeError, json.JSONDecodeError) as exc:
            raise ValueError(f"cannot parse expected artifact {path}: {exc}") from exc
        if not isinstance(payload, dict):
            raise ValueError(f"expected artifact {path} must contain an object")
        case_id = _required_string(payload.get("case_id"), f"expected artifact {path}")
        if case_id in artifacts:
            raise ValueError(f"duplicate expected artifact for {case_id}")
        artifacts[case_id] = (path, payload)
    if len(artifacts) != 21:
        raise ValueError(
            f"conformance corpus must contain exactly 21 expected artifacts, found {len(artifacts)}"
        )
    return artifacts


def _validate_expected(
    case_id: str,
    category: str,
    kind: str,
    manifest: Mapping[str, Any],
    expected: Mapping[str, Any],
) -> None:
    for field, value in (("case_id", case_id), ("category", category), ("kind", kind)):
        if expected.get(field) != value:
            raise ValueError(f"expected artifact {field} mismatch for {case_id}")
    if expected.get("version") != 1:
        raise ValueError(f"expected artifact version mismatch for {case_id}")
    if "description" in manifest and expected.get("description") != manifest["description"]:
        raise ValueError(f"expected artifact description mismatch for {case_id}")
    if kind == "runtime" and expected.get("cycles") != manifest.get("cycles"):
        raise ValueError(f"expected artifact cycle count mismatch for {case_id}")
    if kind == "connector_status_trace":
        trace = expected.get("trace")
        steps = manifest.get("connector_status_steps")
        if not isinstance(trace, list) or not isinstance(steps, list) or len(trace) != len(steps):
            raise ValueError(f"expected artifact trace length mismatch for {case_id}")


def _validate_manifest_fields(
    case_id: str,
    kind: str,
    manifest: Mapping[str, Any],
) -> None:
    for key in _nested_keys(manifest):
        lowered = key.lower()
        if any(part in lowered for part in _NETWORK_FIELD_PARTS):
            raise ValueError(f"{case_id} declares forbidden network field {key}")
    allowed = _COMMON_MANIFEST_FIELDS | _RUNTIME_MANIFEST_FIELDS
    if kind == "connector_status_trace":
        allowed = _COMMON_MANIFEST_FIELDS | _CONNECTOR_MANIFEST_FIELDS
    unknown = sorted(set(manifest) - allowed)
    if unknown:
        raise ValueError(f"{case_id} manifest has unsupported fields: {', '.join(unknown)}")
    if kind == "connector_status_trace":
        steps = manifest.get("connector_status_steps")
        if not isinstance(steps, list) or not steps:
            raise ValueError(f"{case_id} connector status trace needs scripted steps")
        permitted_step_fields = {
            "source",
            "state",
            "degraded_points",
            "error_policy",
            "expected_state",
            "expected_health",
            "detail",
        }
        for index, step in enumerate(steps, start=1):
            if not isinstance(step, Mapping):
                raise ValueError(f"{case_id} connector step {index} must be a table")
            unknown_step = sorted(set(step) - permitted_step_fields)
            if unknown_step:
                raise ValueError(
                    f"{case_id} connector step {index} has unsupported fields: "
                    + ", ".join(unknown_step)
                )


def _category_row(category: str, cases: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    selected = [row for row in cases if row["category"] == category]
    linked = sum(bool(row["invariant_ids"]) for row in selected)
    return {
        "category": category,
        "profile": "v1" if category in V1_CATEGORIES else "v2",
        "case_count": len(selected),
        "expected_artifact_count": len(selected),
        "linked_case_count": linked,
        "unlinked_case_count": len(selected) - linked,
        "case_ids": [row["case_id"] for row in selected],
    }


def _coverage_gap(category: str, cases: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    gaps = unresolved_v2_oracle_gaps((category,), cases)
    if not gaps:
        raise ValueError(f"{category} has no semantic-oracle coverage gap")
    return gaps[0]


def _analyze_contract(
    root: Path,
    spec_sources: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    source = spec_sources.get(CONTRACT_SOURCE_ID)
    if not isinstance(source, Mapping):
        raise ValueError(f"missing registered conformance source {CONTRACT_SOURCE_ID}")
    expected_fields = {
        "id": CONTRACT_SOURCE_ID,
        "path": CONTRACT_PATH,
        "area": "release",
        "owner": "verification",
        "status": "mapped",
        "authority": "normative_product",
        "source_status": "active",
        "oracle_eligible": False,
        "visibility": "public",
        "covers": list(CONTRACT_COVERS),
    }
    for field, expected in expected_fields.items():
        if source.get(field) != expected:
            raise ValueError(f"{CONTRACT_SOURCE_ID} {field} must be {expected!r}")
    contract_file = _contained_file(root, CONTRACT_PATH)
    text = contract_file.read_text()
    if _markdown_categories(text, "### v1 Categories", "### v2 Categories") != V1_CATEGORIES:
        raise ValueError("written v1 conformance category contract drifted")
    if _markdown_categories(text, "### v2 Categories", "## Pass/Fail Rules") != V2_CATEGORIES:
        raise ValueError("written v2 conformance category contract drifted")
    runner_digest = _validate_runner_source_contract(root)
    _validate_public_page(root)
    return {
        "spec_source_id": CONTRACT_SOURCE_ID,
        "path": CONTRACT_PATH,
        "area": "release",
        "owner": "verification",
        "metadata_status": "mapped",
        "covers": list(CONTRACT_COVERS),
        "digest": _digest(contract_file),
        "tracked": _is_tracked_or_fixture(root, CONTRACT_PATH),
        "visibility": "public",
        "authority": "normative_product",
        "oracle_eligible": False,
        "public_page_bound": True,
        "reviewed_runner_source_paths": list(RUNNER_REVIEWED_SOURCE_PATHS),
        "reviewed_runner_source_digest": runner_digest,
        "reviewed_runner_behaviors": list(RUNNER_REVIEWED_BEHAVIORS),
    }


def _analyze_comms(
    root: Path,
    cases: Sequence[Mapping[str, Any]],
    manifests: Mapping[str, Mapping[str, Any]],
) -> dict[str, Any]:
    selected = [row for row in cases if row["category"] == "comms_determinism"]
    if len(selected) != 1:
        raise ValueError("comms_determinism must contain exactly one case")
    row = selected[0]
    if row["kind"] != "connector_status_trace":
        raise ValueError("comms_determinism case must use connector_status_trace")
    manifest = manifests[row["case_id"]]
    steps = manifest.get("connector_status_steps")
    if not isinstance(steps, list) or len(steps) != 8:
        raise ValueError("comms_determinism must contain exactly eight scripted steps")

    execution = _contained_file(root, EXECUTION_PATH).read_text()
    reviewed_source = _reviewed_comms_source(execution)
    reviewed_source_digest = _source_closure_digest(root, COMMS_REVIEWED_SOURCE_PATHS)
    if reviewed_source_digest != COMMS_REVIEWED_SOURCE_DIGEST:
        raise ValueError("reviewed conformance comms source digest drifted")
    dispatch_fragment = (
        "CaseKind::ConnectorStatusTrace => execute_connector_status_trace_case(case)"
    )
    if dispatch_fragment not in execution:
        raise ValueError("reviewed connector-status dispatch path drifted")
    for token in _NETWORK_SOURCE_TOKENS:
        if token.lower() in reviewed_source.lower():
            raise ValueError(f"conformance comms execution introduces live network token {token}")
    required_fragments = (
        "fn execute_connector_status_trace_case(case: &CaseDefinition)",
        "let projection = project_connector_status_step(step)",
        "fn project_connector_status_step(",
    )
    missing = [fragment for fragment in required_fragments if fragment not in reviewed_source]
    if missing:
        raise ValueError("reviewed scripted conformance call path drifted: " + "; ".join(missing))
    return {
        "case_id": row["case_id"],
        "kind": row["kind"],
        "execution_mode": "scripted_in_process",
        "scripted_steps": len(steps),
        "program_source_present": row["program_path"] is not None,
        "live_socket_dependency": False,
        "reviewed_call_path": list(_REVIEWED_COMMS_CALL_PATH),
        "reviewed_source_paths": list(COMMS_REVIEWED_SOURCE_PATHS),
        "reviewed_source_digest": reviewed_source_digest,
    }


def _analyze_publication(root: Path, tracked_report_paths: Sequence[str]) -> dict[str, Any]:
    normalized = sorted(_required_string(path, "tracked report path") for path in tracked_report_paths)
    if normalized != [REPORT_KEEP_PATH]:
        raise ValueError(
            "generated report files under conformance/reports are forbidden; "
            f"tracked paths were {normalized}"
        )
    ignore = _contained_file(root, GITIGNORE_PATH).read_text()
    if "conformance/reports/*\n!conformance/reports/.gitkeep\n" not in ignore:
        raise ValueError("conformance generated-report ignore contract drifted")
    keep_line = ignore.splitlines().index("!conformance/reports/.gitkeep")
    later_negations = [
        line.strip()
        for line in ignore.splitlines()[keep_line + 1 :]
        if line.strip().startswith("!")
    ]
    if later_negations:
        raise ValueError(
            "conformance generated-report ignore contract has a later negation: "
            + ", ".join(later_negations)
        )
    for candidate in (
        "conformance/reports/latest.json",
        "conformance/reports/latest.md",
    ):
        if not _gitignore_ignores(ignore, candidate):
            raise ValueError(f"conformance generated-report ignore is overridden for {candidate}")
    if _gitignore_ignores(ignore, REPORT_KEEP_PATH):
        raise ValueError("conformance report .gitkeep must remain visible")
    workflow = _contained_file(root, WORKFLOW_PATH).read_text()
    job = _workflow_job_block(workflow, "conformance", "architecture-safety")
    job_digest = "sha256:" + hashlib.sha256(job.encode()).hexdigest()
    if job_digest != CI_JOB_REVIEWED_DIGEST:
        raise ValueError("reviewed conformance CI job digest drifted")
    upload_step = _workflow_step(job, "Upload conformance artifacts")
    upload_fragments = (
        r"^        uses: actions/upload-artifact@v[0-9]+$",
        r"^        with:$",
        r"^          name: conformance-suite$",
        r"^          path: \|$",
        r"^            gate-artifacts/conformance-pass-\*\.json$",
        r"^            gate-artifacts/conformance-pass-\*\.md$",
    )
    if any(
        re.search(pattern, upload_step, re.MULTILINE) is None
        for pattern in upload_fragments
    ):
        raise ValueError("conformance upload action is missing or malformed")
    required = (
        "name: conformance-suite",
        "--output gate-artifacts/conformance-pass-1.json",
        "--output gate-artifacts/conformance-pass-2.json",
        "--output gate-artifacts/conformance-pass-1.md",
        "gate-artifacts/conformance-pass-*.json",
        "gate-artifacts/conformance-pass-*.md",
    )
    missing = [fragment for fragment in required if fragment not in job]
    if missing:
        raise ValueError("conformance-suite CI publication binding drifted: " + "; ".join(missing))
    public_page_digest = _validate_public_page(root)
    return {
        "ci_job": ".github/workflows/ci.yml#conformance",
        "ci_job_digest": job_digest,
        "ci_artifact_name": "conformance-suite",
        "generated_json_glob": "gate-artifacts/conformance-pass-*.json",
        "generated_markdown_glob": "gate-artifacts/conformance-pass-*.md",
        "generated_report_policy": "ci_artifact_only",
        "tracked_report_files": normalized,
        "public_page_embeds_generated_result": False,
        "public_page_digest": public_page_digest,
    }


def _validate_public_page(root: Path) -> str:
    path = _contained_file(root, PUBLIC_PAGE_PATH)
    text = path.read_text()
    required = (
        "uploads both\nmachine-readable JSON and human-readable Markdown reports as CI artifacts",
        "Generated files under `conformance/reports/` are not part of the public docs\n"
        "source; use CI artifacts for run reports.",
    )
    if any(fragment not in text for fragment in required):
        raise ValueError("public conformance page no longer binds generated results to CI artifacts")
    embedded = re.search(r"conformance/reports/[A-Za-z0-9_.-]+\.(?:json|md|html)", text)
    if embedded:
        raise ValueError("public conformance page embeds a generated report")
    if re.search(r"^##\s+(?:Latest|Current)\s+(?:Conformance\s+)?Results\s*$", text, re.MULTILINE):
        raise ValueError("public conformance page embeds a generated result summary")
    digest = _digest(path)
    if digest != PUBLIC_PAGE_REVIEWED_DIGEST:
        raise ValueError("reviewed public page digest drifted")
    return digest


def _reviewed_comms_source(execution: str) -> str:
    try:
        start = execution.index("fn execute_connector_status_trace_case(")
        end = execution.index("\nfn parse_ads_connection_state", start)
    except ValueError as exc:
        raise ValueError("reviewed scripted conformance call path is missing") from exc
    return execution[start:end]


def _validate_runner_source_contract(root: Path) -> str:
    digest = _source_closure_digest(root, RUNNER_REVIEWED_SOURCE_PATHS)
    if digest != RUNNER_REVIEWED_SOURCE_DIGEST:
        raise ValueError("reviewed conformance runner source digest drifted")

    root_source = _contained_file(root, RUNNER_REVIEWED_SOURCE_PATHS[0]).read_text()
    for relative in RUNNER_REVIEWED_SOURCE_PATHS[1:]:
        name = Path(relative).name
        if not _contains_executable_fragment(
            root_source, f'include!("conformance/{name}");'
        ):
            raise ValueError(f"reviewed conformance runner module include drifted: {name}")

    models = _contained_file(
        root, "crates/trust-runtime/src/bin/trust-runtime/conformance/models.rs"
    ).read_text()
    category_match = next(
        (
            match
            for match in re.finditer(
                r"const CATEGORIES: \[&str; 16\] = \[(.*?)\];",
                models,
                re.DOTALL,
            )
            if _match_is_executable(models, match.start())
        ),
        None,
    )
    categories = (
        tuple(re.findall(r'"([a-z0-9_]+)"', category_match.group(1)))
        if category_match is not None
        else ()
    )
    if categories != ALL_CATEGORIES:
        raise ValueError("reviewed conformance runner category order drifted")
    for fragment in (
        'const V1_PROFILE_NAME: &str = "trust-conformance-v1";',
        'const V2_PROFILE_NAME: &str = "trust-conformance-v2";',
        'Self::Passed => "passed"',
        'Self::Failed => "failed"',
        'Self::Error => "error"',
        'Self::Skipped => "skipped"',
    ):
        if not _contains_executable_fragment(models, fragment):
            raise ValueError(f"reviewed conformance runner model drifted: {fragment}")

    discovery = _contained_file(
        root, "crates/trust-runtime/src/bin/trust-runtime/conformance/discovery.rs"
    ).read_text()
    for fragment in (
        "for category in CATEGORIES {",
        "entries.sort_by_key(|entry| entry.file_name());",
        'manifest.sources = vec!["program.st".to_string()];',
    ):
        if not _contains_executable_fragment(discovery, fragment):
            raise ValueError(f"reviewed conformance discovery contract drifted: {fragment}")

    runner = _contained_file(
        root, "crates/trust-runtime/src/bin/trust-runtime/conformance/runner.rs"
    ).read_text()
    for fragment in (
        "cases.sort_by(|left, right| left.id.cmp(&right.id));",
        'let expected_ref = format!("expected/{}/{}.json", case.category, case.id);',
        "Ok(expected) if expected == artifact.payload => {",
        'summary_result.status = CaseStatus::Failed.as_str().to_string();',
        'summary_result.status = CaseStatus::Error.as_str().to_string();',
        'ordering: "case_id_asc".to_string(),',
        "if summary.summary.failed > 0 || summary.summary.errors > 0 {",
    ):
        if not _contains_executable_fragment(runner, fragment):
            raise ValueError(f"reviewed conformance runner contract drifted: {fragment}")
    return digest


def _contains_executable_fragment(source: str, fragment: str) -> bool:
    return any(
        _match_is_executable(source, match.start())
        for match in re.finditer(re.escape(fragment), source)
    )


def _match_is_executable(source: str, start: int) -> bool:
    sanitized = sanitize_rust(source)
    return start < len(sanitized) and not sanitized[start].isspace()


def _source_closure_digest(root: Path, paths: Sequence[str]) -> str:
    digest = hashlib.sha256()
    for relative in paths:
        digest.update(relative.encode())
        digest.update(b"\0")
        digest.update(_contained_file(root, relative).read_bytes())
        digest.update(b"\0")
    return "sha256:" + digest.hexdigest()


def _gitignore_ignores(text: str, candidate: str) -> bool:
    ignored = False
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        negated = line.startswith("!")
        pattern = line[1:] if negated else line
        pattern = pattern.lstrip("/")
        if fnmatch.fnmatchcase(candidate, pattern):
            ignored = not negated
    return ignored


def _workflow_job_block(workflow: str, job: str, next_job: str) -> str:
    try:
        start = workflow.index(f"  {job}:\n")
        end = workflow.index(f"\n  {next_job}:\n", start)
    except ValueError as exc:
        raise ValueError(f"reviewed workflow job block is missing: {job}") from exc
    return workflow[start:end]


def _workflow_step(job: str, name: str) -> str:
    pattern = re.compile(
        rf"^      - name: {re.escape(name)}\s*$([\s\S]*?)(?=^      - name:|\Z)",
        re.MULTILINE,
    )
    match = pattern.search(job)
    if match is None:
        raise ValueError(f"conformance workflow step is missing: {name}")
    return match.group(0)


def _markdown_categories(text: str, start: str, end: str) -> tuple[str, ...]:
    try:
        section = text.split(start, 1)[1].split(end, 1)[0]
    except IndexError as exc:
        raise ValueError(f"conformance contract is missing section {start}") from exc
    return tuple(re.findall(r"^- `([a-z0-9_]+)`:", section, re.MULTILINE))


def _nested_keys(value: Any) -> list[str]:
    keys: list[str] = []
    if isinstance(value, Mapping):
        for key, child in value.items():
            if isinstance(key, str):
                keys.append(key)
            keys.extend(_nested_keys(child))
    elif isinstance(value, list):
        for child in value:
            keys.extend(_nested_keys(child))
    return keys


def _contained_file(root: Path, relative: str) -> Path:
    path = root / relative
    _require_contained(root, path)
    if not path.is_file():
        raise ValueError(f"required conformance input is missing: {relative}")
    if path.is_symlink():
        raise ValueError(f"conformance input must not be a symlink: {relative}")
    return path


def _require_contained(root: Path, path: Path) -> None:
    try:
        path.resolve().relative_to(root.resolve())
    except ValueError as exc:
        raise ValueError(f"conformance input escapes workspace: {path}") from exc


def _digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _required_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} must be a non-empty string")
    return value


def _is_tracked_or_fixture(root: Path, relative: str) -> bool:
    result = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--error-unmatch", "--", relative],
        check=False,
        capture_output=True,
    )
    if result.returncode == 0:
        return True
    repository_check = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "--is-inside-work-tree"],
        check=False,
        capture_output=True,
    )
    if repository_check.returncode == 0:
        raise ValueError(f"registered conformance contract is not tracked: {relative}")
    return True
