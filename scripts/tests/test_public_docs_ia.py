import unittest

from scripts.check_public_docs_ia import LINK_RE, short_list_blocks, steps_joined_to_a_block


class PublicDocsLinkTests(unittest.TestCase):
    def test_a_lone_bracket_in_a_shortcut_starts_no_link(self) -> None:
        # The HMI Builder page names Ctrl+] and Ctrl+[ before an image. A pattern that let a link's
        # text run from that bracket to the image read the image as a page missing from the
        # navigation, and the documentation site stopped publishing (main, 28 October 2026).
        text = (
            "group; a double-click selects one member. **Ctrl+]** and **Ctrl+[** move the "
            "selection one step in\nthe order.\n\n"
            "![The Layers tab](../assets/images/hmi-builder/layers.png)\n"
        )
        self.assertEqual(LINK_RE.findall(text), [])

    def test_links_with_plain_or_bracketed_text_are_found(self) -> None:
        # A check that found no links would pass every page; these two must still be seen.
        text = "See [the scan cycle](scan-cycle.md) and [the [IEC] table](iec.md#types)."
        self.assertEqual(LINK_RE.findall(text), ["scan-cycle.md", "iec.md#types"])

    def test_an_image_is_not_a_page_link(self) -> None:
        self.assertEqual(LINK_RE.findall("![A pump](../assets/images/pump.png)"), [])


class StepIndentationTests(unittest.TestCase):
    # The site's Markdown (Python-Markdown) nests a numbered step's blocks at 4 spaces, where GitHub
    # nests them under the step's text, at 3 after "1. ". 77 lines in 14 files the site shows broke
    # their steps that way: the list ended, and the later steps were numbered from 1 again or ran
    # into one paragraph (checklist G286, 23 November 2026).

    def test_a_steps_blocks_indented_3_are_found(self) -> None:
        # Catches a check that misses one kind of block: a paragraph after a blank line (line 3), a
        # fence (line 5) and a table row (line 10), each indented 3 under "1. ".
        lines = [
            "1. Open the project.",
            "",
            "   Then build it:",
            "",
            "   ```bash",
            "   trust-runtime build",
            "   ```",
            "2. Read the table.",
            "",
            "   | Name | Value |",
        ]
        self.assertEqual(short_list_blocks(lines), [3, 5, 10])

    def test_a_bullet_left_of_a_two_digit_steps_text_is_found(self) -> None:
        # Catches a check that measures only from the step's text: "10. " puts the text at column 4,
        # so a bullet 3 spaces in nests on neither site. Python-Markdown drew it as an 11th step and
        # GitHub ended the list there (the communication README, 23 November 2026).
        lines = [
            "9. `gpio/README.md`",
            "    - learn IEC bit mapping to GPIO lines.",
            "10. `multi_driver/README.md`",
            "   - learn composed-driver commissioning.",
        ]
        self.assertEqual(short_list_blocks(lines), [4])

    def test_blocks_at_4_a_continued_line_and_a_top_level_fence_are_not_found(self) -> None:
        # Catches a check that fails correct pages: a block at 4 spaces (line 3), a line that only
        # continues the step's sentence (line 6), and a fence at the margin, whose content is no step's.
        lines = [
            "1. Open the project.",
            "",
            "    Then build it.",
            "",
            "2. Run the program, which starts",
            "   at once.",
            "",
            "```text",
            "1. Not a step",
            "   inside a fence",
            "```",
        ]
        self.assertEqual(short_list_blocks(lines), [])

    def test_a_step_under_a_step_of_several_blocks_needs_a_blank_line(self) -> None:
        # Catches the step that runs into the closing fence above it (line 7). A step under a step of
        # one block (line 2) and a step after a blank line (line 9) start their items on both sites.
        lines = [
            "1. Open the project.",
            "2. Build it:",
            "",
            "    ```bash",
            "    trust-runtime build",
            "    ```",
            "3. Run it.",
            "",
            "4. Stop it.",
        ]
        self.assertEqual(steps_joined_to_a_block(lines), [7])


if __name__ == "__main__":
    unittest.main()
