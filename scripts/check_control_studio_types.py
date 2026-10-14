#!/usr/bin/env python3
"""Check that the committed Control Studio TypeScript declarations match the engine's message types.

Exports the engine's JSON Schema with trust-control's export-schema into a temporary folder, regenerates the
declarations with editors/vscode/scripts/generate-control-studio-types.js, and fails when the committed files in
editors/vscode/src/controlStudio/generated differ from them.
"""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args()
    root: Path = args.root
    committed_folder = root / "editors" / "vscode" / "src" / "controlStudio" / "generated"

    with tempfile.TemporaryDirectory(prefix="control-studio-types-") as scratch:
        schema_folder = Path(scratch) / "schema"
        output_folder = Path(scratch) / "generated"
        subprocess.run(
            [
                "cargo",
                "run",
                "--quiet",
                "-p",
                "trust-control",
                "--bin",
                "export-schema",
                "--",
                str(schema_folder),
            ],
            cwd=root,
            check=True,
        )
        subprocess.run(
            [
                "node",
                "scripts/generate-control-studio-types.js",
                str(schema_folder),
                str(output_folder),
            ],
            cwd=root / "editors" / "vscode",
            check=True,
        )
        regenerated = {path.name: path.read_bytes() for path in output_folder.glob("*.ts")}

    committed = {path.name: path.read_bytes() for path in committed_folder.glob("*.ts")}
    problems: list[str] = []
    for name in sorted(regenerated.keys() - committed.keys()):
        problems.append(f"{name} is produced by the engine's schema but not committed")
    for name in sorted(committed.keys() - regenerated.keys()):
        problems.append(f"{name} is committed but the engine's schema no longer produces it")
    for name in sorted(regenerated.keys() & committed.keys()):
        if regenerated[name] != committed[name]:
            problems.append(f"{name} differs from the engine's schema")

    if problems:
        print("Control Studio TypeScript declarations are stale (regenerate them; see generated/README.md):")
        for problem in problems:
            print(f"- {problem}")
        return 1
    print(f"Control Studio declarations match the engine's schema: {len(regenerated)} files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
