import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts import check_regression_test_first as regression_test_first
from scripts.bump_version import LOCKFILES, bump
from scripts.changelog_fragments import check, gather, sections
from scripts.verification.direct_change_contract import validate_direct_change_contract

# Specification 22, "Change and release workflow": a user-visible change adds a fragment under
# `changelog.d/`, `## [Unreleased]` stays empty, and the version bump gathers the fragments into the
# version's section, the newest first, under an empty `## [Unreleased]`.
CHANGELOG = """# Changelog

Intro.

## [Unreleased]

## [0.28.0] - 2026-11-01

### Fixed

- A fix in 0.28.0.
"""

FRAGMENT = """### Added

- A new thing,
  over two lines.

### Fixed

- A fix.
- Another fix.
"""


def write(root: Path, files: dict[str, str]) -> None:
    for name, text in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


class FragmentTests(unittest.TestCase):
    def test_a_fragment_reads_as_its_sections_and_entries(self) -> None:
        self.assertEqual(
            sections(FRAGMENT, "f.md"),
            {"Added": ["- A new thing,\n  over two lines."], "Fixed": ["- A fix.", "- Another fix."]},
        )

    def test_what_a_fragment_may_not_hold_is_named(self) -> None:
        # Catches a fragment the bump would gather into a broken release text.
        for text, words in [
            ("### Bugfixes\n\n- A fix.\n", "a heading is one of"),
            ("## [0.29.0] - 2026-11-09\n\n### Fixed\n\n- A fix.\n", "a heading is one of"),
            ("A fix without a section.\n", "an entry begins with `- `"),
            ("- A fix.\n", "an entry comes under a `### ` section"),
            ("### Fixed\n\n- A fix.\n\n### Fixed\n\n- Again.\n", "comes twice"),
            ("### Fixed\n\n### Added\n\n- A thing.\n", "`### Fixed` has no entry"),
            ("### Fixed\n\n  an orphan continuation\n", "an indented line continues an entry"),
        ]:
            with self.assertRaises(ValueError, msg=text) as raised:
                sections(text, "f.md")
            self.assertIn(words, str(raised.exception), text)

    def test_check_names_a_bad_fragment_and_a_filled_unreleased(self) -> None:
        # Catches an entry added to `CHANGELOG.md` directly, which conflicts with every other pull
        # request that does the same, and a fragment name the bump cannot order.
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            write(
                root,
                {
                    "CHANGELOG.md": CHANGELOG,
                    "changelog.d/README.md": "How to write a fragment.\n",
                    "changelog.d/live-values-timeout.md": FRAGMENT,
                },
            )
            self.assertEqual(check(root), [])
            write(root, {"changelog.d/Live Values.md": FRAGMENT})
            filled = CHANGELOG.replace("## [Unreleased]\n", "## [Unreleased]\n\n### Fixed\n\n- Direct.\n")
            write(root, {"CHANGELOG.md": filled})
            problems = check(root)
            self.assertEqual(len(problems), 2, problems)
            self.assertIn("named in lower case with hyphens", problems[0])
            self.assertIn("`## [Unreleased]` stays empty", problems[1])


    def test_check_names_a_change_to_a_released_section(self) -> None:
        # Catches a pull request written before a release whose entry, appended to the old `## [Unreleased]`, merged
        # into the section the bump released (the carries of #166 and #178, 1 November 2026): the GitHub release
        # would no longer match the changelog. A bump that adds a new section above leaves the released ones alone.
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            write(root, {"CHANGELOG.md": CHANGELOG, "changelog.d/README.md": "How to write a fragment.\n"})
            self.assertEqual(check(root, CHANGELOG), [])
            moved_in = CHANGELOG.replace("- A fix in 0.28.0.\n", "- A fix in 0.28.0.\n- Written before 0.28.0.\n")
            write(root, {"CHANGELOG.md": moved_in})
            problems = check(root, CHANGELOG)
            self.assertEqual(len(problems), 1, problems)
            self.assertIn("the released section [0.28.0] changed", problems[0])
            bumped = CHANGELOG.replace(
                "## [Unreleased]\n", "## [Unreleased]\n\n## [0.29.0] - 2026-11-09\n\n### Fixed\n\n- A fix in 0.29.0.\n"
            )
            write(root, {"CHANGELOG.md": bumped})
            self.assertEqual(check(root, CHANGELOG), [])


class GatherTests(unittest.TestCase):
    def test_the_release_section_takes_the_fragments_the_newest_first(self) -> None:
        # Catches entries lost, a section out of Keep a Changelog's order, and a filled
        # `## [Unreleased]` after the bump.
        newest = {"Fixed": ["- The newest fix."]}
        older = sections(FRAGMENT, "older.md")
        legacy = CHANGELOG.replace("## [Unreleased]\n", "## [Unreleased]\n\n### Fixed\n\n- Written before fragments.\n")
        self.assertEqual(
            gather(legacy, [newest, older], "0.29.0", "2026-11-09"),
            """# Changelog

Intro.

## [Unreleased]

## [0.29.0] - 2026-11-09

### Added

- A new thing,
  over two lines.

### Fixed

- The newest fix.
- A fix.
- Another fix.
- Written before fragments.

## [0.28.0] - 2026-11-01

### Fixed

- A fix in 0.28.0.
""",
        )

    def test_a_release_without_entries_is_refused(self) -> None:
        # Catches an empty release text, which the release workflow would refuse only later.
        with self.assertRaises(ValueError) as raised:
            gather(CHANGELOG, [], "0.29.0", "2026-11-09")
        self.assertIn("nothing to release", str(raised.exception))


