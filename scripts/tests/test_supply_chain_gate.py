"""The supply chain gate reads every Cargo lock of the repository (specification 24, "Dependency Exceptions").

Each lock is its own graph: `cargo deny --locked` fails when its manifests have left it behind, and both cargo deny
and cargo audit check its advisories. Until G75 the gate read the root and the two firmware locks only, so the locks
of the two fuzz harnesses drifted from their manifests unseen, and `fuzz/Cargo.lock` still held rustix 1.1.5
(RUSTSEC-2026-0486) after the root lock had left it.
"""

import os
import unittest
from pathlib import Path

from scripts.bump_version import LOCKFILES

ROOT = Path(__file__).resolve().parents[2]
GATE = ROOT / "scripts" / "supply_chain_gate.sh"
# Build output and installed packages, never a workspace of the repository.
SKIPPED = {"target", "node_modules"}


def cargo_locks() -> list[str]:
    """The repository's Cargo locks, relative to its root. Hidden folders hold no workspace of it."""
    locks = []
    for folder, subfolders, files in os.walk(ROOT):
        subfolders[:] = sorted(
            name for name in subfolders if name not in SKIPPED and not name.startswith(".")
        )
        if "Cargo.lock" in files:
            locks.append((Path(folder) / "Cargo.lock").relative_to(ROOT).as_posix())
    return sorted(locks)


class SupplyChainGateGraphTests(unittest.TestCase):
    def test_every_cargo_lock_is_read_locked_by_cargo_deny_and_cargo_audit(self) -> None:
        """Catches a lock that the gate does not read, whose drift and advisories would pass unseen."""
        gate = GATE.read_text(encoding="utf-8")
        locks = cargo_locks()
        self.assertIn("Cargo.lock", locks)
        for lock in locks:
            if lock == "Cargo.lock":
                deny = "cargo deny --locked check "
            else:
                deny = f"cargo deny --locked --manifest-path {lock.removesuffix('/Cargo.lock')}/Cargo.toml "
            for command in (deny, f"run_cargo_audit {lock} "):
                with self.subTest(lock=lock, command=command.strip()):
                    self.assertTrue(command in gate, f"the gate does not run `{command.strip()}`")

    def test_the_version_bump_moves_every_cargo_lock(self) -> None:
        """Catches a lock that the bump leaves behind. Each lock lists the workspace crates it builds at the
        workspace version, and the gate reads it `--locked`, so a lock the bump does not move fails the gate on
        the release pull request (0.38.0 left both fuzz locks at 0.37.0)."""
        self.assertEqual(sorted(LOCKFILES), cargo_locks())


if __name__ == "__main__":
    unittest.main()
