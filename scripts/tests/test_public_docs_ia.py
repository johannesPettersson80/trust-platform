import unittest

from scripts.check_public_docs_ia import LINK_RE


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


if __name__ == "__main__":
    unittest.main()
