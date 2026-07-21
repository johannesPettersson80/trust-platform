"""CLI orchestration for Phase 17 code-to-spec coverage."""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import subprocess
import sys
import tomllib
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .code_spec_debt import validate_delete_only_history
from .code_spec_gate import blocking_failures, changed_path_outcomes
from .code_spec_live import build_code_spec_live_state
from .code_spec_provenance import (
    CodeSpecProvenanceError,
    PRIVATE_INPUT_MANIFEST_PATH,
    input_closure_digest,
    load_private_input_manifest,
    validate_clean_source_revision,
)
from .code_spec_report import (
    CodeSpecDenominator,
    CodeSpecProvenance,
    build_report_payload,
    canonical_json,
    render_markdown,
    validate_report_bytes,
)
from .test_catalog_json_schema import validate_json_schema_instance


DEBT_PATH = "verification/code-spec-debt-baseline.toml"
STATIC_REPORT_INPUTS = frozenset(
    {
        "Cargo.lock",
        "Cargo.toml",
        "editors/vscode/package-lock.json",
        "editors/vscode/package.json",
        "crates/trust-runtime/web/ide-frontend/package-lock.json",
        "crates/trust-runtime/web/ide-frontend/package.json",
        "verification/code-spec-map.toml",
        PRIVATE_INPUT_MANIFEST_PATH,
        DEBT_PATH,
        "verification/matrix.toml",
        "verification/gate-inventory.toml",
        "verification/fuzz-program.toml",
        "verification/spec-gaps.toml",
        "verification/spec-sources.toml",
        "verification/test-catalog.toml",
    }
)


def collect_report_inputs(root: Path, state: Any) -> tuple[str, ...]:
    inputs = set(STATIC_REPORT_INPUTS)
    for inventory_name in ("rust", "typescript", "tooling"):
        inventory = getattr(state, inventory_name)
        inputs.update(inventory.source_paths)
    for pattern in (
        "scripts/verification/code_spec_*.py",
        "scripts/verification/code_spec_*.mjs",
        "verification/schemas/code-spec-*.json",
        "verification/invariants/**/*.toml",
        "xtask/src/code_spec_scan/*.rs",
        "**/Cargo.toml",
    ):
        for path in root.glob(pattern):
            if not path.is_file() or any(
                part in {"target", "node_modules", ".git"} for part in path.parts
            ):
                continue
            if path.name.endswith(("_test.py", "_tests.py")):
                continue
            inputs.add(path.relative_to(root).as_posix())
    return tuple(sorted(inputs))


