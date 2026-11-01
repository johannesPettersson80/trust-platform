"""Every example README follows the standard of specification 25 §3.5 (checklist G21). Each expected finding
follows from the READMEs and the lists each test writes."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_example_readmes as check  # noqa: E402

WALKTHROUGH = """# Temperature control

Tune a heater loop in Control Studio and watch it hold its setpoint.

## What you learn

- How Identify finds the plant.

## What you need

- truST in VS Code; about 15 minutes.

## Step by step

1. Press **Start**. The status bar shows `truST: Simulator running`.

## What happened

The controller ...

## Try it yourself

- Ask for 6 s instead of 12 s.

## Reference

| File | What it holds |
|---|---|
| `src/Main.st` | the program |
"""

INDEX = """# Communication examples

Each example connects the runtime to one protocol.

| Example | What it teaches |
|---|---|
| `modbus_tcp` | a Modbus TCP server |
"""


def root_with(readmes: dict[str, str], pending: list[str] | None = None, indexes: list[str] | None = None):
    holder = tempfile.TemporaryDirectory()
    root = Path(holder.name)
    (root / "scripts").mkdir()
    (root / "scripts" / "example_readmes.json").write_text(
        json.dumps({"pending": pending or [], "indexes": indexes or []})
    )
    for name, text in readmes.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    return holder, root


class ExampleReadmeStandardTests(unittest.TestCase):
    def test_a_walkthrough_with_every_section_in_order_passes(self) -> None:
        # Catches a check that refuses the standard's own shape.
        holder, root = root_with({"examples/pid/README.md": WALKTHROUGH})
        with holder:
            self.assertEqual(check.findings(root), [])

    def test_a_missing_section_is_named(self) -> None:
        # Catches a README that skips the explanation, as control-pid-commissioning's did on 30 October 2026.
        text = WALKTHROUGH.replace("## What happened\n\nThe controller ...\n\n", "")
        holder, root = root_with({"examples/pid/README.md": text})
        with holder:
            self.assertEqual(
                check.findings(root),
                ["examples/pid/README.md: lacks the section(s) '## What happened'"],
            )

    def test_sections_out_of_order_fail(self) -> None:
        # Catches a README that offers changes to try before it explains the result.
        text = WALKTHROUGH.replace("## What happened", "## Swap").replace("## Try it yourself", "## What happened")
        text = text.replace("## Swap", "## Try it yourself")
        holder, root = root_with({"examples/pid/README.md": text})
        with holder:
            self.assertEqual(
                check.findings(root),
                [
                    "examples/pid/README.md: has its sections out of order; the order is "
                    "What you learn, What you need, Step by step, What happened, Try it yourself, Reference"
                ],
            )

    def test_a_section_the_standard_does_not_have_fails(self) -> None:
        # Catches a reference-style README that keeps its own top-level sections, such as "Signals and timing".
        text = WALKTHROUGH.replace("## Reference", "## Signals and timing\n\nText.\n\n## Reference")
        holder, root = root_with({"examples/pid/README.md": text})
        with holder:
            self.assertEqual(
                check.findings(root),
                [
                    "examples/pid/README.md: has the section(s) '## Signals and timing', "
                    "which the standard does not have"
                ],
            )

    def test_step_by_step_needs_numbered_steps(self) -> None:
        # Catches a walkthrough section that describes instead of giving steps.
        text = WALKTHROUGH.replace("1. Press **Start**.", "Press **Start**.")
        holder, root = root_with({"examples/pid/README.md": text})
        with holder:
            self.assertEqual(
                check.findings(root),
                ["examples/pid/README.md: has no numbered step under '## Step by step'"],
            )

    def test_the_title_needs_its_sentences(self) -> None:
        # Catches a README that never says what you do and what it teaches.
        text = WALKTHROUGH.replace("Tune a heater loop in Control Studio and watch it hold its setpoint.\n\n", "")
        holder, root = root_with({"examples/pid/README.md": text})
        with holder:
            self.assertEqual(
                check.findings(root),
                ["examples/pid/README.md: has no sentence between its title and its first section"],
            )

    def test_a_heading_inside_a_code_block_is_not_a_section(self) -> None:
        # Catches a check that reads a Markdown sample in a fenced block as the README's own section.
        text = WALKTHROUGH.replace("The controller ...", "```markdown\n## Not a section\n```")
        holder, root = root_with({"examples/pid/README.md": text})
        with holder:
            self.assertEqual(check.findings(root), [])

    def test_an_index_needs_a_table_of_its_examples(self) -> None:
        # Catches an index that lists nothing a reader can open.
        holder, root = root_with(
            {
                "examples/communication/README.md": INDEX,
                "examples/opcua/README.md": "# OPC UA\n\nThe OPC UA examples.\n\n- `server`\n",
            },
            indexes=["examples/communication/README.md", "examples/opcua/README.md"],
        )
        with holder:
            self.assertEqual(
                check.findings(root),
                ["examples/opcua/README.md: is an index without a table of its examples"],
            )

    def test_a_pending_readme_is_not_checked_until_it_is_converted(self) -> None:
        # Catches a check that fails every unconverted README at once, which would block the conversion lane.
        holder, root = root_with({"examples/old/README.md": "# Old\n\nNotes.\n"}, pending=["examples/old/README.md"])
        with holder:
            self.assertEqual(check.findings(root), [])

    def test_a_pending_readme_that_passes_must_leave_the_list(self) -> None:
        # Catches a pending list that keeps converted READMEs, so a later change could break them unseen.
        holder, root = root_with({"examples/pid/README.md": WALKTHROUGH}, pending=["examples/pid/README.md"])
        with holder:
            self.assertEqual(
                check.findings(root),
                [
                    "examples/pid/README.md: follows the standard now; take it off the pending list in "
                    "scripts/example_readmes.json"
                ],
            )

    def test_a_listed_path_must_be_a_checked_readme(self) -> None:
        # Catches a list entry left behind by a moved or removed example.
        holder, root = root_with({}, pending=["examples/gone/README.md"])
        with holder:
            self.assertEqual(
                check.findings(root),
                ["scripts/example_readmes.json: lists 'examples/gone/README.md', which is no checked example README"],
            )

    def test_a_new_readme_must_follow_the_standard(self) -> None:
        # Catches a new example added in the old reference style: it is on no list, so it is checked.
        holder, root = root_with({"examples/new/README.md": "# New\n\nNotes.\n\n## Signals\n"})
        with holder:
            self.assertEqual(
                check.findings(root),
                [
                    "examples/new/README.md: lacks the section(s) '## What you learn', '## What you need', "
                    "'## Step by step', '## What happened', '## Try it yourself', '## Reference'",
                    "examples/new/README.md: has the section(s) '## Signals', which the standard does not have",
                ],
            )

    def test_the_catalog_the_course_the_archive_and_library_manuals_are_not_examples(self) -> None:
        # Catches a check that holds the lesson template or a vendored library's manual to the example standard.
        holder, root = root_with(
            {
                "examples/README.md": "# Examples\n",
                "examples/control/01-feedback/README.md": "# Lesson\n",
                "examples/archive/old/README.md": "# Archived\n",
                "examples/room_heating/libraries/plant/README.md": "# Plant library\n",
            }
        )
        with holder:
            self.assertEqual(check.checked_readmes(root), [])
            self.assertEqual(check.findings(root), [])

    def test_the_repository_follows_the_standard(self) -> None:
        # Catches a converted README that drifted, a new example in the old style, or a stale list entry.
        self.assertEqual(check.findings(ROOT), [])


if __name__ == "__main__":
    unittest.main()
