"""Phase 16 product-change readiness and standing-row guard."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path, PurePosixPath

from .metadata_validator.constants import ROOT


BOARD_PATH = Path(
    "docs/internal/testing/checklists/plc-verification-program/implementation-board.md"
)
PHASE16_READINESS_ROWS = (
    "VERIF-P16-000",
    "VERIF-P16-000A",
    "VERIF-P16-000B",
    "VERIF-P16-000C",
)
PHASE16_REVIEW_ROW = "VERIF-P16-000D"
PHASE16_PILOT_ROW = "VERIF-P16-001"
CHECKBOX_RE = re.compile(
    r"^- \[(?P<state>[ xX])\] `(?P<row_id>VERIF-[^`]+)`",
    re.MULTILINE,
)


def validate_phase16_readiness(board: str, changed_files: list[str]) -> list[str]:
    """Return fail-closed row and product-fence findings."""

    failures: list[str] = []
    states: dict[str, list[bool]] = {}
    for match in CHECKBOX_RE.finditer(board):
        states.setdefault(match.group("row_id"), []).append(
            match.group("state").lower() == "x"
        )

    required = (*PHASE16_READINESS_ROWS, PHASE16_REVIEW_ROW, PHASE16_PILOT_ROW)
    for row_id in required:
        if len(states.get(row_id, [])) != 1:
            failures.append(f"{row_id} must appear exactly once on the implementation board")

    product_paths = sorted(
        {
            normalized
            for value in changed_files
            if (normalized := normalize_changed_path(value)) is not None
            and is_product_path(normalized)
        }
    )
    if not product_paths:
        return failures

    for row_id in PHASE16_READINESS_ROWS:
        row_states = states.get(row_id, [])
        if len(row_states) != 1 or not row_states[0]:
            failures.append(f"product changes are blocked while {row_id} is open")
    review_states = states.get(PHASE16_REVIEW_ROW, [])
    if len(review_states) != 1 or not review_states[0]:
        failures.append(
            f"product changes are blocked until {PHASE16_REVIEW_ROW} records independent acceptance"
        )
    if any("product changes are blocked" in failure for failure in failures):
        failures.extend(f"blocked Phase 16 product path: {path}" for path in product_paths)
    return failures


def normalize_changed_path(value: str) -> str | None:
    normalized = value.strip().replace("\\", "/")
    while normalized.startswith("./"):
        normalized = normalized[2:]
    if not normalized:
        return None
    path = PurePosixPath(normalized)
    if path.is_absolute() or ".." in path.parts:
        return normalized
    return path.as_posix()


def is_product_path(path: str) -> bool:
    """Classify shipped code/test surfaces; malformed paths fail safe."""

    parsed = PurePosixPath(path)
    if parsed.is_absolute() or ".." in parsed.parts:
        return True
    if path in {"Cargo.toml", "Cargo.lock"}:
        return True
    if path.startswith("crates/"):
        return not path.startswith("crates/verification-cases/")
    return path.startswith(("editors/", "hmi/", "libraries/", "third_party/"))


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check the Phase 16 product-change readiness fence."
    )
    parser.add_argument("--board", default=str(ROOT / BOARD_PATH))
    parser.add_argument("--changed-file", action="append", default=[])
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    board_path = Path(args.board)
    try:
        board = board_path.read_text()
    except OSError as exc:
        print(f"Phase 16 readiness check failed: cannot read {board_path}: {exc}", file=sys.stderr)
        return 1
    failures = validate_phase16_readiness(board, list(args.changed_file))
    if failures:
        print("Phase 16 readiness check failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print(
        "Phase 16 readiness validated: "
        f"{len(args.changed_file)} changed paths; product fence ready"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
