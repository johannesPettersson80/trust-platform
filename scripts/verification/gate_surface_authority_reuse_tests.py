"""Focused tests for exact gate-surface code-authority routing."""

from __future__ import annotations

import tomllib
import unittest
from pathlib import Path
from types import SimpleNamespace

from scripts.verification.code_spec_authority_reuse import _candidate_invariant
from scripts.verification.gate_inventory import load_gate_inventory
from scripts.verification.test_catalog_surfaces import scan_gate_scripts

try:
    from scripts.verification.gate_surface_authority_reuse import (
        build_gate_surface_authority_routes,
    )
except ModuleNotFoundError:
    build_gate_surface_authority_routes = None


INVARIANT_ID = "VERIFICATION_GATE_SURFACE_EXECUTION_001"
TEST_ID = "TEST_AUTO_GATE"
DISCOVERY_ID = "DISC_1234567890ABCDEF1234"
CODE_ENTRY = "CODE_" + "1" * 64
CODE_HELPER = "CODE_" + "2" * 64


def candidate() -> dict[str, object]:
    return {
        "invariant_id": INVARIANT_ID,
        "area": "verification",
        "spec_ref": (
            "SPEC_VERIFICATION_INVENTORY_001"
            "#gate-and-workflow-surface-proof-contract"
        ),
        "test_ids": [TEST_ID],
    }


def catalog() -> dict[str, dict[str, object]]:
    return {
        TEST_ID: {
            "id": TEST_ID,
            "status": "mapped",
            "area": "verification",
            "invariants": [INVARIANT_ID],
            "discovery_id": DISCOVERY_ID,
            "discovery_source_kind": "gate_script",
            "path": "scripts/example_gate.py",
            "name": "example_gate",
            "command": "python3 scripts/example_gate.py --catalog-mode",
        }
    }


def inventory() -> dict[str, dict[str, object]]:
    return {
        "GATE_EXAMPLE": {
            "id": "GATE_EXAMPLE",
            "discovery_id": DISCOVERY_ID,
            "source_kind": "gate_script",
            "path": "scripts/example_gate.py",
            "name": "example_gate",
            "command": "python3 scripts/example_gate.py",
            "command_role": "helper",
            "owner": "release-engineering",
            "environment": "trust_builder",
            "enforcement": "supporting",
            "artifact_kind": "machine_local",
            "artifact_paths": ["target/gate-artifacts/example.json"],
            "artifact_retention": "machine_local",
        }
    }


def live_fact() -> SimpleNamespace:
    return SimpleNamespace(
        stable_id=DISCOVERY_ID,
        source_kind="gate_script",
        path="scripts/example_gate.py",
        name="example_gate",
        command_hint="python3 scripts/example_gate.py",
    )


def mappings() -> dict[str, dict[str, object]]:
    common = {
        "area": "verification",
        "path": "scripts/example_gate.py",
        "owner": "verification",
        "disposition": "spec_gap",
    }
    return {
        CODE_ENTRY: {
            **common,
            "code_id": CODE_ENTRY,
            "item_kind": "python_entrypoint",
            "qualified_symbol": "scripts::example_gate::<entrypoint>",
        },
        CODE_HELPER: {
            **common,
            "code_id": CODE_HELPER,
            "item_kind": "python_function",
            "qualified_symbol": "scripts::example_gate::helper",
        },
    }


class GateSurfaceAuthorityReuseTests(unittest.TestCase):
    def test_exact_gate_surface_route_provider_is_available(self) -> None:
        self.assertIsNotNone(
            build_gate_surface_authority_routes,
            "exact gate-surface authority route provider is not implemented",
        )

    def test_python_gate_route_selects_only_the_entrypoint_code_identity(self) -> None:
        self.assertIsNotNone(build_gate_surface_authority_routes)
        routes = build_gate_surface_authority_routes(
            candidate_invariant=candidate(),
            tests=catalog(),
            inventory_records=inventory(),
            live_gate_facts=[live_fact()],
            mappings=mappings(),
        )

        self.assertEqual([CODE_ENTRY], sorted(routes))
        route = routes[CODE_ENTRY]
        self.assertEqual(INVARIANT_ID, route["invariant_id"])
        self.assertEqual(TEST_ID, route["test_id"])
        self.assertEqual(DISCOVERY_ID, route["discovery_id"])
        self.assertEqual("GATE_EXAMPLE", route["inventory_id"])
        self.assertEqual("verification", mappings()[CODE_ENTRY]["owner"])
        self.assertEqual("release-engineering", route["inventory_owner"])
        self.assertEqual("helper", route["command_role"])
        self.assertEqual("supporting", route["enforcement"])

    def test_workflow_catalog_identity_does_not_create_a_code_route(self) -> None:
        self.assertIsNotNone(build_gate_surface_authority_routes)
        rows = catalog()
        rows[TEST_ID]["discovery_source_kind"] = "github_workflow_job"
        self.assertEqual(
            {},
            build_gate_surface_authority_routes(
                candidate_invariant=candidate(),
                tests=rows,
                inventory_records=inventory(),
                live_gate_facts=[live_fact()],
                mappings=mappings(),
            ),
        )

    def test_gate_route_fails_closed_on_inventory_identity_drift(self) -> None:
        self.assertIsNotNone(build_gate_surface_authority_routes)
        records = inventory()
        records["GATE_EXAMPLE"]["path"] = "scripts/other_gate.py"
        with self.assertRaisesRegex(ValueError, "inventory path differs"):
            build_gate_surface_authority_routes(
                candidate_invariant=candidate(),
                tests=catalog(),
                inventory_records=records,
                live_gate_facts=[live_fact()],
                mappings=mappings(),
            )

    def test_repository_gate_routes_are_exactly_the_29_file_entrypoints(self) -> None:
        root = Path.cwd()
        source_rows = tomllib.loads(
            (root / "verification/spec-sources.toml").read_text()
        )["spec_sources"]
        sources = {row["id"]: row for row in source_rows}
        test_rows = tomllib.loads(
            (root / "verification/test-catalog.toml").read_text()
        )["tests"]
        tests = {row["id"]: row for row in test_rows}
        invariant_path = (
            root
            / "verification/invariants/verification"
            / f"{INVARIANT_ID}.toml"
        )
        invariant = tomllib.loads(invariant_path.read_text())
        candidate_row = _candidate_invariant(
            invariant_id=INVARIANT_ID,
            invariant=invariant,
            spec_sources=sources,
            tests=tests,
        )
        self.assertIsNotNone(candidate_row)
        mapping_rows = tomllib.loads(
            (root / "verification/code-spec-map.toml").read_text()
        )["mappings"]
        map_by_id = {row["code_id"]: row for row in mapping_rows}
        routes = build_gate_surface_authority_routes(
            candidate_invariant=candidate_row,
            tests=tests,
            inventory_records=load_gate_inventory(root),
            live_gate_facts=scan_gate_scripts(root).facts,
            mappings=map_by_id,
        )

        self.assertEqual(29, len(routes))
        selected = [map_by_id[code_id] for code_id in routes]
        self.assertEqual(
            {"python_entrypoint", "shell_entrypoint"},
            {row["item_kind"] for row in selected},
        )
        self.assertNotIn(
            "python_function", {row["item_kind"] for row in selected}
        )


if __name__ == "__main__":
    unittest.main()
