"""Orchestration for the mechanical existing-test catalog scanner."""

from __future__ import annotations

import argparse
import platform
import subprocess
import sys
from pathlib import Path

from .test_catalog_common import diagnostic, input_digest
from .test_catalog_models import (
    DEFAULT_JSON_PATH,
    DEFAULT_MARKDOWN_PATH,
    GeneratedTestCatalog,
    ReportProvenance,
    write_reports as write_report_files,
)
from .test_catalog_rust import scan_rust_tests
from .test_catalog_st import scan_structured_text_tests
from .test_catalog_surfaces import (
    scan_conformance,
    scan_fuzz_targets,
    scan_gate_scripts,
    scan_workflow_jobs,
)
from .test_catalog_vscode import scan_vscode_tests
from .test_catalog_validation import (
    validate_report_files,
    validate_report_payload,
    validate_schema_file,
)


LIMITATIONS = (
    "Discovery is static and recognizes only the declaration forms documented for this slice.",
    "Command hints are navigation aids; non-exact hints are not runnable proof commands.",
    "Reference candidates are lexical observations and never create proof or invariant mappings.",
    "Nested Rust integration support files use package-level hints when no Cargo target is evident.",
    "Dynamic VS Code titles and runtime skips remain visible diagnostics rather than inferred facts.",
    "Structured Text command hints use project-level substring filters and remain conservative.",
    "The scripts/*gate* surface means root-level files under scripts whose names contain gate.",
    "The P2 board scope excludes Rust tests under xtask/**; they are not included in report totals.",
    "Only the root fuzz/Cargo.toml is scanned; crate-local fuzz workspaces are excluded.",
    "Reviewed live census counts are an evidence tripwire and require an intentional refresh on drift.",
    "Hand-owned catalog intent and enforcement remain outside VERIF-P2-001 through VERIF-P2-003.",
)


def scan_repository(
    root: Path,
    *,
    output_json: Path = DEFAULT_JSON_PATH,
    output_markdown: Path = DEFAULT_MARKDOWN_PATH,
    timestamp: str | None = None,
    command: tuple[str, ...] | None = None,
) -> GeneratedTestCatalog:
    root = root.resolve()
    batches = [
        scan_rust_tests(root),
        scan_structured_text_tests(root),
        scan_vscode_tests(root),
        scan_conformance(root),
        scan_fuzz_targets(root),
        scan_gate_scripts(root),
        scan_workflow_jobs(root),
    ]
    facts = [fact for batch in batches for fact in batch.facts]
    diagnostics = [item for batch in batches for item in batch.diagnostics]
    input_paths = sorted({path for batch in batches for path in batch.input_paths})

    facts.sort(key=lambda fact: (fact.source_kind, fact.path, fact.line, fact.name, fact.stable_id))
    diagnostics.sort(key=lambda item: (item.path, item.line, item.severity, item.kind, item.message))
    duplicates = duplicate_ids(facts)
    for stable_id in duplicates:
        diagnostics.append(
            diagnostic(
                "duplicate_discovery_id",
                "<generated>",
                1,
                f"semantic discovery identity is not unique: {stable_id}",
                severity="error",
            )
        )
    diagnostics.sort(key=lambda item: (item.path, item.line, item.severity, item.kind, item.message))

    revision, default_timestamp = repository_revision(root)
    provenance = ReportProvenance(
        command=command or default_command(output_json, output_markdown, timestamp),
        commit=revision,
        timestamp=timestamp or default_timestamp,
        platform=f"{platform.system().lower()}-{platform.machine().lower()}",
        input_paths=tuple(input_paths),
        output_json=output_json.as_posix(),
        output_markdown=output_markdown.as_posix(),
    )
    return GeneratedTestCatalog(
        provenance=provenance,
        input_digest=input_digest(root, input_paths),
        inferred_facts=tuple(facts),
        diagnostics=tuple(diagnostics),
        limitations=LIMITATIONS,
    )


def duplicate_ids(facts: list) -> list[str]:
    seen: set[str] = set()
    duplicates: set[str] = set()
    for fact in facts:
        if fact.stable_id in seen:
            duplicates.add(fact.stable_id)
        seen.add(fact.stable_id)
    return sorted(duplicates)


def repository_revision(root: Path) -> tuple[str, str]:
    head = run_git(root, "rev-parse", "HEAD")
    if head is None:
        return "unavailable", "1970-01-01T00:00:00Z"
    status = run_git(root, "status", "--porcelain", "--untracked-files=normal")
    revision = f"dirty:{head}" if status else head
    timestamp = run_git(root, "show", "-s", "--format=%cI", "HEAD")
    return revision, timestamp or "1970-01-01T00:00:00Z"


def run_git(root: Path, *args: str) -> str | None:
    try:
        result = subprocess.run(
            ["git", "-C", str(root), *args],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError:
        return None
    if result.returncode != 0:
        return None
    return result.stdout.strip()


def default_command(
    output_json: Path,
    output_markdown: Path,
    timestamp: str | None,
) -> tuple[str, ...]:
    command = [
        "python3",
        "scripts/scan_test_catalog.py",
        "--json-out",
        output_json.as_posix(),
        "--markdown-out",
        output_markdown.as_posix(),
    ]
    if timestamp:
        command.extend(("--timestamp", timestamp))
    return tuple(command)


def write_reports(
    report: GeneratedTestCatalog,
    *,
    json_path: Path,
    markdown_path: Path,
) -> None:
    failures = validate_report_payload(report.to_dict())
    if failures:
        raise ValueError("invalid generated test catalog: " + "; ".join(failures))
    write_report_files(report, json_path=json_path, markdown_path=markdown_path)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON_PATH)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MARKDOWN_PATH)
    parser.add_argument("--timestamp", help="fixed ISO-8601 report timestamp")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    command = default_command(args.json_out, args.markdown_out, args.timestamp)
    report = scan_repository(
        args.root,
        output_json=args.json_out,
        output_markdown=args.markdown_out,
        timestamp=args.timestamp,
        command=command,
    )
    failures = validate_report_payload(report.to_dict())
    failures.extend(
        validate_schema_file(args.root / "verification/schemas/generated-test-catalog.schema.json")
    )
    if report.to_dict()["scan_status"] != "complete":
        failures.append("scan_status is incomplete; error diagnostics must be resolved")
    if failures:
        for failure in failures:
            print(f"test catalog scan failed: {failure}", file=sys.stderr)
        return 2
    write_reports(report, json_path=args.json_out, markdown_path=args.markdown_out)
    failures = validate_report_files(
        args.root,
        args.json_out,
        args.markdown_out,
        args.root / "verification/schemas/generated-test-catalog.schema.json",
    )
    if failures:
        for failure in failures:
            print(f"generated test catalog failed at-rest validation: {failure}", file=sys.stderr)
        return 2
    summary = report.to_dict()["summary"]
    print(
        f"generated {summary['records']} records from {summary['files']} source files "
        f"with {summary['warnings']} warning(s)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