def git(root: Path, *args: str) -> str:
    # A fixed identity, so that a commit needs nothing of the configuration of the machine that runs the test.
    identity = ["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false"]
    return subprocess.run(["git", *identity, *args], cwd=root, check=True, capture_output=True, text=True).stdout


def write_checkout_at_0_28_0(root: Path) -> None:
    """A git checkout at 0.28.0 with every file the version bump changes: each Cargo lockfile holds a workspace crate
    and a registry package, and `changelog.d/` one fragment."""
    package_lock = {
        "name": "trust-lsp",
        "version": "0.28.0",
        "lockfileVersion": 3,
        "packages": {"": {"name": "trust-lsp", "version": "0.28.0", "license": "MIT OR Apache-2.0"}},
    }
    lock = '# lock\n\n[[package]]\nname = "trust-hir"\nversion = "0.28.0"\n\n[[package]]\nname = "serde"\nversion = "0.28.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n'
    write(
        root,
        {
            "Cargo.toml": '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.28.0"\nedition = "2024"\n\n[workspace.dependencies]\n',
            "firmware/trust-esp32c6/Cargo.toml": '[package]\nname = "trust-esp32c6"\nversion = "0.28.0"\n',
            **{name: lock for name in LOCKFILES},
            "editors/vscode/package.json": '{\n  "name": "trust-lsp",\n  "version": "0.28.0"\n}\n',
            "editors/vscode/package-lock.json": json.dumps(package_lock, indent=2) + "\n",
            "CHANGELOG.md": CHANGELOG,
            "changelog.d/README.md": "How to write a fragment.\n",
            "changelog.d/a-fix.md": "### Fixed\n\n- A fix in 0.29.0.\n",
        },
    )
    git(root, "init", "--quiet")


class BumpTests(unittest.TestCase):
    def test_a_release_pull_request_needs_no_companion_and_no_regression_test(self) -> None:
        # Specification 30, "Owning-spec companion contract", and specification 22, "Change and release workflow": a
        # change that moves the workspace version and changes only what the bump changes needs neither a test nor a
        # specification. Catches a file the bump writes that these checks do not count as the bump's: the release pull
        # request of 0.39.0 (#472) failed both, since the bump moved five Cargo lockfiles and the checks knew three.
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            write_checkout_at_0_28_0(root)
            git(root, "add", "--all")
            git(root, "commit", "--quiet", "--message", "0.28.0")
            bump(root, "0.28.0", "0.29.0", "2026-11-09")
            git(root, "add", "--all")
            changed = git(root, "diff", "--cached", "--name-only").split()
            self.assertEqual(
                sorted(changed),
                sorted(
                    [
                        "CHANGELOG.md",
                        "Cargo.toml",
                        "changelog.d/a-fix.md",
                        "editors/vscode/package-lock.json",
                        "editors/vscode/package.json",
                        "firmware/trust-esp32c6/Cargo.toml",
                        *LOCKFILES,
                    ]
                ),
            )
            self.assertEqual(validate_direct_change_contract(changed, version_bump=True), [])
            self.assertEqual(
                regression_test_first.findings_for_inputs(
                    changed=changed, text="Release 0.29.0\n\nIt fixes #1.", inline_test_added=False, version_bump=True
                ),
                [],
            )

    def test_the_bump_moves_every_version_and_gathers_the_fragments(self) -> None:
        # Catches a version file left behind and a fragment kept after its release.
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            write_checkout_at_0_28_0(root)
            report = bump(root, "0.28.0", "0.29.0", "2026-11-09")
            self.assertIn("1 changelog fragments gathered", report[-1])
            self.assertIn('version = "0.29.0"', (root / "Cargo.toml").read_text())
            self.assertIn('version = "0.29.0"', (root / "firmware/trust-esp32c6/Cargo.toml").read_text())
            for name in (
                "Cargo.lock",
                "firmware/trust-esp32c6/Cargo.lock",
                "firmware/trust-stm32u5a9/Cargo.lock",
                "fuzz/Cargo.lock",
                "crates/trust-ads-server/fuzz/Cargo.lock",
            ):
                text = (root / name).read_text()
                self.assertIn('name = "trust-hir"\nversion = "0.29.0"', text, name)
                self.assertIn('name = "serde"\nversion = "0.28.0"', text, name)
            self.assertIn('"version": "0.29.0"', (root / "editors/vscode/package.json").read_text())
            lock_after = json.loads((root / "editors/vscode/package-lock.json").read_text())
            self.assertEqual((lock_after["version"], lock_after["packages"][""]["version"]), ("0.29.0", "0.29.0"))
            self.assertIn(
                "## [Unreleased]\n\n## [0.29.0] - 2026-11-09\n\n### Fixed\n\n- A fix in 0.29.0.\n\n## [0.28.0]",
                (root / "CHANGELOG.md").read_text(),
            )
            self.assertEqual(sorted(path.name for path in (root / "changelog.d").iterdir()), ["README.md"])


if __name__ == "__main__":
    unittest.main()
