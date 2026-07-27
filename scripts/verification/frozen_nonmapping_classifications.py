"""Apply reviewed owned-nonmapping reasons without hand-editing the denominator."""

from __future__ import annotations

import argparse
import json
import os
import tempfile
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .mapping_catalog_updates import render_denominator_document
from .test_catalog_denominator import (
    DENOMINATOR_PATH,
    NONMAPPING_REASON_CODES,
    NONMAPPING_REASON_DEFINITIONS,
)

MAPPING_DEBT_REASON = "no_reviewed_spec_or_invariant_binding"


def classify_nonmappings(
    *,
    denominator: Mapping[str, Any],
    classifications: Sequence[Mapping[str, Any]],
    reviewed_date: str,
    allow_mapping_debt: bool = False,
) -> dict[str, Any]:
    """Return a denominator with the exact reviewed identities reclassified."""

    if not reviewed_date:
        raise ValueError("reviewed_date must be non-empty")
    requested: dict[tuple[str, str], str] = {}
    for index, row in enumerate(classifications):
        if set(row) != {"path", "name", "reason_code"}:
            raise ValueError(f"classification {index} fields drift")
        path = _string(row, "path", index)
        name = _string(row, "name", index)
        reason = _string(row, "reason_code", index)
        if reason not in NONMAPPING_REASON_CODES or (
            reason == MAPPING_DEBT_REASON and not allow_mapping_debt
        ):
            raise ValueError(f"classification {index} reason is not owned")
        key = (path, name)
        if key in requested:
            raise ValueError(f"duplicate classification identity {path}:{name}")
        requested[key] = reason
    if not requested:
        raise ValueError("classification set must be non-empty")

    updated = dict(denominator)
    updated["reason_definitions"] = dict(NONMAPPING_REASON_DEFINITIONS)
    seen: set[tuple[str, str]] = set()
    reviews = []
    for raw in denominator.get("reviews", []):
        if not isinstance(raw, Mapping):
            raise ValueError("denominator review must be an object")
        row = dict(raw)
        key = (str(row.get("path")), str(row.get("name")))
        if key in requested:
            if row.get("disposition") != "reviewed_nonmapping":
                raise ValueError(f"{key} is not reviewed_nonmapping")
            allowed_existing = {
                MAPPING_DEBT_REASON,
                requested[key],
            }
            if (
                allow_mapping_debt
                and requested[key] == MAPPING_DEBT_REASON
            ):
                allowed_existing.update(NONMAPPING_REASON_CODES)
            if row.get("reason_code") not in allowed_existing:
                raise ValueError(f"{key} already has a different owned reason")
            row["reason_code"] = requested[key]
            row["last_reviewed"] = reviewed_date
            seen.add(key)
        reviews.append(row)
    missing = sorted(set(requested) - seen)
    if missing:
        raise ValueError(
            "classification identities are absent from denominator: "
            + ", ".join(f"{path}:{name}" for path, name in missing)
        )
    updated["reviews"] = reviews
    updated["last_reviewed"] = reviewed_date
    return updated


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--classifications", type=Path, required=True)
    parser.add_argument("--reviewed-date", required=True)
    parser.add_argument(
        "--reopen-mapping-debt",
        action="store_true",
        help=(
            "permit a reviewed owned-nonmapping row to return to mapping debt "
            "after its oracle has been resolved"
        ),
    )
    parser.add_argument("--apply", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    root = args.root.resolve()
    plan_path = (
        args.classifications
        if args.classifications.is_absolute()
        else root / args.classifications
    )
    try:
        document = tomllib.loads((root / DENOMINATOR_PATH).read_text())
        plan = json.loads(plan_path.read_text())
        rows = plan.get("classifications") if isinstance(plan, Mapping) else None
        if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
            raise ValueError("classification plan lacks classifications array")
        updated = classify_nonmappings(
            denominator=document,
            classifications=rows,
            reviewed_date=args.reviewed_date,
            allow_mapping_debt=args.reopen_mapping_debt,
        )
        rendered = render_denominator_document(updated)
        if args.apply:
            destination = root / DENOMINATOR_PATH
            descriptor, temporary = tempfile.mkstemp(
                prefix=f".{destination.name}.",
                dir=destination.parent,
            )
            try:
                with os.fdopen(descriptor, "w") as handle:
                    handle.write(rendered)
                    handle.flush()
                    os.fsync(handle.fileno())
                os.replace(temporary, destination)
            except Exception:
                try:
                    os.unlink(temporary)
                except FileNotFoundError:
                    pass
                raise
        verb = "applied" if args.apply else "ready"
        print(f"frozen nonmapping classifications {verb}: {len(rows)}")
        return 0
    except (OSError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"frozen nonmapping classification failed: {error}")
        return 2


def _string(row: Mapping[str, Any], field: str, index: int) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"classification {index} {field} must be non-empty")
    return value


if __name__ == "__main__":
    raise SystemExit(main())