def validate_frozen_debt_history(root: Path, current: dict[str, Any]) -> list[str]:
    history = subprocess.run(
        [
            "git",
            "-C",
            str(root),
            "log",
            "--follow",
            "--diff-filter=A",
            "--format=%H",
            "--",
            DEBT_PATH,
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    commits = [line for line in history.stdout.splitlines() if line]
    if history.returncode != 0 or not commits:
        return ["debt baseline has no committed frozen origin"]
    frozen_commit = commits[-1]
    frozen_bytes = subprocess.run(
        ["git", "-C", str(root), "show", f"{frozen_commit}:{DEBT_PATH}"],
        check=False,
        capture_output=True,
    )
    if frozen_bytes.returncode != 0:
        return [f"debt baseline cannot be read from frozen origin {frozen_commit}"]
    try:
        frozen = tomllib.loads(frozen_bytes.stdout.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        return [f"frozen debt baseline is invalid: {error}"]
    failures: list[str] = []
    parent_line = subprocess.run(
        ["git", "-C", str(root), "rev-list", "--parents", "-n", "1", frozen_commit],
        check=False,
        capture_output=True,
        text=True,
    )
    parent_fields = parent_line.stdout.split()
    if parent_line.returncode != 0 or len(parent_fields) != 2:
        failures.append("debt baseline frozen origin must be a single-parent commit")
    elif frozen.get("created_from_commit") != parent_fields[1]:
        failures.append(
            "debt baseline created_from_commit does not match its introduction parent: "
            f"claimed={frozen.get('created_from_commit')} parent={parent_fields[1]}"
        )
    failures.extend(validate_delete_only_history(current, frozen))
    return sorted(set(failures))


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate and enforce Phase 17 production code-to-spec coverage."
    )
    parser.add_argument("--root", default=".")
    parser.add_argument("--mode", choices=("report", "changed", "full"))
    parser.add_argument("--base", help="Base Git revision for changed-path outcomes")
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--changed-file", action="append", default=[])
    parser.add_argument(
        "--out-dir",
        default="target/gate-artifacts/code-spec-coverage",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    root = Path(args.root).resolve()
    try:
        state = build_code_spec_live_state(root)
    except Exception as error:
        print(f"code-spec coverage failed during live extraction: {error}", file=sys.stderr)
        return 1

    configured_mode = str(
        state.metadata.debt_baseline.get("enforcement_mode", "changed")
    )
    mode = args.mode or configured_mode
    if configured_mode == "full" and mode == "changed":
        print("code-spec coverage refuses to weaken configured full enforcement", file=sys.stderr)
        return 1

    changed_paths = sorted(set(args.changed_file))
    if args.base:
        changed_paths.extend(_git_changed_paths(root, args.base, args.head))
        changed_paths = sorted(set(changed_paths))
    if mode == "changed" and not changed_paths:
        print(
            "code-spec changed enforcement requires --base or --changed-file",
            file=sys.stderr,
        )
        return 1

    history_failures = validate_frozen_debt_history(
        root, dict(state.metadata.debt_baseline)
    )
    findings = tuple(sorted(set((*state.failures, *history_failures))))
    blocking = blocking_failures(mode, findings, state.metadata.debt_code_ids)
    if mode == "changed":
        blocking = sorted(
            set(
                (
                    *blocking,
                    *changed_path_outcomes(
                        changed_paths,
                        facts=state.facts,
                        mappings=state.metadata.mappings,
                        failures=findings,
                    ),
                )
            )
        )

    input_paths = collect_report_inputs(root, state)
    commit = _git_text(root, "rev-parse", "HEAD")
    try:
        private_inputs = load_private_input_manifest(root)
    except CodeSpecProvenanceError as error:
        print(f"code-spec private input manifest failed: {error}", file=sys.stderr)
        return 1
    provenance_failures = validate_clean_source_revision(
        root,
        commit,
        input_paths,
        allowed_untracked=private_inputs,
    )
    if provenance_failures:
        print("code-spec report provenance failed:", file=sys.stderr)
        for failure in provenance_failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    try:
        input_digest = input_closure_digest(root, input_paths)
    except CodeSpecProvenanceError as error:
        print(f"code-spec input closure failed: {error}", file=sys.stderr)
        return 1

    output_dir = Path(args.out_dir)
    if not output_dir.is_absolute():
        output_dir = root / output_dir
    output_dir.mkdir(parents=True, exist_ok=True)
    json_path = output_dir / "code-spec-coverage.json"
    markdown_path = output_dir / "code-spec-coverage.md"
    try:
        json_relative = json_path.relative_to(root).as_posix()
        markdown_relative = markdown_path.relative_to(root).as_posix()
    except ValueError:
        print("code-spec output directory must stay inside the repository", file=sys.stderr)
        return 1

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
    payload = build_report_payload(
        facts=state.facts,
        mappings=state.metadata.mappings,
        failures=findings,
        provenance=CodeSpecProvenance(
            commit=commit,
            timestamp=datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
            platform=f"{platform.system().lower()}-{platform.machine().lower()}",
            input_paths=input_paths,
            output_json=json_relative,
            output_markdown=markdown_relative,
        ),
        input_digest=input_digest,
        debt_baseline=state.metadata.debt_baseline,
        cargo_bindings=state.rust.cargo_bindings,
        inventory_bindings=state.tooling_bindings,
        denominator=CodeSpecDenominator(
            source_counts=source_counts,
            cargo_target_counts={
                "rust_product": state.rust.target_count,
                "verification_tooling_rust": state.tooling.rust_target_count,
            },
        ),
        enforcement_mode=mode,
        blocking_findings=tuple(blocking),
    )
    schema = json.loads(
        (root / "verification/schemas/code-spec-coverage-report.schema.json").read_text()
    )
    schema_failures = validate_json_schema_instance(payload, schema)
    if schema_failures:
        print("code-spec report schema validation failed:", file=sys.stderr)
        for failure in schema_failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    json_text = canonical_json(payload)
    json_digest = hashlib.sha256(json_text.encode("utf-8")).hexdigest()
    markdown = render_markdown(payload, json_digest=json_digest)
    json_path.write_text(json_text, encoding="utf-8")
    markdown_path.write_text(markdown, encoding="utf-8")
    at_rest_failures = validate_report_bytes(json_path.read_bytes(), markdown_path.read_text())
    if at_rest_failures:
        print("code-spec at-rest validation failed:", file=sys.stderr)
        for failure in at_rest_failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print(
        "code-spec coverage generated: "
        f"{len(state.facts)} facts; {len(findings)} findings; "
        f"{len(blocking)} blocking in {mode} mode"
    )
    if blocking:
        for failure in blocking[:100]:
            print(f"- {failure}", file=sys.stderr)
        if len(blocking) > 100:
            print(f"- ... {len(blocking) - 100} more", file=sys.stderr)
        return 1
    return 0


def _git_changed_paths(root: Path, base: str, head: str) -> list[str]:
    completed = subprocess.run(
        ["git", "-C", str(root), "diff", "--name-only", "--diff-filter=ACMR", base, head],
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise ValueError(f"cannot resolve changed paths: {completed.stderr.strip()}")
    return [line for line in completed.stdout.splitlines() if line]


def _git_text(root: Path, *arguments: str) -> str:
    completed = subprocess.run(
        ["git", "-C", str(root), *arguments],
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise ValueError(completed.stderr.strip() or "Git command failed")
    return completed.stdout.strip()


if __name__ == "__main__":
    raise SystemExit(main())
