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
# The extension's packaged copies of examples/, which its build writes and git ignores (.gitignore:
# /editors/vscode/media/*): their manifests are the examples' own, read where they are.
PACKAGED_COPIES = Path("editors/vscode/media")


def crate_folders() -> list[str]:
    """The top-level folders that hold a manifest of the repository's own crates: every one but the vendored crates
    of third_party/, which keep their upstream manifests (specification 24, "Unused Dependencies"). The root
    manifest is the workspace's and lists no dependencies of a crate."""
    folders = set()
    for folder, subfolders, files in os.walk(ROOT):
        subfolders[:] = sorted(
            name for name in subfolders if name not in SKIPPED and not name.startswith(".")
        )
        relative = Path(folder).relative_to(ROOT)
        if relative == PACKAGED_COPIES:
            subfolders[:] = []
            continue
        if "Cargo.toml" in files and relative.parts and relative.parts[0] != "third_party":
            folders.add(relative.parts[0])
    return sorted(folders)


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

    def test_cargo_machete_reads_every_folder_of_the_repositorys_own_crates(self) -> None:
        """Catches a folder of crates that cargo machete does not read, whose unused dependencies would pass unseen,
        and a vendored crate read, whose manifest is upstream's."""
        gate = GATE.read_text(encoding="utf-8")
        commands = [line for line in gate.splitlines() if line.startswith("cargo machete ")]
        self.assertEqual(len(commands), 1, "the gate runs cargo machete once")
        words = commands[0].split("||")[0].split()
        self.assertEqual([word for word in words[2:] if not word.startswith("-")], crate_folders())

    def test_the_version_bump_moves_every_cargo_lock(self) -> None:
        """Catches a lock that the bump leaves behind. Each lock lists the workspace crates it builds at the
        workspace version, and the gate reads it `--locked`, so a lock the bump does not move fails the gate on
        the release pull request (0.38.0 left both fuzz locks at 0.37.0)."""
        self.assertEqual(sorted(LOCKFILES), cargo_locks())


if __name__ == "__main__":
    unittest.main()
