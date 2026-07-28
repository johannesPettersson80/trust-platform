"""Contract tests for the final Phase 18 missing-authority assignment batch."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.verification.remaining_authority_assignments import (
    REVIEWED_IDENTITIES,
    REVIEWED_ROUTES,
    _target_profile,
    generate_assignments,
    write_review_lock,
)
from scripts.verification.frozen_rust_nextest_target import (
    get_target_profile,
)


ROOT = Path(__file__).resolve().parents[2]
FROZEN_REPORT = ROOT / (
    "target/gate-artifacts/verification/"
    "frozen-catalog-gaps-after-runtime-boundary-12.json"
)
ASSERTION_INVENTORY = ROOT / (
    "target/gate-artifacts/verification/"
    "p18-remaining-authority-142/all-assertions.json"
)


class RemainingAuthorityAssignmentsTests(unittest.TestCase):
    maxDiff = None

    def test_reviewed_identities_equal_the_complete_frozen_authority_debt(
        self,
    ) -> None:
        report = json.loads(FROZEN_REPORT.read_text())
        expected = {
            test["discovery_id"]: (batch["path"], test["name"])
            for batch in report["specification_batches"]
            for test in batch["tests"]
        }

        self.assertEqual(142, len(expected))
        self.assertEqual(92, len({path for path, _name in expected.values()}))
        self.assertEqual(expected, dict(REVIEWED_IDENTITIES))

    def test_every_closed_assertion_has_one_reviewed_route(self) -> None:
        inventories = json.loads(ASSERTION_INVENTORY.read_text())[
            "inventories"
        ]

        self.assertEqual(set(REVIEWED_IDENTITIES), set(inventories))
        self.assertEqual(
            {
                discovery_id
                for discovery_id, inventory in inventories.items()
                if not inventory["unresolved_assertion_helpers"]
            },
            set(REVIEWED_ROUTES),
        )
        for discovery_id, inventory in inventories.items():
            if inventory["unresolved_assertion_helpers"]:
                continue
            assertion_ids = {
                assertion["assertion_id"]
                for assertion in inventory["assertions"]
            }
            routes = REVIEWED_ROUTES[discovery_id]
            self.assertEqual(assertion_ids, set(routes))
            self.assertTrue(
                all(route[2] == "oracle_bound" for route in routes.values())
            )

    def test_assignments_use_only_closed_product_authority_groups(self) -> None:
        expected = {
            "BROWSER_ANALYSIS_FOUNDATION_CONFORMANCE_001",
            "DEV_WORKBENCH_CONFORMANCE_001",
            "IEC_PROJECT_EXAMPLE_CONFORMANCE_001",
            "RT_ADS_FOUNDATION_CONFORMANCE_001",
            "RT_AGENT_COMMAND_CONFORMANCE_001",
            "RT_EXECUTION_LANGUAGE_CONFORMANCE_001",
            "RT_HOST_LIFECYCLE_CONFORMANCE_001",
            "RT_HOST_SECURITY_WEB_CONFORMANCE_001",
            "VERIFICATION_CASE_MODEL_CONFORMANCE_001",
        }
        actual = {
            route[0]
            for routes in REVIEWED_ROUTES.values()
            for route in routes.values()
        }

        self.assertEqual(expected, actual)

    def test_every_selected_path_has_one_closed_nextest_target(self) -> None:
        resolved = {
            path: get_target_profile(
                _target_profile(path),
                path=path,
            )
            for path, _name in REVIEWED_IDENTITIES.values()
        }

        self.assertEqual(92, len(resolved))
        self.assertEqual(
            {
                "ads-core-remaining",
                "ads-server-remaining",
                "dev-remaining",
                "hir-remaining",
                "runtime-core-remaining",
                "runtime-remaining",
                "syntax-remaining",
                "verification-cases-remaining",
                "wasm-analysis-remaining",
            },
            {profile.key for profile in resolved.values()},
        )

    def test_review_lock_and_partition_cover_one_atomic_transaction(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory)
            lock = destination / "review-lock.json"

            write_review_lock(
                assertions_path=ASSERTION_INVENTORY,
                output_path=lock,
            )
            assignments = generate_assignments(
                root=ROOT,
                assertions_path=ASSERTION_INVENTORY,
            )

            lock_document = json.loads(lock.read_text())
            self.assertEqual(142, lock_document["reviewed_test_count"])
            self.assertEqual(1094, lock_document["reviewed_assertion_count"])
            self.assertEqual(139, lock_document["mapped_test_count"])
            self.assertEqual(
                3,
                lock_document["owned_nonmapping_test_count"],
            )
            self.assertEqual(139, assignments["summary"]["tests"])
            self.assertEqual(1079, assignments["summary"]["direct_oracle"])
            unresolved = {
                discovery_id
                for discovery_id, inventory in json.loads(
                    ASSERTION_INVENTORY.read_text()
                )["inventories"].items()
                if inventory["unresolved_assertion_helpers"]
            }
            self.assertEqual(
                {
                    "DISC_39AB329C1EF74B96CF4F",
                    "DISC_ABC89CA7DBB5226C25B0",
                    "DISC_F7350A6B5413D070B89F",
                },
                unresolved,
            )
            self.assertEqual(
                {
                    path
                    for discovery_id, (path, _name) in (
                        REVIEWED_IDENTITIES.items()
                    )
                    if discovery_id not in unresolved
                },
                {
                    row["path"]
                    for row in assignments["assignments"]
                },
            )


if __name__ == "__main__":
    unittest.main()
