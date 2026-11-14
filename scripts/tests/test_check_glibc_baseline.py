"""The release's check of the glibc that each Linux binary needs (specification 24, "Platform Support Matrix")."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_glibc_baseline as check  # noqa: E402

# `readelf -W -V` and `readelf -W --dyn-syms` of the linux-x64 `trust-runtime` of 0.41.1, built on the hosted
# runner's Ubuntu 26.04 (lines cut to those about glibc).
NATIVE_VERSIONS = """\
Version needs section '.gnu.version_r' contains 3 entries:
  0x0010: Version: 1  File: libm.so.6  Cnt: 3
  0x0060:   Name: GLIBC_2.2.5  Flags: none  Version: 29
  0x0070:   Name: GLIBC_2.29  Flags: none  Version: 12
  0x0080:   Name: GLIBC_2.43  Flags: none  Version: 30
  0x0020: Version: 1  File: libc.so.6  Cnt: 24
  0x0090:   Name: GLIBC_2.2.5  Flags: none  Version: 3
  0x00c0:   Name: GLIBC_2.3.4  Flags: none  Version: 10
  0x0120:   Name: GLIBC_2.14  Flags: none  Version: 5
  0x01a0:   Name: GLIBC_2.29  Flags: WEAK  Version: 23
  0x01e0:   Name: GLIBC_2.34  Flags: none  Version: 2
  0x01f0:   Name: GLIBC_2.38  Flags: none  Version: 31
  0x0200:   Name: GLIBC_2.39  Flags: WEAK  Version: 22
"""
NATIVE_SYMBOLS = """\
   134: 0000000000000000     0 FUNC    WEAK   DEFAULT  UND pidfd_spawnp@GLIBC_2.39 (22)
   150: 0000000000000000     0 FUNC    WEAK   DEFAULT  UND pidfd_getpid@GLIBC_2.39 (22)
   189: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND asinf@GLIBC_2.43 (30)
   230: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND __isoc23_strtol@GLIBC_2.38 (31)
   247: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND __isoc23_strtoul@GLIBC_2.38 (31)
   278: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND __isoc23_sscanf@GLIBC_2.38 (31)
   301: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND exp@GLIBC_2.29 (12)
   302: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND __libc_start_main@GLIBC_2.34 (2)
"""

# `readelf -W -V` of the linux-arm64 `trust-runtime` of 0.41.1, built with `cross`.
CROSS_VERSIONS = """\
Version needs section '.gnu.version_r' contains 6 entries:
 Addr: 0x0000000000009548  Offset: 0x00009548  Link: 6 (.dynstr)
  000000: Version: 1  File: libm.so.6  Cnt: 1
  0x0010:   Name: GLIBC_2.17  Flags: none  Version: 7
  0x0020: Version: 1  File: ld-linux-aarch64.so.1  Cnt: 1
  0x0030:   Name: GLIBC_2.17  Flags: none  Version: 6
  0x0060: Version: 1  File: libgcc_s.so.1  Cnt: 3
  0x0070:   Name: GCC_4.2.0  Flags: none  Version: 10
  0x00c0: Version: 1  File: libc.so.6  Cnt: 2
  0x00d0:   Name: GLIBC_2.18  Flags: none  Version: 8
  0x00e0:   Name: GLIBC_2.17  Flags: none  Version: 2
"""


class GlibcBaselineTests(unittest.TestCase):
    def test_a_binary_built_on_a_newer_glibc_is_refused_with_each_version_and_its_symbols(self) -> None:
        # Catches 0.41.1's linux-x64 runtime passing: it needed glibc 2.43 for `asinf` and did not start on Ubuntu 24.04
        # or Debian 13. Every version above 2.28 is named, each with the functions that need it, so the cause is plain.
        found = check.problems("trust-runtime", NATIVE_VERSIONS, NATIVE_SYMBOLS)
        self.assertEqual(
            [line.split(" (")[0] for line in found],
            [
                "trust-runtime needs glibc 2.29",
                "trust-runtime needs glibc 2.34",
                "trust-runtime needs glibc 2.38",
                "trust-runtime needs glibc 2.43",
            ],
        )
        self.assertIn("(asinf)", found[3])
        self.assertIn("(__isoc23_sscanf, __isoc23_strtol, __isoc23_strtoul)", found[2])
        self.assertIn("the oldest that the Linux packages support", found[3])

    def test_a_weak_need_does_not_count(self) -> None:
        # Catches a refusal for `pidfd_spawnp@GLIBC_2.39`, which the binary binds weakly: it starts without it.
        self.assertEqual(check.needed_versions(NATIVE_VERSIONS)[-1], (2, 43))
        self.assertNotIn((2, 39), check.needed_versions(NATIVE_VERSIONS))

    def test_the_cross_built_binary_passes(self) -> None:
        # Catches a check that refuses the arm64 package, which runs on glibc 2.17 and later.
        self.assertEqual(check.problems("trust-runtime", CROSS_VERSIONS, ""), [])
        self.assertEqual(check.needed_versions(CROSS_VERSIONS), [(2, 17), (2, 18)])

    def test_the_baseline_itself_passes_and_the_next_version_does_not(self) -> None:
        # Catches an off-by-one at the baseline, and versions compared as text (2.3 against 2.28).
        need = "  0x0010:   Name: GLIBC_{}  Flags: none  Version: 2\n"
        self.assertEqual(check.problems("b", need.format("2.28"), ""), [])
        self.assertEqual(check.problems("b", need.format("2.3.4"), ""), [])
        self.assertEqual(len(check.problems("b", need.format("2.29"), "")), 1)


if __name__ == "__main__":
    unittest.main()
