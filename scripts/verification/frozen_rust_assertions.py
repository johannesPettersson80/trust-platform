"""Extract one exact frozen Rust assertion batch without a repository scan."""

from __future__ import annotations

import argparse
import json
import os
import tempfile
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .mapping_assertions import invoke_rust_assertion_inventory


DENOMINATOR_PATH = Path("verification/test-catalog-denominator.toml")
MAPPING_DEBT_REASON = "no_reviewed_spec_or_invariant_binding"


def select_frozen_facts(
    *,
    root: Path,
    path: str,
    reason_code: str,
    names: Sequence[str] = (),
) -> list[dict[str, Any]]:
    denominator = tomllib.loads((root / DENOMINATOR_PATH).read_text())
    requested_names = set(names)
    if len(requested_names) != len(names):
        raise ValueError("requested names must be distinct")
    facts = [
        {
            "discovery_id": str(row["discovery_id"]),
            "discovery_source_kind": str(row["discovery_source_kind"]),
            "path": str(row["path"]),
            "name": str(row["name"]),
        }
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("path") == path
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code") == reason_code
        and (not requested_names or row.get("name") in requested_names)
    ]
    selected_names = {row["name"] for row in facts}
    if requested_names and selected_names != requested_names:
        missing = sorted(requested_names - selected_names)
        raise ValueError(
            "requested names are absent from frozen mapping debt: "
            + ", ".join(missing)
        )
    if not facts:
        raise ValueError("frozen Rust assertion selection is empty")
    return sorted(facts, key=lambda row: (row["path"], row["name"]))


def _write_json_atomic(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    content = json.dumps(document, sort_keys=True, indent=2) + "\n"
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        dir=path.parent,
        delete=False,
    ) as temporary:
        temporary.write(content)
        temporary.flush()
        os.fsync(temporary.fileno())
        temporary_path = Path(temporary.name)
    temporary_path.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--path", required=True)
    parser.add_argument(
        "--reason-code",
        default=MAPPING_DEBT_REASON,
    )
    parser.add_argument("--name", action="append", default=[])
    parser.add_argument("--expected-count", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve()
    facts = select_frozen_facts(
        root=root,
        path=args.path,
        reason_code=args.reason_code,
        names=args.name,
    )
    if len(facts) != args.expected_count:
        raise ValueError(
            f"frozen Rust assertion selection has {len(facts)} rows, "
            f"expected {args.expected_count}"
        )
    inventories = invoke_rust_assertion_inventory(
        root,
        facts,
        environment=os.environ,
        workspace_root=root,
    )
    output = args.output if args.output.is_absolute() else root / args.output
    _write_json_atomic(
        output,
        {"schema_version": 2, "inventories": inventories},
    )
    print(
        f"frozen Rust assertions extracted: {len(inventories)} -> "
        f"{output.relative_to(root)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
