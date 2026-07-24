"""Run focused ST/conformance coverage with one instrumented Rust build."""

from __future__ import annotations

import argparse
import os
import platform
import re
import subprocess
import sys
import tomllib
from pathlib import Path

from .code_spec_live import load_code_spec_metadata
from .code_spec_rust import invoke_rust_extractor
from .coverage_trace_direct import build_instrumented_binaries, run_direct_trace
from .coverage_trace_inputs import trace_input_digest
from .test_catalog_scanner import scan_repository


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument("--discovery-id", action="append")
    selection.add_argument("--all-direct", action="store_true")
    parser.add_argument(
        "--trace-root",
        type=Path,
        default=Path("target/gate-artifacts/verification/direct-catalog-trace"),
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(
            "target/gate-artifacts/verification/direct-test-coverage-trace.json"
        ),
    )
    parser.add_argument("--cargo-target-dir", type=Path, required=True)
    parser.add_argument("--llvm-tools-dir", type=Path, required=True)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument(
        "--jobs",
        type=int,
        default=1,
        help="parallel Structured Text workers; conformance remains serialized",
    )
    parser.add_argument(
        "--temporary-dir",
        type=Path,
        default=Path(
            "target/gate-artifacts/verification/direct-trace-temporary"
        ),
    )
    parser.add_argument(
        "--mode",
        choices=("focused_shard", "full_campaign"),
        default="focused_shard",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    try:
        scan = scan_repository(root)
        facts = {
            fact.stable_id: {
                "discovery_id": fact.stable_id,
                "discovery_source_kind": fact.source_kind,
                "path": fact.path,
                "name": fact.name,
                "package": fact.package,
                "command_hint": fact.command_hint,
                "ignore_state": fact.ignore_state,
            }
            for fact in scan.inferred_facts
        }
        if args.all_direct:
            selected_ids = sorted(
                discovery_id
                for discovery_id, fact in facts.items()
                if fact["discovery_source_kind"]
                in {"structured_text_test", "conformance_case"}
                and fact["ignore_state"] != "ignored"
            )
        else:
            selected_ids = sorted(set(args.discovery_id or []))
            if len(selected_ids) != len(args.discovery_id or []):
                raise ValueError("direct trace discovery IDs must be unique")
        unknown = sorted(set(selected_ids) - set(facts))
        if unknown:
            raise ValueError(f"direct trace names unknown discovery IDs: {unknown}")
        selected = [facts[discovery_id] for discovery_id in selected_ids]
        unsupported = sorted(
            fact["discovery_id"]
            for fact in selected
            if fact["discovery_source_kind"]
            not in {"structured_text_test", "conformance_case"}
        )
        if unsupported:
            raise ValueError(
                f"direct trace selection contains unsupported facts: {unsupported}"
            )
        ignored = sorted(
            fact["discovery_id"]
            for fact in selected
            if fact["ignore_state"] == "ignored"
        )
        if ignored:
            raise ValueError(
                f"direct trace selection contains structurally routed ignores: {ignored}"
            )
        metadata = load_code_spec_metadata(root)
        matrix = tomllib.loads((root / "verification/matrix.toml").read_text())
        inventory = invoke_rust_extractor(
            root,
            matrix,
            area_bindings={},
            reject_invented_area_bindings=False,
            environment=os.environ,
        )
        temporary_dir = _resolve(root, args.temporary_dir)
        temporary_dir.mkdir(parents=True, exist_ok=True)
        environment = dict(os.environ)
        environment["TMPDIR"] = str(temporary_dir)
        binaries, coverage_environment = build_instrumented_binaries(
            root=root,
            cargo_target_dir=_resolve(root, args.cargo_target_dir),
            base_environment=environment,
        )
        document = run_direct_trace(
            root=root,
            facts=selected,
            trace_root=_resolve(root, args.trace_root),
            output_path=_resolve(root, args.out),
            binaries=binaries,
            environment=coverage_environment,
            llvm_tools_dir=_resolve(root, args.llvm_tools_dir),
            rust_inventory=inventory,
            source_commit=_git_head(root),
            platform=f"{platform.system().lower()}-{platform.machine().lower()}",
            input_digest=trace_input_digest(
                scan.input_digest,
                selected,
                metadata.code_map,
            ),
            mode=args.mode,
            resume=args.resume,
            jobs=args.jobs,
        )
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"direct coverage trace failed: {error}", file=sys.stderr)
        return 2
    observed = sum(
        record["trace_status"] == "observed" for record in document["tests"]
    )
    failed = sum(
        record["trace_status"] == "failed" for record in document["tests"]
    )
    print(
        f"direct coverage trace complete: {observed}/{len(document['tests'])} "
        f"tests observed; {failed} failed"
    )
    return 0


def _resolve(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _git_head(root: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    head = completed.stdout.strip()
    if completed.returncode != 0 or re.fullmatch(r"[0-9a-f]{40}", head) is None:
        raise ValueError("direct trace cannot resolve source commit")
    return head


if __name__ == "__main__":
    raise SystemExit(main())
