"""Tests for the mechanical specification and public-prose scanner."""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from .spec_source_markdown import scan_public_prose
from .spec_source_models import BLOCK_KINDS, stable_document_id
from .spec_source_scanner import discover_spec_documents
from .spec_source_scope import OBVIOUS_SPEC_TOPICS


class SpecSourceScannerTests(unittest.TestCase):
    def test_public_block_kind_vocabulary_is_closed(self) -> None:
        self.assertEqual(
            BLOCK_KINDS,
            (
                "heading",
                "paragraph",
                "list_item",
                "table_row",
                "blockquote",
                "directive",
            ),
        )

    def test_obvious_spec_topics_are_closed_ordered_and_reviewed(self) -> None:
        expected_ids = (
            "P1A004_BYTECODE_FORMAT",
            "P1A004_BYTECODE_VALIDATOR",
            "P1A004_VM_VALUE_SEMANTICS",
            "P1A004_SCAN_CYCLE_LIFECYCLE",
            "P1A004_STOP_SAFE_STATE",
            "P1A004_RETAIN_RESTART",
            "P1A004_PROTOCOL_STATUS_DISCOVERY",
            "P1A004_HMI_API_UI",
            "P1A004_SOURCE_TRANSFORMATIONS",
            "P1A004_LSP_SYNC_POSITIONS_CANCELLATION",
            "P1A004_DEBUG_DAP_FORCE_WRITE_RELEASE_LIFECYCLE",
            "P1A004_CONTROL_RBAC_SECURITY",
            "P1A004_PLCOPEN_IMPORT_EXPORT",
            "P1A004_TEST_HARNESS_SIMULATION_SEMANTICS",
            "P1A004_RUNTIME_PROJECT_HMI_CONFIG_SCHEMAS",
            "P1A004_CLI_CONTROL_SOCKET_SURFACES",
            "P1A004_GPIO",
            "P1A004_RUNTIME_PERFORMANCE_BUDGETS",
            "P1A004_SUPPLY_CHAIN",
            "P1A004_PLATFORM_PACKAGE_BEHAVIOR",
            "P1A004_RELEASE_PROOF",
        )
        self.assertEqual(tuple(item.topic_id for item in OBVIOUS_SPEC_TOPICS), expected_ids)
        self.assertEqual(len({item.topic_id for item in OBVIOUS_SPEC_TOPICS}), 21)
        self.assertTrue(all(item.board_topic and item.reviewed_posture for item in OBVIOUS_SPEC_TOPICS))
        self.assertEqual(
            tuple(item.areas for item in OBVIOUS_SPEC_TOPICS),
            (
                ("bytecode_vm",), ("bytecode_vm",), ("bytecode_vm",),
                ("runtime_safety",), ("runtime_safety",), ("runtime_safety",),
                ("protocols",), ("control_security", "hmi_ui"), ("bytecode_vm",),
                ("editor_safety",),
                ("control_security", "editor_safety", "runtime_safety"),
                ("control_security",), ("plcopen_devtools",), ("verification",),
                ("hmi_ui", "runtime_safety"),
                ("control_security", "runtime_safety"),
                ("runtime_safety",), ("runtime_safety",),
                ("supply_chain_platform",),
                ("release", "supply_chain_platform"), ("release",),
            ),
        )
        self.assertEqual(
            set(OBVIOUS_SPEC_TOPICS[0].to_dict()),
            {
                "topic_id",
                "board_topic",
                "reviewed_posture",
                "eligible_source_ids",
                "nonoracle_source_ids",
                "open_spec_gap_ids",
                "public_claim_context_ids",
                "areas",
            },
        )

    def test_retain_restart_topic_uses_the_closed_gap_source_posture(self) -> None:
        topic = next(
            item for item in OBVIOUS_SPEC_TOPICS if item.topic_id == "P1A004_RETAIN_RESTART"
        )

        self.assertEqual(topic.reviewed_posture, "source_present")
        self.assertEqual(topic.open_spec_gap_ids, ())

    def test_discovery_uses_only_tracked_reviewed_text_surfaces(self) -> None:
        with tracked_repository(
            {
                "README.md": "# Product\n\nPublic overview.\n",
                "RUNTIME_REVIEW_2026-07-05.md": "# Internal review\n",
                "docs/public/about.md": "# About\n\nShipped behavior.\n",
                "docs/guides/guide.md": "# Guide\n",
                "docs/specs/runtime.md": "# Runtime\n",
                "conformance/contract.md": "# Contract\n",
                "docs/internal/testing/evidence/noise/report.md": "# Evidence output\n",
                "docs/internal/testing/evidence/plc-verification-program/2026-07-08/review-verdict.md": "# Reviewed source\n",
                "docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-04-phase-03/opcua-client-subscription-spike.md": "# Lifecycle decision\n",
                "docs/public/not-markdown.rst": "Heading\n=======\n",
                "docs/internal/references/vendor/upstream.txt": b"\xff\xfe",
            }
        ) as root:
            (root / "docs/public/untracked.md").write_text("# Not tracked\n")

            scan = discover_spec_documents(root)

        paths = [document.path for document in scan.documents]
        self.assertEqual(
            paths,
            [
                "README.md",
                "conformance/contract.md",
                "docs/guides/guide.md",
                "docs/internal/testing/evidence/plc-verification-program/2026-07-08/review-verdict.md",
                "docs/internal/testing/evidence/runtime-connectivity-conformance/2026-07-04-phase-03/opcua-client-subscription-spike.md",
                "docs/public/about.md",
                "docs/specs/runtime.md",
            ],
        )
        self.assertNotIn("docs/internal/testing/evidence/noise/report.md", scan.input_paths)
        self.assertNotIn("RUNTIME_REVIEW_2026-07-05.md", scan.input_paths)
        self.assertNotIn("docs/public/untracked.md", scan.input_paths)
        self.assertNotIn("docs/internal/references/vendor/upstream.txt", scan.input_paths)
        self.assertEqual(
            {block.path for block in scan.public_blocks},
            {"README.md", "docs/public/about.md"},
        )

    def test_document_identity_is_path_based_and_semantics_are_not_inferred(self) -> None:
        with tracked_repository({"docs/specs/runtime.md": "# Runtime\n\nFirst text.\n"}) as root:
            first = discover_spec_documents(root).documents[0]
            (root / "docs/specs/runtime.md").write_text("\n# Runtime\n\nChanged text.\n")
            second = discover_spec_documents(root).documents[0]

        self.assertEqual(first.document_id, second.document_id)
        self.assertEqual(first.document_id, stable_document_id("docs/specs/runtime.md"))
        self.assertNotEqual(first.content_sha256, second.content_sha256)
        forbidden = {
            "area",
            "authority",
            "owner",
            "oracle_eligible",
            "claim_type",
            "classification",
        }
        self.assertTrue(forbidden.isdisjoint(first.to_dict()))

    def test_markdown_blocks_exclude_fences_comments_and_include_directives(self) -> None:
        text = """# Runtime

Visible [guide](../guides/runtime.md#start).

```rust
hidden [claim](hidden.md)
```

Before <!-- hidden [comment](comment.md) --> after.

Visible paragraph
    continuation stays visible.

    hidden [indented code](indented.md)

<!--
hidden paragraph
-->

--8<-- "docs/guides/shared.md:3"
"""
        blocks, diagnostics = scan_public_prose("docs/public/runtime.md", text)

        self.assertEqual(diagnostics, ())
        self.assertEqual(blocks[0].block_kind, "heading")
        self.assertEqual(
            [block.text for block in blocks if block.block_kind == "paragraph"],
            [
                "Visible [guide](../guides/runtime.md#start).",
                "Before  after.",
                "Visible paragraph\n    continuation stays visible.",
            ],
        )
        references = [reference for block in blocks for reference in block.local_references]
        self.assertEqual(len(references), 1)
        self.assertEqual(references[0].kind, "markdown_link")
        self.assertEqual(references[0].target_path, "docs/guides/runtime.md")
        self.assertEqual(references[0].fragment, "start")

    def test_unclosed_fence_and_comment_are_visible_errors(self) -> None:
        _, fence_diagnostics = scan_public_prose(
            "docs/public/fence.md", "# Fence\n\n```text\nnot visible\n"
        )
        _, comment_diagnostics = scan_public_prose(
            "docs/public/comment.md", "# Comment\n\n<!-- never closed\n"
        )

        self.assertEqual(
            [(item.severity, item.kind) for item in fence_diagnostics],
            [("error", "unclosed_code_fence")],
        )
        self.assertEqual(
            [(item.severity, item.kind) for item in comment_diagnostics],
            [("error", "unclosed_html_comment")],
        )

    def test_public_block_identity_survives_line_and_text_edits(self) -> None:
        before, _ = scan_public_prose(
            "docs/public/runtime.md", "# Runtime\n\nThe old behavior.\n"
        )
        after, _ = scan_public_prose(
            "docs/public/runtime.md", "\n# Runtime\n\n\nThe revised behavior.\n"
        )

        before_prose = next(block for block in before if block.block_kind == "paragraph")
        after_prose = next(block for block in after if block.block_kind == "paragraph")
        self.assertEqual(before_prose.block_id, after_prose.block_id)
        self.assertNotEqual(before_prose.text_sha256, after_prose.text_sha256)
        self.assertEqual(before_prose.heading_path, ("Runtime",))

    def test_visible_text_normalizes_rendered_links_lists_and_inline_markup(self) -> None:
        blocks, _ = scan_public_prose(
            "README.md",
            "# Platforms\n\n- Supported **wire**: [truST Mesh](docs/mesh.md) and `Modbus`.\n",
        )

        prose = next(block for block in blocks if block.block_kind == "list_item")
        self.assertEqual(
            prose.visible_text,
            "Supported wire: truST Mesh and Modbus.",
        )
        self.assertNotEqual(prose.visible_text_sha256, prose.text_sha256)

    def test_structural_blocks_and_document_headings_are_explicit(self) -> None:
        with tracked_repository(
            {
                "docs/public/shapes.md": (
                    "# Shapes\n\nParagraph.\n\n- First item\n- Second item\n\n"
                    "| State | Meaning |\n| --- | --- |\n| green | passing |\n"
                ),
            }
        ) as root:
            scan = discover_spec_documents(root)

        document = scan.documents[0]
        self.assertEqual(
            [block.block_kind for block in scan.public_blocks],
            ["heading", "paragraph", "list_item", "list_item", "table_row", "table_row"],
        )
        self.assertEqual(len(document.headings), 1)
        self.assertEqual(document.headings[0].text, "Shapes")
        self.assertEqual(document.headings[0].level, 1)
        self.assertEqual(document.headings[0].anchor, "shapes")
        self.assertTrue(document.headings[0].heading_id.startswith("SPEC_HEADING_"))

    def test_public_includes_are_recursive_tracked_and_range_aware(self) -> None:
        with tracked_repository(
            {
                "README.md": '# Readme\n\n--8<-- "docs/guides/shared.md:3"\n',
                "docs/public/about.md": '# About\n\n--8<-- "docs/guides/shared.md:3"\n',
                "docs/guides/shared.md": (
                    "# Shared\n\nVisible shared prose.\n\n"
                    '--8<-- "docs/specs/nested.md"\n'
                ),
                "docs/specs/nested.md": "# Nested\n\nNormative nested prose.\n",
            }
        ) as root:
            scan = discover_spec_documents(root)

        by_text = {block.text: block for block in scan.public_blocks}
        shared = by_text["Visible shared prose."]
        nested = by_text["Normative nested prose."]
        self.assertEqual(
            shared.public_entry_paths,
            ("README.md", "docs/public/about.md"),
        )
        self.assertEqual(nested.public_entry_paths, shared.public_entry_paths)
        self.assertNotIn("Shared", {block.text for block in scan.public_blocks})
        self.assertEqual(scan.diagnostics, ())

    def test_invalid_public_includes_fail_closed(self) -> None:
        with tracked_repository(
            {
                "docs/public/about.md": (
                    '# About\n\n--8<-- "../escape.md"\n\n'
                    '--8<-- "docs/guides/missing.md"\n\n'
                    '--8<-- "docs/guides/binary.md"\n'
                ),
                "docs/guides/binary.md": b"\xff\xfe",
            }
        ) as root:
            scan = discover_spec_documents(root)

        self.assertEqual(
            {item.kind for item in scan.diagnostics},
            {"escaping_include", "untracked_include", "non_utf8_document"},
        )
        self.assertTrue(all(item.severity == "error" for item in scan.diagnostics))

    def test_explicitly_included_text_is_scanned_but_unrelated_text_is_not(self) -> None:
        with tracked_repository(
            {
                "docs/public/about.md": '# About\n\n--8<-- "assets/public-notes.txt"\n',
                "assets/public-notes.txt": "Rendered text claim.\n",
                "docs/internal/references/vendor/upstream.txt": b"\xff\xfe",
            }
        ) as root:
            scan = discover_spec_documents(root)

        self.assertIn("assets/public-notes.txt", scan.input_paths)
        self.assertNotIn("docs/internal/references/vendor/upstream.txt", scan.input_paths)
        self.assertIn("Rendered text claim.", {block.text for block in scan.public_blocks})
        self.assertEqual(scan.diagnostics, ())

    def test_local_reference_observations_are_structured_and_code_is_ignored(self) -> None:
        with tracked_repository(
            {
                "docs/public/index.md": (
                    "# Index\n\n"
                    "[Guide](../guides/start.md)\n\n"
                    "![Image](../assets/logo.png)\n\n"
                    "[ref]: ../specs/runtime.md#rule\n\n"
                    "`[not a link](hidden.md)`\n"
                ),
                "docs/guides/start.md": "# Start\n",
                "docs/specs/runtime.md": "# Runtime\n",
                "docs/assets/logo.png": b"png",
            }
        ) as root:
            document = next(
                item
                for item in discover_spec_documents(root).documents
                if item.path == "docs/public/index.md"
            )

        references = document.local_references
        self.assertEqual(
            [item.kind for item in references],
            ["markdown_link", "markdown_image", "reference_definition"],
        )
        self.assertTrue(all(item.exists for item in references))
        self.assertTrue(all(item.tracked for item in references))
        self.assertNotIn("hidden.md", {item.raw_target for item in references})

    def test_fragment_existence_tracks_live_target_headings(self) -> None:
        with tracked_repository(
            {
                "docs/public/index.md": (
                    "# Index\n\n[Good](../guides/start.md#setup) "
                    "[Stale](../guides/start.md#removed)\n"
                ),
                "docs/guides/start.md": "# Start\n\n## Setup\n",
            }
        ) as root:
            document = next(
                item
                for item in discover_spec_documents(root).documents
                if item.path == "docs/public/index.md"
            )

        fragments = {item.fragment: item.fragment_exists for item in document.local_references}
        self.assertEqual(fragments, {"setup": True, "removed": False})

    def test_numbered_heading_anchor_preserves_the_heading_prefix(self) -> None:
        with tracked_repository(
            {
                "docs/public/index.md": (
                    "# Index\n\n[Summary](#0-executive-summary)\n\n"
                    "## 0. Executive summary\n"
                ),
            }
        ) as root:
            document = discover_spec_documents(root).documents[0]

        self.assertEqual(document.headings[1].visible_text, "0. Executive summary")
        self.assertEqual(document.headings[1].anchor, "0-executive-summary")
        self.assertTrue(document.local_references[0].fragment_exists)

    def test_non_utf8_tracked_document_is_an_error_not_an_exception(self) -> None:
        with tracked_repository({"docs/specs/binary.md": b"\xff\xfe"}) as root:
            scan = discover_spec_documents(root)

        self.assertEqual(scan.documents, ())
        self.assertEqual(len(scan.diagnostics), 1)
        self.assertEqual(scan.diagnostics[0].kind, "non_utf8_document")
        self.assertEqual(scan.input_paths, ("docs/specs/binary.md",))


class tracked_repository:
    def __init__(self, files: dict[str, str | bytes]) -> None:
        self.files = files
        self._temporary: tempfile.TemporaryDirectory[str] | None = None

    def __enter__(self) -> Path:
        self._temporary = tempfile.TemporaryDirectory()
        root = Path(self._temporary.name)
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        for relative, content in self.files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if isinstance(content, bytes):
                path.write_bytes(content)
            else:
                path.write_text(content)
        subprocess.run(["git", "-C", str(root), "add", "--all"], check=True)
        return root

    def __exit__(self, *_args: object) -> None:
        assert self._temporary is not None
        self._temporary.cleanup()


if __name__ == "__main__":
    unittest.main()
