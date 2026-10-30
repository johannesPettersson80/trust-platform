import unittest

from scripts.extract_changelog_section import RELEASE_TEXT_LIMIT, extract_section, release_text

# AGENTS.md, Releases: the version bump turns `## [Unreleased]` into the version's own section, and
# the release's text on GitHub is that section under the tag as its title.
CHANGELOG = """# Changelog

## [Unreleased]

### Added

- Not released yet.

## [0.25.2] - 2026-11-03

### Fixed

- A fix in 0.25.2.

## [0.25.1] - 2026-10-29

### Fixed

- A fix in 0.25.1.
"""


class ReleaseTextTests(unittest.TestCase):
    def test_a_release_takes_its_own_section_and_no_other(self) -> None:
        # Catches the old release text, which was `[Unreleased]`, and a section that runs on into
        # the older versions below it.
        self.assertEqual(
            release_text(CHANGELOG, "0.25.2", "v0.25.2"),
            "# v0.25.2\n\n### Fixed\n\n- A fix in 0.25.2.\n",
        )

    def test_the_oldest_section_ends_with_the_file(self) -> None:
        self.assertEqual(extract_section(CHANGELOG, "0.25.1"), "### Fixed\n\n- A fix in 0.25.1.")

    def test_a_version_without_a_section_is_refused(self) -> None:
        with self.assertRaisesRegex(ValueError, r"\[0\.25\.3\]"):
            release_text(CHANGELOG, "0.25.3", "v0.25.3")

    def test_an_empty_section_is_refused(self) -> None:
        empty = CHANGELOG.replace(
            "## [0.25.2] - 2026-11-03\n", "## [0.25.3] - 2026-11-04\n\n## [0.25.2] - 2026-11-03\n"
        )
        with self.assertRaisesRegex(ValueError, "empty"):
            release_text(empty, "0.25.3", "v0.25.3")

    def test_a_release_text_longer_than_github_keeps_is_refused(self) -> None:
        # GitHub cut 0.24.75's and 0.25.1's release texts, the whole changelog then, mid-word at its
        # limit. "# v1.0.0" and the blank line after it are 10 characters and the closing newline
        # is 1, so a section of 124,989 characters makes a text of exactly 125,000.
        def changelog(section_length: int) -> str:
            return f"## [1.0.0] - 2026-11-03\n\n{'x' * section_length}\n"

        self.assertEqual(len(release_text(changelog(124_989), "1.0.0", "v1.0.0")), RELEASE_TEXT_LIMIT)
        with self.assertRaisesRegex(ValueError, "125000"):
            release_text(changelog(124_990), "1.0.0", "v1.0.0")


if __name__ == "__main__":
    unittest.main()
