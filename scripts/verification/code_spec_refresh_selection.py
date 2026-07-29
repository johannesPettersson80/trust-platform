"""Live-reconstructed, recoverable acceptance of code-spec refresh reviews."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .code_spec_cli import (
    collect_report_inputs,
    validate_frozen_debt_history,
)
from .code_spec_live import build_code_spec_live_state
from .code_spec_provenance import (
    input_closure_digest,
    load_private_input_manifest,
    validate_clean_source_revision,
)
from .code_spec_refresh import build_refresh_proposals
from .code_spec_refresh_projection import (
    CODE_MAP_PATH,
    RISK_ACK_PATH,
    SPEC_GAPS_PATH,
    RefreshProjection,
    project_refresh_selection,
)
from .code_spec_report import (
    CodeSpecDenominator,
    CodeSpecProvenance,
    build_report_payload,
    canonical_json,
    validate_report_bytes,
)
from .mapping_transaction import apply_mapping_transaction
from .metadata_validator.code_spec import (
    validate_code_spec_metadata_instances,
)
from .quarantine_paths import quarantine_path
from .test_catalog_json_schema import validate_json_schema_instance


REPORT_SCHEMA_PATH = "verification/schemas/code-spec-coverage-report.schema.json"
REFRESH_SCHEMA_PATH = "verification/schemas/code-spec-refresh-proposals.schema.json"
LiveReconstructor = Callable[[Path, bytes], Mapping[str, Any]]
PostValidator = Callable[[Path], Sequence[str]]


@dataclass(frozen=True)
class RefreshApplication:
    """Exact projection, manifest, and compare-and-swap preconditions."""

    projection: RefreshProjection
    replacements: dict[str, bytes]
    expected_originals: dict[str, bytes | None]
    manifest: dict[str, Any]
    manifest_bytes: bytes
    proposal_path: str
    selection_path: str
    output_path: str


def build_refresh_application(
    *,
    root: Path,
    proposal_path: Path,
    selection_path: Path,
    output_path: Path,
    reconstruct_live: LiveReconstructor | None = None,
) -> RefreshApplication:
    """Reconstruct live inputs and build a write-free acceptance projection."""

    root = root.resolve()
    proposal_relative, proposal_resolved = quarantine_path(
        root, proposal_path, label="refresh proposal", must_exist=True
    )
    selection_relative, selection_resolved = quarantine_path(
        root, selection_path, label="refresh selection", must_exist=True
    )
    output_relative, output_resolved = quarantine_path(
        root, output_path, label="refresh application output", must_exist=False
    )
    if len({proposal_resolved, selection_resolved, output_resolved}) != 3:
        raise ValueError(
            "proposal, selection, and application output paths must differ"
        )
    if output_resolved.suffix != ".json":
        raise ValueError("refresh application output must be JSON")

    proposal_bytes = proposal_resolved.read_bytes()
    selection_bytes = selection_resolved.read_bytes()
    reconstruct = reconstruct_live or reconstruct_live_refresh_proposal
    live_reconstruction = dict(reconstruct(root, proposal_bytes))
    if not live_reconstruction:
        raise ValueError("live refresh reconstruction returned no bindings")

    canonical_paths = (CODE_MAP_PATH, RISK_ACK_PATH, SPEC_GAPS_PATH)
    originals = {
        relative: _canonical_file(root, relative).read_bytes()
        for relative in canonical_paths
    }
    projection = project_refresh_selection(
        proposal_bytes=proposal_bytes,
        selection_bytes=selection_bytes,
        code_map_bytes=originals[CODE_MAP_PATH],
        risk_acknowledgements_bytes=originals[RISK_ACK_PATH],
        spec_gaps_bytes=originals[SPEC_GAPS_PATH],
    )
    manifest = {
        **projection.manifest,
        "kind": "p18_code_spec_refresh_acceptance_application",
        "proposal_path": proposal_relative,
        "selection_path": selection_relative,
        "application_path": output_relative,
        "live_reconstruction": live_reconstruction,
        "canonical_replacement_paths": sorted(projection.replacements),
    }
    manifest_bytes = (
        json.dumps(
            manifest,
            indent=2,
            sort_keys=True,
            ensure_ascii=False,
        )
        + "\n"
    ).encode()
    expected_originals: dict[str, bytes | None] = {
        **originals,
        proposal_relative: proposal_bytes,
        selection_relative: selection_bytes,
        output_relative: (
            output_resolved.read_bytes() if output_resolved.is_file() else None
        ),
    }
    replacements = {
        **projection.replacements,
        output_relative: manifest_bytes,
    }
    return RefreshApplication(
        projection=projection,
        replacements=replacements,
        expected_originals=expected_originals,
        manifest=manifest,
        manifest_bytes=manifest_bytes,
        proposal_path=proposal_relative,
        selection_path=selection_relative,
        output_path=output_relative,
    )


def apply_refresh_application(
    *,
    root: Path,
    proposal_path: Path,
    selection_path: Path,
    output_path: Path,
    application: RefreshApplication,
    reconstruct_live: LiveReconstructor | None = None,
    post_validate: PostValidator | None = None,
) -> None:
    """Rebuild, compare, and atomically apply only the exact projection."""

    root = root.resolve()
    rebuilt = build_refresh_application(
        root=root,
        proposal_path=proposal_path,
        selection_path=selection_path,
        output_path=output_path,
        reconstruct_live=reconstruct_live,
    )
    if rebuilt != application:
        raise ValueError(
            "refresh application differs from canonical rebuilt inputs"
        )
    validate_after = post_validate or validate_refresh_post_state

    def validate() -> list[str]:
        failures = []
        for relative, expected in rebuilt.replacements.items():
            path = root / relative
            if not path.is_file() or path.read_bytes() != expected:
                failures.append(f"{relative}: atomic replacement differs")
        failures.extend(validate_after(root))
        return failures

    apply_mapping_transaction(
        root=root,
        replacements=rebuilt.replacements,
        expected_originals=rebuilt.expected_originals,
        validate=validate,
    )


def reconstruct_live_refresh_proposal(
    root: Path, proposal_bytes: bytes
) -> Mapping[str, Any]:
    """Rebuild the complete report and proposal from current clean sources."""

    proposal = _json_object(proposal_bytes, "refresh proposal")
    source_report = proposal.get("source_report")
    if not isinstance(source_report, Mapping):
        raise ValueError("refresh proposal source_report is malformed")
    report_relative = _required_string(
        source_report, "path", "refresh proposal source report"
    )
    report_path = _canonical_file(root, report_relative)
    report_bytes = report_path.read_bytes()
    if _digest(report_bytes) != source_report.get("sha256"):
        raise ValueError("refresh proposal source report digest differs")
    report = _json_object(report_bytes, "source report")
    markdown_relative = _report_markdown_path(report, report_relative)
    markdown_path = _canonical_file(root, markdown_relative)
    markdown = markdown_path.read_text(encoding="utf-8")

    report_schema = _json_file(root / REPORT_SCHEMA_PATH, "report schema")
    report_failures = validate_json_schema_instance(report, report_schema)
    report_failures.extend(validate_report_bytes(report_bytes, markdown))
    if report_failures:
        raise ValueError(
            "source report validation failed: "
            + "; ".join(sorted(set(report_failures)))
        )

    commit = _git_head(root)
    if report.get("commit") != commit or proposal.get("source_commit") != commit:
        raise ValueError("refresh source commit does not match current HEAD")
    raw_input_paths = report.get("input_paths")
    if not isinstance(raw_input_paths, list) or any(
        not isinstance(path, str) or not path for path in raw_input_paths
    ):
        raise ValueError("source report input_paths are malformed")
    input_paths = tuple(raw_input_paths)
    private_inputs = load_private_input_manifest(root)
    provenance_failures = validate_clean_source_revision(
        root,
        commit,
        input_paths,
        allowed_untracked=private_inputs,
    )
    if provenance_failures:
        raise ValueError(
            "source report provenance failed: "
            + "; ".join(provenance_failures)
        )
    live_input_digest = input_closure_digest(root, input_paths)
    if live_input_digest != report.get("input_digest"):
        raise ValueError("source report input digest differs from live closure")

    state = build_code_spec_live_state(root)
    rebuilt_input_paths = collect_report_inputs(root, state)
    if tuple(rebuilt_input_paths) != input_paths:
        raise ValueError("source report input path closure differs from live scan")
    findings = tuple(
        sorted(
            set(
                (
                    *state.failures,
                    *validate_frozen_debt_history(
                        root, dict(state.metadata.debt_baseline)
                    ),
                )
            )
        )
    )
    output_paths = report.get("output_paths")
    if not isinstance(output_paths, Mapping):
        raise ValueError("source report output_paths are malformed")
    rebuilt_report = build_report_payload(
        facts=state.facts,
        mappings=state.metadata.mappings,
        failures=findings,
        provenance=CodeSpecProvenance(
            commit=commit,
            timestamp=_required_string(report, "timestamp", "source report"),
            platform=_required_string(report, "platform", "source report"),
            input_paths=input_paths,
            output_json=_required_string(
                output_paths, "json", "source report output paths"
            ),
            output_markdown=_required_string(
                output_paths, "markdown", "source report output paths"
            ),
        ),
        input_digest=live_input_digest,
        debt_baseline=state.metadata.debt_baseline,
        cargo_bindings=state.rust.cargo_bindings,
        inventory_bindings=state.tooling_bindings,
        risk_acknowledgements=state.metadata.risk_acknowledgements,
        denominator=_live_denominator(state),
        enforcement_mode="report",
        blocking_findings=(),
    )
    if rebuilt_report != report:
        raise ValueError("source report differs from canonical live reconstruction")

    code_map_bytes = _canonical_file(root, CODE_MAP_PATH).read_bytes()
    rebuilt_proposal = build_refresh_proposals(
        report=rebuilt_report,
        source_report_path=report_relative,
        source_report_sha256=_digest(report_bytes),
        code_map=state.metadata.code_map,
        code_map_sha256=_digest(code_map_bytes),
    )
    refresh_schema = _json_file(root / REFRESH_SCHEMA_PATH, "refresh schema")
    refresh_failures = validate_json_schema_instance(
        rebuilt_proposal, refresh_schema
    )
    if refresh_failures:
        raise ValueError(
            "rebuilt refresh proposal schema failed: "
            + "; ".join(sorted(set(refresh_failures)))
        )
    rebuilt_bytes = canonical_json(rebuilt_proposal).encode()
    if rebuilt_bytes != proposal_bytes:
        raise ValueError(
            "refresh proposal differs from canonical live reconstruction"
        )
    summary = rebuilt_proposal["summary"]
    return {
        "source_commit": commit,
        "source_report_sha256": _digest(report_bytes),
        "source_input_digest": live_input_digest,
        "rebuilt_proposal_sha256": _digest(rebuilt_bytes),
        "operation_set_digest": rebuilt_proposal["operation_set_digest"],
        "source_facts": summary["source_facts"],
        "source_findings": summary["source_findings"],
        "missing_mapping_proposals": summary[
            "missing_mapping_proposals"
        ],
        "digest_rebind_proposals": summary["digest_rebind_proposals"],
        "stale_identity_proposals": summary["stale_identity_proposals"],
        "semantic_review_groups": summary["semantic_review_groups"],
        "unclassified_findings": summary["unclassified_findings"],
    }


def validate_refresh_post_state(root: Path) -> list[str]:
    """Require an exact live identity join after the atomic replacement."""

    failures = [
        f"{path}: {message}"
        for path, message in validate_code_spec_metadata_instances(root)
    ]
    try:
        state = build_code_spec_live_state(root)
    except Exception as error:
        return [*failures, f"post-apply live reconstruction failed: {error}"]
    mapping_ids = set(state.metadata.mappings)
    fact_ids = set(state.facts)
    missing = sorted(fact_ids - mapping_ids)
    stale = sorted(mapping_ids - fact_ids)
    if missing:
        failures.append(
            f"post-apply map omits {len(missing)} live identities: {missing[:5]}"
        )
    if stale:
        failures.append(
            f"post-apply map retains {len(stale)} stale identities: {stale[:5]}"
        )
    blocking_terms = (
        "missing_spec",
        "stale_spec_review",
        "unmapped",
        "stale mapping identity",
        "invented code identity",
        "unknown code identity",
        "risk acknowledgement",
    )
    for finding in state.failures:
        if any(term in finding for term in blocking_terms):
            failures.append(f"post-apply identity finding: {finding}")
    return sorted(set(failures))


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Apply a fully reviewed code-spec refresh identity queue."
    )
    parser.add_argument("--root", default=".")
    parser.add_argument("--proposal", required=True)
    parser.add_argument("--selection", required=True)
    parser.add_argument("--output", required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--apply", action="store_true")
    args = parser.parse_args(argv)
    root = Path(args.root).resolve()
    try:
        application = build_refresh_application(
            root=root,
            proposal_path=Path(args.proposal),
            selection_path=Path(args.selection),
            output_path=Path(args.output),
        )
        if args.apply:
            apply_refresh_application(
                root=root,
                proposal_path=Path(args.proposal),
                selection_path=Path(args.selection),
                output_path=Path(args.output),
                application=application,
            )
            verb = "applied"
        else:
            verb = "valid"
    except (
        OSError,
        UnicodeError,
        ValueError,
        json.JSONDecodeError,
    ) as error:
        print(f"code-spec refresh acceptance failed: {error}", file=sys.stderr)
        return 1
    print(
        f"code-spec refresh acceptance {verb}: "
        f"transferred={application.manifest['transferred_successors']} "
        f"removed={application.manifest['removed_stale_identities']} "
        f"rebinds={application.manifest['digest_rebinds']} "
        f"direct={application.manifest['direct_missing_mappings']} "
        f"after={application.manifest['code_map_after_sha256']}"
    )
    return 0


def _live_denominator(state: Any) -> CodeSpecDenominator:
    source_counts = {
        "rust_product": state.rust.source_count,
        **{
            f"typescript_product:{name}": count
            for name, count in state.typescript.source_counts.items()
        },
        **{
            f"verification_tooling:{name}": count
            for name, count in state.tooling.source_counts.items()
        },
    }
    return CodeSpecDenominator(
        source_counts=source_counts,
        cargo_target_counts={
            "rust_product": state.rust.target_count,
            "verification_tooling_rust": state.tooling.rust_target_count,
        },
    )


def _report_markdown_path(
    report: Mapping[str, Any], report_relative: str
) -> str:
    output_paths = report.get("output_paths")
    if isinstance(output_paths, Mapping):
        value = output_paths.get("markdown")
        if isinstance(value, str) and value:
            return value
    path = Path(report_relative)
    return path.with_suffix(".md").as_posix()


def _canonical_file(root: Path, relative: str) -> Path:
    candidate = Path(relative)
    if candidate.is_absolute() or any(
        part in {"", ".", ".."} for part in candidate.parts
    ):
        raise ValueError(f"path must be canonical and repository-relative: {relative}")
    current = root
    for part in candidate.parts:
        current /= part
        if current.is_symlink():
            raise ValueError(f"path cannot traverse symlink: {relative}")
    resolved = current.resolve()
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise ValueError(f"path escapes repository root: {relative}") from error
    if not resolved.is_file():
        raise ValueError(f"required file is absent: {relative}")
    return resolved


def _json_file(path: Path, label: str) -> dict[str, Any]:
    return _json_object(path.read_bytes(), label)


def _json_object(content: bytes, label: str) -> dict[str, Any]:
    value = json.loads(content)
    if not isinstance(value, dict):
        raise ValueError(f"{label} root must be an object")
    return value


def _required_string(
    row: Mapping[str, Any], field: str, label: str
) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} requires nonempty {field}")
    return value


def _git_head(root: Path) -> str:
    import subprocess

    result = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "HEAD"],
        check=False,
        capture_output=True,
        text=True,
    )
    value = result.stdout.strip()
    if result.returncode != 0 or len(value) != 40:
        raise ValueError("cannot resolve current Git HEAD")
    return value


def _digest(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


if __name__ == "__main__":
    raise SystemExit(main())
