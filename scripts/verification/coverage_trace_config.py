"""Generate the isolated nextest tool configuration for focused tracing."""

from __future__ import annotations

import argparse
from pathlib import Path


def render_nextest_tool_config(wrapper_path: Path) -> str:
    if not wrapper_path.is_absolute():
        raise ValueError("coverage trace wrapper path must be absolute")
    wrapper = wrapper_path.as_posix()
    if any(character in wrapper for character in ('"', "\n", "\r")):
        raise ValueError("coverage trace wrapper path contains unsupported characters")
    return (
        '[scripts.wrapper."@tool:trust-catalog:catalog-trace"]\n'
        f'command = ["python3", "{wrapper}"]\n\n'
        "[[profile.catalog-trace.scripts]]\n"
        'filter = "all()"\n'
        'run-wrapper = "@tool:trust-catalog:catalog-trace"\n'
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wrapper", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    rendered = render_nextest_tool_config(args.wrapper)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
