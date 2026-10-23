"""Exhaustive advisory discovery for verification-tooling Python tests.

The public wrapper keeps its historical filename for compatibility. Pull-request
and exact-candidate paths use the bounded report smoke instead.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from typing import TextIO


TEST_ROOT = Path("scripts/verification")
NON_TEST_SUFFIX_COLLISIONS = {
    Path("scripts/verification/metadata_validator/ignored_tests.py"),
}


def discover_test_modules(root: Path) -> list[str]:
    root = root.resolve()
    test_root = root / TEST_ROOT
    modules: list[str] = []
    for path in sorted(test_root.rglob("*_tests.py")):
        relative = path.relative_to(root)
        if relative in NON_TEST_SUFFIX_COLLISIONS:
            continue
        modules.append(".".join(relative.with_suffix("").parts))
    if not modules:
        raise ValueError(f"no verification test modules found under {TEST_ROOT}")
    return modules


def run_focused_test_suite(root: Path, *, stream: TextIO | None = None) -> bool:
    modules = discover_test_modules(root)
    suite = unittest.defaultTestLoader.loadTestsFromNames(modules)
    result = unittest.TextTestRunner(stream=stream, verbosity=1).run(suite)
    return result.wasSuccessful()


def main(argv: list[str] | None = None) -> int:
    root = Path(__file__).resolve().parents[2]
    root_text = str(root)
    if root_text not in sys.path:
        sys.path.insert(0, root_text)
    arguments = sys.argv[1:] if argv is None else argv
    if arguments == ["--list"]:
        print("\n".join(discover_test_modules(root)))
        return 0
    if arguments:
        print("usage: run_verification_focused_tests.py [--list]", file=sys.stderr)
        return 2
    return 0 if run_focused_test_suite(root, stream=sys.stderr) else 1


if __name__ == "__main__":
    raise SystemExit(main())
