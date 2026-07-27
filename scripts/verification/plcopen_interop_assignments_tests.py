from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.plcopen_interop_assignments import (
    EXPECTED_PATH_COUNTS,
    TEST_BINDINGS,
    _expected_path_for_name,
    generate_assignments,
    select_live_inventories,
)


class PlcopenInteropAssignmentsTests(unittest.TestCase):
    def _inventory(self) -> dict[str, object]:
        return {
            "schema_version": 2,
            "inventories": {
                f"DISC_{index:02d}": {
                    "path": _expected_path_for_name(name),
                    "test_name": name,
                    "unresolved_assertion_helpers": [],
                    "assertions": [
                        {
                            "assertion_id": f"ASSERT_{index:02d}",
                            "expression": "assert ! (observed)",
                        }
                    ],
                }
                for index, name in enumerate(TEST_BINDINGS)
            },
        }

    def _root(self, inventory: dict[str, object]) -> tempfile.TemporaryDirectory:
        temp = tempfile.TemporaryDirectory()
        root = Path(temp.name)
        counts: dict[str, int] = {}
        for invariant_id, behavior_index in TEST_BINDINGS.values():
            counts[invariant_id] = max(
                counts.get(invariant_id, 0), behavior_index + 1
            )
        for invariant_id, count in counts.items():
            path = (
                root
                / "verification"
                / "invariants"
                / "plcopen_devtools"
                / f"{invariant_id}.toml"
            )
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(
                f'id = "{invariant_id}"\n'
                + "\n".join(
                    '[[behavior]]\npartition = { equals = "B" }'
                    for _ in range(count)
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
        denominator = root / "verification/test-catalog-denominator.toml"
        denominator.write_text("schema_version = 1\n" + "\n".join(reviews))
        return temp

    def test_live_selection_is_exact_across_four_paths(self) -> None:
        inventory = self._inventory()
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        selected = select_live_inventories(
            root=Path(temp.name),
            inventory_document=inventory,
        )
        self.assertEqual(22, len(selected["inventories"]))
        self.assertEqual(
            dict(EXPECTED_PATH_COUNTS),
            {
                path: sum(
                    row["path"] == path
                    for row in selected["inventories"].values()
                )
                for path in EXPECTED_PATH_COUNTS
            },
        )

    def test_assignments_lock_every_reviewed_binding(self) -> None:
        inventory = self._inventory()
        temp = self._root(inventory)
        self.addCleanup(temp.cleanup)
        assignments = generate_assignments(
            root=Path(temp.name),
            inventory_document=inventory,
        )
        self.assertEqual(
            dict(TEST_BINDINGS),
            {
                row["name"]: (
                    row["assertion_assignments"][0]["invariant_id"],
                    row["assertion_assignments"][0]["behavior_index"],
                )
                for row in assignments["assignments"]
            },
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
