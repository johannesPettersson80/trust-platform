"""Write one frozen focused-execution manifest for reviewed authority routes."""

from __future__ import annotations

import argparse
import json
import os
import tempfile
import tomllib
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .authority_route_execution_plan import (
    build_authority_route_execution_plan,
)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--routes", type=Path, required=True)
    parser.add_argument("--facts", type=Path, required=True)
    parser.add_argument(
        "--denominator",
        type=Path,
        default=Path("verification/test-catalog-denominator.toml"),
    )
    parser.add_argument(
        "--catalog",
        type=Path,
        default=Path("verification/test-catalog.toml"),
    )
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)

    routes_document = _json_object(args.routes, "authority routes")
    facts_document = _json_object(args.facts, "scanner facts")
    denominator = tomllib.loads(args.denominator.read_text())
    catalog = tomllib.loads(args.catalog.read_text())
    result = build_authority_route_execution_plan(
        routes=_array(routes_document, "routes", "authority routes"),
        facts=_array(facts_document, "inferred_facts", "scanner facts"),
        denominator_reviews=_array(denominator, "reviews", "denominator"),
        catalog_tests=_array(catalog, "tests", "catalog"),
    )
    _write_json_atomic(args.out, result)
    print(
        "authority-route execution plan: "
        f"{result['summary']['routes']} routes -> "
        f"{result['summary']['batches']} batches"
    )
    return 0


def _json_object(path: Path, label: str) -> Mapping[str, Any]:
    document = json.loads(path.read_text())
    if not isinstance(document, Mapping):
        raise ValueError(f"{label} must be a JSON object")
    return document


def _array(
    document: Mapping[str, Any],
    field: str,
    label: str,
) -> list[Mapping[str, Any]]:
    rows = document.get(field)
    if not isinstance(rows, list) or any(
        not isinstance(row, Mapping) for row in rows
    ):
        raise ValueError(f"{label} requires a {field} array")
    return rows


def _write_json_atomic(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    content = json.dumps(document, indent=2, ensure_ascii=False) + "\n"
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
        delete=False,
    ) as temporary:
        temporary.write(content)
        temporary_path = Path(temporary.name)
    os.replace(temporary_path, path)


if __name__ == "__main__":
    raise SystemExit(main())
