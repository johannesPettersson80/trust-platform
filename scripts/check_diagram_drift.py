#!/usr/bin/env python3
import argparse
import hashlib
import json
from pathlib import Path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def compute_manifest(root: Path) -> dict[str, str]:
    manifest: dict[str, str] = {}
    for path in sorted(root.glob("docs/diagrams/**/*.puml")):
        manifest[str(path.relative_to(root))] = sha256(path)
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description="Check UML diagram drift.")
    parser.add_argument(
        "--update",
        action="store_true",
        help="Update the diagram manifest instead of checking.",
    )
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    manifest_path = root / "docs" / "diagrams" / "manifest.json"
    current = compute_manifest(root)

    if args.update:
        manifest_path.write_text(json.dumps(current, indent=2, sort_keys=True) + "\n")
        return 0

    if not manifest_path.exists():
        print("diagram manifest missing; run with --update to create it")
        return 1

    stored = json.loads(manifest_path.read_text())
    if stored != current:
        print("diagram drift detected:")
        stored_keys = set(stored.keys())
        current_keys = set(current.keys())
        for missing in sorted(stored_keys - current_keys):
            print(f"  missing file: {missing}")
        for added in sorted(current_keys - stored_keys):
            print(f"  new file: {added}")
        for path in sorted(stored_keys & current_keys):
            if stored[path] != current[path]:
                print(f"  changed file: {path}")
        print("run scripts/check_diagram_drift.py --update to refresh the manifest")
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
