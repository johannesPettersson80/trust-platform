"""Coupling fixtures for ignored-test discovery and historical source paths."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from .ignored_test_discovery import IgnoredDiscoveryBatch
from .ignored_test_live import build_live_inventory_state
from .ignored_test_source_contract import validate_discovery_path_contract


class IgnoredTestSourceContractTests(unittest.TestCase):
    def test_unmodeled_discovery_input_is_rejected(self) -> None:
        failures = validate_discovery_path_contract(
            (
                "crates/trust-runtime/tests/runtime.rs",
                "editors/vscode/src/test/suite/runtime.test.ts",
                "future/ignored-test-surface/probe.case",
            )
        )

        self.assertEqual(
            failures,
            [
                "current ignored-test discovery inputs are not recognized by the "
                "historical source predicate: future/ignored-test-surface/probe.case"
            ],
        )

    def test_live_inventory_checks_discovery_paths_before_report_contract_paths(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            future_path = "future/ignored-test-surface/probe.case"
            source = root / future_path
            source.parent.mkdir(parents=True)
            source.write_text("future source\n")
            scanner = SimpleNamespace(
                to_dict=lambda: {"scan_status": "complete"},
                provenance=SimpleNamespace(
                    commit="0" * 40,
                    timestamp="2026-07-10T12:00:00Z",
                    platform="linux-aarch64",
                    input_paths=(future_path,),
                ),
                inferred_facts=(),
                diagnostics=(),
            )
            empty_batch = IgnoredDiscoveryBatch()
            analysis = SimpleNamespace(summary={"errors": 0})
            with (
                patch(
                    "scripts.verification.ignored_test_live.scan_repository",
                    return_value=scanner,
                ),
                patch(
                    "scripts.verification.ignored_test_live.discover_playwright_skips",
                    return_value=empty_batch,
                ),
                patch(
                    "scripts.verification.ignored_test_live.discover_vscode_unsupported_skip_markers",
                    return_value=empty_batch,
                ),
                patch(
                    "scripts.verification.ignored_test_live.discover_excluded_rust_ignore_markers",
                    return_value=empty_batch,
                ),
                patch(
                    "scripts.verification.ignored_test_live.discover_excluded_node_skip_markers",
                    return_value=empty_batch,
                ),
                patch(
                    "scripts.verification.ignored_test_live.build_inventory_payload",
                    return_value=analysis,
                ),
                patch(
                    "scripts.verification.ignored_test_live._report_contract_paths",
                    return_value=set(),
                ),
            ):
                with self.assertRaisesRegex(
                    ValueError,
                    "future/ignored-test-surface/probe.case",
                ):
                    build_live_inventory_state(root)


if __name__ == "__main__":
    unittest.main()
