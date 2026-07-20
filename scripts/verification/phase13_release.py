"""Canonical Phase 13 release-evidence audit model and renderer."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


GENERATOR = "phase13-release-evidence-audit"
GENERATOR_VERSION = 1
MANIFEST_PATH = "verification/release-evidence.toml"
MANIFEST_SCHEMA_PATH = "verification/schemas/release-evidence-manifest.schema.json"
REPORT_SCHEMA_PATH = "verification/schemas/phase13-release-evidence-report.schema.json"
DEFAULT_JSON_PATH = Path("target/gate-artifacts/verification/phase13-release-evidence.json")
DEFAULT_MARKDOWN_PATH = Path(
    "docs/internal/testing/evidence/plc-verification-program/2026-07-19/phase13-release-evidence.md"
)
PROOF_ORIGINS = ("local", "remote_builder", "ci", "hardware_lab", "public_github")
PLATFORM_IDS = ("linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "win32-x64")
REQUIRED_RELEASE_ASSETS = (
    "SHA256SUMS",
    "release-provenance.json",
    "conformance-status.json",
    "conformance-status.md",
)
BOUNDARIES = {
    "configured_gate_is_execution_proof": False,
    "artifact_only_is_native_execution_proof": False,
    "skipped_hardware_is_hardware_proof": False,
    "provisional_ui_is_acceptance": False,
    "version_bump_is_release_completion": False,
    "report_emits_product_proof": False,
}
LIMITATIONS = (
    "The report audits checked repository metadata and one reviewed public GitHub snapshot; it does not query GitHub during at-rest validation.",
    "The evidence index is live-recomputed but excluded from the input digest to avoid a report/evidence self-cycle; Phase 13 evidence rows are excluded from origin counts.",
    "Configured CI and release jobs are policy, not successful execution evidence; only typed evidence records count as proof origins.",
    "Artifact-only targets, skipped hardware rows, provisional UI captures, and expected conformance artifacts do not establish native, physical, visual, or conformance proof.",
)


def validate_manifest(manifest: Mapping[str, Any]) -> list[str]:
    failures: list[str] = []
    expected_top = {
        "schema_version", "id", "owner", "status", "last_reviewed", "spec_source_id",
        "release_branches", "version_sources", "changelog_path", "release_workflow_path",
        "ci_workflow_path", "security_policy_path", "required_release_assets", "proof_origins",
        "hardware_lab_rows", "security_policy", "latest_public_snapshot", "platforms",
    }
    if set(manifest) != expected_top:
        failures.append("release-evidence manifest fields drift from the closed contract")
    for field, expected in (
        ("schema_version", 1), ("id", "RELEASE_EVIDENCE_MANIFEST_001"),
        ("owner", "release"), ("status", "mapped"),
        ("spec_source_id", "SPEC_RELEASE_EVIDENCE_001"),
    ):
        if manifest.get(field) != expected:
            failures.append(f"release-evidence manifest {field} must equal {expected!r}")
    if manifest.get("release_branches") != ["main", "master"]:
        failures.append("release branches must be exactly main and master")
    if manifest.get("proof_origins") != list(PROOF_ORIGINS):
        failures.append("proof origins drift from the reviewed five-origin vocabulary")
    if manifest.get("required_release_assets") != list(REQUIRED_RELEASE_ASSETS):
        failures.append("required release assets drift from the publication guard")
    policy = manifest.get("security_policy")
    if not isinstance(policy, Mapping):
        failures.append("security_policy must be a table")
    else:
        if set(policy) != {
            "exception_owner_required", "exception_expiry_required",
            "maximum_exception_days", "rust_commands", "node_commands",
        }:
            failures.append("security_policy fields drift from the closed contract")
        if policy.get("exception_owner_required") is not True:
            failures.append("dependency exceptions must require an owner")
        if policy.get("exception_expiry_required") is not True:
            failures.append("dependency exceptions must require an expiry")
        if policy.get("maximum_exception_days") != 90:
            failures.append("dependency exceptions must expire within 90 days")
    snapshot = manifest.get("latest_public_snapshot")
    if not isinstance(snapshot, Mapping):
        failures.append("latest_public_snapshot must be a table")
    elif snapshot.get("workflow_conclusion") != "success" or snapshot.get("status") != "published_latest":
        failures.append("latest public snapshot must identify a successful final publication")
    platforms = manifest.get("platforms")
    if not isinstance(platforms, list) or [row.get("id") for row in platforms if isinstance(row, Mapping)] != list(PLATFORM_IDS):
        failures.append("platform rows must use the reviewed five-target order")
    else:
        for row in platforms:
            if set(row) != {
                "id", "target", "support_tier", "required_proof", "runtime_asset",
                "lsp_asset", "vsix_asset_template",
            }:
                failures.append(f"platform {row.get('id')} fields drift from the closed contract")
            if row.get("support_tier") == "native_ci" and "native_ci_test" not in row.get("required_proof", []):
                failures.append(f"platform {row.get('id')} native_ci tier lacks native CI proof")
            if row.get("support_tier") == "artifact_only" and "native_ci_test" in row.get("required_proof", []):
                failures.append(f"platform {row.get('id')} artifact-only tier claims native CI proof")
    return sorted(set(failures))


def build_payload(
    *, commit: str, branch: str, timestamp: str, platform: str,
    input_paths: Sequence[str], input_digest: str, output_json: str,
    output_markdown: str, command: Sequence[str], candidate: Mapping[str, Any],
    public_release: Mapping[str, Any], proof_origins: Sequence[Mapping[str, Any]],
    security: Mapping[str, Any], platforms: Sequence[Mapping[str, Any]],
    conformance: Mapping[str, Any], hardware_labs: Sequence[Mapping[str, Any]],
    ui_acceptance: Mapping[str, Any], known_gaps: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "generator": GENERATOR,
        "generator_version": GENERATOR_VERSION,
        "report_status": "complete",
        "commit": commit,
        "branch": branch,
        "timestamp": timestamp,
        "platform": platform,
        "input_paths": list(input_paths),
        "input_digest": input_digest,
        "output_paths": {"json": output_json, "markdown": output_markdown},
        "command": list(command),
        "boundaries": dict(BOUNDARIES),
        "candidate": dict(candidate),
        "public_release": dict(public_release),
        "proof_origins": [dict(row) for row in proof_origins],
        "security": dict(security),
        "platforms": [dict(row) for row in platforms],
        "conformance": dict(conformance),
        "hardware_labs": [dict(row) for row in hardware_labs],
        "ui_acceptance": dict(ui_acceptance),
        "known_gaps": [dict(row) for row in known_gaps],
        "limitations": list(LIMITATIONS),
    }


def canonical_json(payload: Mapping[str, Any]) -> str:
    return json.dumps(payload, indent=2, sort_keys=True) + "\n"


def render_markdown(payload: Mapping[str, Any], *, json_digest: str) -> str:
    candidate = payload["candidate"]
    public = payload["public_release"]
    security = payload["security"]
    conformance = payload["conformance"]
    ui = payload["ui_acceptance"]
    lines = [
        "# Phase 13 Release Evidence Audit", "",
        f"Generator: `{GENERATOR} v{GENERATOR_VERSION}`",
        f"Source revision: `{payload['commit']}`",
        f"Branch label: `{payload['branch']}`",
        f"Generated: `{payload['timestamp']}`",
        f"Platform: `{payload['platform']}`",
        f"Generated JSON SHA-256: `{json_digest}`",
        f"Input SHA-256: `{payload['input_digest']}`", "",
        "## Candidate", "",
        f"- Workspace version: `{candidate['version']}`",
        f"- Expected tag: `{candidate['expected_tag']}`",
        f"- Versions synchronized: `{str(candidate['versions_synchronized']).lower()}`",
        f"- Changelog names candidate: `{str(candidate['changelog_mentions_version']).lower()}`",
        f"- Annotated tag present: `{str(candidate['annotated_tag_present']).lower()}`",
        f"- Release complete: `{str(candidate['release_complete']).lower()}`", "",
        "## Public Release Snapshot", "",
        f"- Latest tag: `{public['tag']}`",
        f"- Workflow conclusion: `{public['workflow_conclusion']}`",
        f"- Matches candidate: `{str(public['matches_candidate']).lower()}`",
        f"- Missing required assets: `{', '.join(public['missing_required_assets']) or 'none'}`", "",
        "## Proof Origins", "",
        "| Origin | Typed evidence rows | Status | Limitation |",
        "| --- | ---: | --- | --- |",
    ]
    for row in payload["proof_origins"]:
        lines.append(f"| `{row['origin']}` | {row['evidence_count']} | `{row['status']}` | {row['limitation']} |")
    lines.extend(["", "## Platform Matrix", "", "| Platform | Tier | Required proof | Public assets present |", "| --- | --- | --- | --- |"])
    for row in payload["platforms"]:
        lines.append(
            f"| `{row['id']}` | `{row['support_tier']}` | "
            f"`{', '.join(row['required_proof'])}` | `{str(row['public_assets_present']).lower()}` |"
        )
    lines.extend([
        "", "## Security And Dependencies", "",
        f"- Owned exceptions: {security['owned_exceptions']}",
        f"- Expired exceptions: {security['expired_exceptions']}",
        f"- Cargo policy configured: `{str(security['cargo_policy_configured']).lower()}`",
        f"- npm audit configured: `{str(security['npm_audit_configured']).lower()}`",
        f"- Gate execution claimed: `{str(security['gate_execution_claimed']).lower()}`",
        "", "## Conformance, Hardware, And UI", "",
        f"- Conformance cases cataloged/linked: {conformance['catalog_cases']}/{conformance['linked_cases']}",
        f"- Published conformance asset present: `{str(conformance['public_asset_present']).lower()}`",
        f"- Hardware lab rows skipped/unproven: {sum(row['status'] == 'skipped_unproven' for row in payload['hardware_labs'])}",
        f"- UI journeys accepted/total: {ui['accepted_journeys']}/{ui['journeys']}",
        "", "## Known Gaps", "",
    ])
    lines.extend(f"- `{row['id']}`: {row['status']} - {row['detail']}" for row in payload["known_gaps"])
    lines.extend(["", "## Boundaries", ""])
    lines.extend(f"- `{name}`: `{str(payload['boundaries'][name]).lower()}`" for name in BOUNDARIES)
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in payload["limitations"])
    return "\n".join(lines) + "\n"


def write_report(payload: Mapping[str, Any], json_path: Path, markdown_path: Path) -> None:
    rendered = canonical_json(payload)
    digest = hashlib.sha256(rendered.encode()).hexdigest()
    json_path.parent.mkdir(parents=True, exist_ok=True)
    markdown_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(rendered, encoding="utf-8")
    markdown_path.write_text(render_markdown(payload, json_digest=digest), encoding="utf-8")
