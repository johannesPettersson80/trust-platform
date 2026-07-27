from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.simulation_workflow_assignments import (
    BEHAVIOR_BY_TEST,
    INVARIANT,
    TEST_PATH,
    generate_assignments,
    select_live_inventories,
)


class SimulationWorkflowAssignmentsTests(unittest.TestCase):
    def _inventory(self) -> dict[str, object]:
        def assertions(index: int, name: str) -> list[dict[str, str]]:
            rows = [
                {
                    "assertion_id": f"ASSERT_{index:02d}",
                    "expression": "assert ! (observed)",
                }
            ]
            if name == "physics_revolute_trace_is_deterministic_for_same_seed":
                rows.extend(
                    [
                        {
                            "assertion_id": "ASSERT_TRACE_LEN",
                            "expression": (
                                "assert_eq ! (trace_a . len () , 1000)"
                            ),
                        },
                        {
                            "assertion_id": "ASSERT_TRACE_HASH",
                            "expression": (
                                'assert_eq ! (trace_hash , "compat-hash")'
                            ),
                        },
                        {
                            "assertion_id": "ASSERT_ARTIFACT",
                            "expression": (
                                "write_simulation_physics_artifact "
                                "(& trace_hash , first , last) . "
                                'expect ("write artifact")'
                            ),
                        },
                    ]
                )
            return rows

        return {
            "schema_version": 2,
            "inventories": {
                f"DISC_{index:02d}": {
                    "path": TEST_PATH,
                    "test_name": name,
                    "unresolved_assertion_helpers": [],
                    "assertions": assertions(index, name),
                }
                for index, name in enumerate(BEHAVIOR_BY_TEST)
            },
        }

    def _root(self, inventory: dict[str, object]) -> tempfile.TemporaryDirectory:
        temp = tempfile.TemporaryDirectory()
        root = Path(temp.name)
        invariant = (
            root
            / "verification"
            / "invariants"
            / "runtime_safety"
            / f"{INVARIANT}.toml"
        )
        invariant.parent.mkdir(parents=True)
        invariant.write_text(
            "id = \"" + INVARIANT + "\"\n"
            + "\n".join(
                "[[behavior]]\npartition = { equals = \"B\" }"
                for _ in range(9)
            )
            + "\n"
        )
        reviews = []
        for discovery_id, row in inventory["inventories"].items():
            reviews.append(
                "[[reviews]]\n"
                f'discovery_id = "{discovery_id}"\n'
                f'path = "{row["path"]}"\n'
                f'name = "{row["test_name"]}"\n'
                'disposition = "reviewed_nonmapping"\n'
                'reason_code = "no_reviewed_spec_or_invariant_binding"\n'
            )
        denominator = root / "verification" / "test-catalog-denominator.toml"
        denominator.write_text("schema_version = 1\n" + "\n".join(reviews))
        return temp

    def test_live_selection_is_exact(self) -> None:
        inventory = self._inventory()
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        selected = select_live_inventories(
            root=Path(temp.name),
            inventory_document=inventory,
        )
        self.assertEqual(
            len(BEHAVIOR_BY_TEST),
            len(selected["inventories"]),
        )

    def test_assignments_cover_every_reviewed_test(self) -> None:
        inventory = self._inventory()
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        assignments = generate_assignments(
            root=Path(temp.name),
            inventory_document=inventory,
        )
        self.assertEqual(
            set(BEHAVIOR_BY_TEST),
            {row["name"] for row in assignments["assignments"]},
        )
        self.assertEqual(
            dict(BEHAVIOR_BY_TEST),
            {
                row["name"]: row["assertion_assignments"][0][
                    "behavior_index"
                ]
                for row in assignments["assignments"]
            },
        )
        self.assertTrue(
            all(
                row["assertion_assignments"][0]["invariant_id"] == INVARIANT
                for row in assignments["assignments"]
            )
        )
        physics = next(
            row
            for row in assignments["assignments"]
            if row["name"]
            == "physics_revolute_trace_is_deterministic_for_same_seed"
        )
        self.assertEqual(
            [
                "ASSERT_TRACE_LEN",
                "ASSERT_TRACE_HASH",
                "ASSERT_ARTIFACT",
            ],
            physics["non_oracle_assertion_ids"],
        )

    def test_unresolved_helpers_are_rejected(self) -> None:
        inventory = self._inventory()
        first = next(iter(inventory["inventories"].values()))
        first["unresolved_assertion_helpers"] = ["helper"]
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        with self.assertRaisesRegex(ValueError, "unresolved assertion helpers"):
            generate_assignments(
                root=Path(temp.name),
                inventory_document=inventory,
            )


if __name__ == "__main__":
    unittest.main()
