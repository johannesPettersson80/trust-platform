#!/usr/bin/env python3

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
ASSET_ROOT = REPO_ROOT / "docs/public/assets/images"

COPY_MAP: list[tuple[Path, Path]] = [
    (
        REPO_ROOT / "docs/internal/assets/ui-overview.png",
        ASSET_ROOT / "runtime/ui-overview.png",
    ),
    (
        REPO_ROOT / "docs/internal/assets/ui-io.png",
        ASSET_ROOT / "runtime/ui-io.png",
    ),
    (
        REPO_ROOT / "docs/internal/assets/ui-setup.png",
        ASSET_ROOT / "runtime/ui-setup.png",
    ),
    (
        REPO_ROOT / "editors/vscode/assets/screenshot-diagnostics.png",
        ASSET_ROOT / "vscode/diagnostics.png",
    ),
    (
        REPO_ROOT / "editors/vscode/assets/screenshot-refactor.png",
        ASSET_ROOT / "vscode/refactor.png",
    ),
    (
        REPO_ROOT / "editors/vscode/assets/screenshot-debug.png",
        ASSET_ROOT / "vscode/debug.png",
    ),
    (
        REPO_ROOT / "editors/vscode/assets/screenshot-ladder-editor.png",
        ASSET_ROOT / "visual-editors/ladder.png",
    ),
    (
        REPO_ROOT / "editors/vscode/assets/screenshot-statechart-editor.png",
        ASSET_ROOT / "visual-editors/statechart.png",
    ),
    (
        REPO_ROOT / "editors/vscode/assets/screenshot-blockly-editor.png",
        ASSET_ROOT / "visual-editors/blockly.png",
    ),
    (
        REPO_ROOT / "editors/vscode/assets/screenshot-sfc-editor.png",
        ASSET_ROOT / "visual-editors/sfc.png",
    ),
    (
        REPO_ROOT / "examples/tutorials/12_hmi_pid_process_dashboard/hmi/plant.svg",
        ASSET_ROOT / "hmi/plant.svg",
    ),
    (
        REPO_ROOT / "examples/tutorials/12_hmi_pid_process_dashboard/hmi/plant-minimal.svg",
        ASSET_ROOT / "hmi/plant-minimal.svg",
    ),
]

VIDEO_STILL_FALLBACKS: list[tuple[Path, Path, str]] = [
    (
        REPO_ROOT / "examples/ladder/ladder-editor.mp4",
        ASSET_ROOT / "visual-editors/ladder.png",
        "00:00:02.000",
    ),
    (
        REPO_ROOT / "examples/statecharts/stachart-snake.mp4",
        ASSET_ROOT / "visual-editors/statechart.png",
        "00:00:02.000",
    ),
    (
        REPO_ROOT / "examples/blockly/Blockly-snake.mp4",
        ASSET_ROOT / "visual-editors/blockly.png",
        "00:00:02.000",
    ),
]


def run(cmd: list[str]) -> None:
    subprocess.run(cmd, check=True, cwd=REPO_ROOT)


def ensure_parent(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)


def copy_asset(source: Path, dest: Path) -> None:
    if not source.exists():
        raise FileNotFoundError(f"missing asset source: {source}")
    ensure_parent(dest)
    shutil.copy2(source, dest)


def extract_frame(source: Path, dest: Path, timestamp: str) -> None:
    if not source.exists():
        raise FileNotFoundError(f"missing video source: {source}")
    ensure_parent(dest)
    run(
        [
            "ffmpeg",
            "-y",
            "-loglevel",
            "error",
            "-ss",
            timestamp,
            "-i",
            str(source),
            "-frames:v",
            "1",
            str(dest),
        ]
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate public-doc screenshots and media assets."
    )
    parser.add_argument(
        "--regenerate-vscode",
        action="store_true",
        help="Regenerate VS Code screenshots with the existing auto-capture script before syncing assets.",
    )
    parser.add_argument(
        "--regenerate-visual-editors",
        action="store_true",
        help="Regenerate the public-doc visual-editor screenshots before syncing assets.",
    )
    args = parser.parse_args()

    if args.regenerate_vscode:
        run([str(REPO_ROOT / "scripts/capture-readme-screenshots-auto.sh")])
    if args.regenerate_visual_editors:
        run([str(REPO_ROOT / "scripts/capture-public-docs-visual-editors.sh")])

    generated: list[Path] = []
    copied_targets: set[Path] = set()
    fallback_by_dest = {dest: (source, timestamp) for source, dest, timestamp in VIDEO_STILL_FALLBACKS}
    for source, dest in COPY_MAP:
        if source.exists():
            copy_asset(source, dest)
            copied_targets.add(dest)
            generated.append(dest)
            continue
        if dest.exists():
            copied_targets.add(dest)
            generated.append(dest)
            continue
        if dest not in fallback_by_dest:
            raise FileNotFoundError(f"missing asset source: {source}")

    pending_fallbacks = [
        (source, dest, timestamp)
        for source, dest, timestamp in VIDEO_STILL_FALLBACKS
        if dest not in copied_targets
    ]

    if pending_fallbacks and shutil.which("ffmpeg") is None:
        print("ffmpeg is required to extract visual-editor stills", file=sys.stderr)
        return 1

    for source, dest, timestamp in pending_fallbacks:
        extract_frame(source, dest, timestamp)
        generated.append(dest)

    print("generated public docs media:")
    for path in generated:
        print(f"  - {path.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
