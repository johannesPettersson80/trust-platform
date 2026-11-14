#!/usr/bin/env python3
"""Refuses a Linux release binary that needs a newer glibc than the Linux packages support (specification 24,
"Platform Support Matrix").

The Linux packages run on any Linux with glibc 2.28 or newer. A binary binds each glibc function it calls to the newest
version of it that the glibc it was built against has, and fails to start where that version is missing, with
"version `GLIBC_2.43' not found". From 0.35.0 to 0.41.1, `trust-runtime` and `trust-dev` of the linux-x64 package
were built on the hosted runner's Ubuntu 26.04, bound `asinf` to `GLIBC_2.43`, and did not start on Ubuntu 24.04 or
Debian 13.

Usage: check_glibc_baseline.py BINARY...

It reads each binary's version needs with `readelf`. A weak need does not count: the binary starts without it.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

# The oldest glibc that the Linux packages support.
BASELINE = (2, 28)

_NEED = re.compile(r"Name: GLIBC_(\d+(?:\.\d+)+)\s+Flags: (\S+)")
_SYMBOL = re.compile(r"\s(\S+?)@GLIBC_(\d+(?:\.\d+)+)")


def version(text: str) -> tuple[int, ...]:
    return tuple(int(part) for part in text.split("."))


def shown(numbers: tuple[int, ...]) -> str:
    return ".".join(str(number) for number in numbers)


def needed_versions(version_info: str) -> list[tuple[int, ...]]:
    """The glibc versions that `readelf -V` lists as needed, the weak ones left out, in ascending order."""
    return sorted({version(match.group(1)) for match in _NEED.finditer(version_info) if "WEAK" not in match.group(2)})


def symbols_at(dynamic_symbols: str, needed: tuple[int, ...]) -> list[str]:
    """The undefined symbols that `readelf --dyn-syms` binds to glibc version `needed`."""
    return sorted(
        {
            match.group(1)
            for line in dynamic_symbols.splitlines()
            if " UND " in line
            for match in _SYMBOL.finditer(line)
            if version(match.group(2)) == needed
        }
    )


def problems(name: str, version_info: str, dynamic_symbols: str) -> list[str]:
    """One line for each glibc version newer than the baseline that the binary `name` needs, with its symbols."""
    return [
        f"{name} needs glibc {shown(needed)} ({', '.join(symbols_at(dynamic_symbols, needed)) or 'no symbol named'}), "
        f"newer than glibc {shown(BASELINE)}, the oldest that the Linux packages support"
        for needed in needed_versions(version_info)
        if needed > BASELINE
    ]


def readelf(option: str, binary: Path) -> str:
    result = subprocess.run(["readelf", "-W", option, str(binary)], capture_output=True, text=True, check=False)
    if result.returncode != 0:
        raise SystemExit(f"readelf {option} {binary} failed: {result.stderr.strip()}")
    return result.stdout


def main(arguments: list[str]) -> int:
    if not arguments:
        print(__doc__.strip().split("\n\n")[2], file=sys.stderr)
        return 2
    failed = False
    for name in arguments:
        binary = Path(name)
        version_info = readelf("-V", binary)
        found = problems(name, version_info, readelf("--dyn-syms", binary))
        for line in found:
            print(line, file=sys.stderr)
        if found:
            failed = True
            continue
        newest = needed_versions(version_info)
        print(f"{name}: needs glibc {shown(newest[-1]) if newest else '(none)'} at most")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
