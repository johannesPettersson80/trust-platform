"""Plan or execute the single resumable test-catalogue campaign."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tomllib
from pathlib import Path

from .code_spec_live import load_code_spec_metadata
from .code_spec_rust import invoke_rust_extractor
from .test_catalog_campaign import (
    campaign_commands,
    execute_campaign_commands,
    partition_campaign_facts,
    rank_traceable_rust_ground_truth,
    select_pilot_ids,
)
from .test_catalog_scanner import scan_repository


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--pilot", action="store_true")
    mode.add_argument("--full", action="store_true")
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument(
        "--artifact-root",
        type=Path,
        default=Path("target/gate-artifacts/verification/catalog-campaign"),
    )
    parser.add_argument("--cargo-target-dir", type=Path, required=True)
    parser.add_argument("--llvm-tools-dir", type=Path, required=True)
    parser.add_argument(
        "--temporary-dir",
        type=Path,
        default=Path(
            "target/gate-artifacts/verification/catalog-campaign-temporary"
        ),
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    campaign_mode = "full" if args.full else "pilot"
    try:
        scan = scan_repository(root)
        facts = {fact.stable_id: fact for fact in scan.inferred_facts}
        routes = partition_campaign_facts(scan.inferred_facts)
        metadata = load_code_spec_metadata(root)
        preferred_rust_ids: list[str] = []
        if campaign_mode == "pilot":
            matrix = tomllib.loads((root / "verification/matrix.toml").read_text())
            inventory = invoke_rust_extractor(
                root,
                matrix,
                area_bindings={},
                reject_invented_area_bindings=False,
                environment=os.environ,
            )
            preferred_rust_ids = rank_traceable_rust_ground_truth(
                facts=facts,
                catalog_tests=metadata.tests,
                code_mappings=metadata.mappings,
                code_facts={
                    code_id: {"path": fact.path}
                    for code_id, fact in inventory.facts.items()
                },
                limit=16,
            )
        pilot_ids = (
            None
            if campaign_mode == "full"
            else select_pilot_ids(
                routes=routes,
                facts=facts,
                catalog_tests=metadata.tests,
                preferred_rust_ids=preferred_rust_ids,
            )
        )
        if campaign_mode == "full" and _git_dirty(root):
            raise ValueError(
                "full catalogue campaign requires a frozen clean checkout"
            )
        artifact_root = _resolve(root, args.artifact_root)
        commands = campaign_commands(
            root=root,
            artifact_root=artifact_root,
            cargo_target_dir=_resolve(root, args.cargo_target_dir),
            llvm_tools_dir=_resolve(root, args.llvm_tools_dir),
            temporary_dir=_resolve(root, args.temporary_dir),
            mode=campaign_mode,
            pilot_ids=pilot_ids,
            resume=args.resume,
        )
        plan = {
            "mode": campaign_mode,
            "live_denominator": len(facts),
            "route_counts": {
                route: len(discovery_ids)
                for route, discovery_ids in sorted(routes.items())
            },
            "pilot_ids": pilot_ids,
            "stages": [
                {"label": label, "command": command}
                for label, command in commands
            ],
        }
        if args.execute:
            print(
                "catalogue campaign start: "
                f"mode={campaign_mode}, denominator={len(facts)}, "
                + ", ".join(
                    f"{route}={len(discovery_ids)}"
                    for route, discovery_ids in sorted(routes.items())
                ),
                flush=True,
            )
            artifact_root.mkdir(parents=True, exist_ok=True)
            execute_campaign_commands(commands, root=root)
        else:
            print(json.dumps(plan, indent=2, sort_keys=True))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"catalogue campaign failed: {error}", file=sys.stderr)
        return 2
    return 0


def _git_dirty(root: Path) -> bool:
    completed = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise ValueError("cannot inspect catalogue campaign checkout state")
    return bool(completed.stdout.strip())


def _resolve(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


if __name__ == "__main__":
    raise SystemExit(main())
