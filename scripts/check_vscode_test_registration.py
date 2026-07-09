#!/usr/bin/env python3
"""Check that every VS Code extension test file is explicitly registered."""

from __future__ import annotations

import argparse
from pathlib import Path

from verification.test_catalog_vscode_registration import audit_vscode_test_registration


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    audit = audit_vscode_test_registration(args.root)
    if not audit.is_clean:
        print("VS Code test registration validation failed:")
        for item in audit.diagnostics:
            print(f"- {item.path}:{item.line}: {item.kind}: {item.message}")
        return 1
    print(
        "VS Code test registration validated: "
        f"{len(audit.test_files)} files, {len(audit.entries)} registrations"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
