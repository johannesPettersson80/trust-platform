"""Adversarial schema-vocabulary fixtures for ignored-test inventory reports."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from .ignored_test_validation import validate_schema_contract


SCHEMA_PATH = (
    Path(__file__).resolve().parents[2]
    / "verification/schemas/ignored-test-inventory-report.schema.json"
)


class IgnoredTestSchemaDriftTests(unittest.TestCase):
    def test_schema_contract_pins_closed_report_vocabularies(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text())
        mutations = {
            "diagnostic severity": lambda value: value["$defs"]["diagnostic"][
                "properties"
            ]["severity"]["enum"].append("info"),
            "surface name": lambda value: value["$defs"]["surface"]["properties"][
                "surface"
            ]["enum"].append("invented"),
            "surface coverage": lambda value: value["$defs"]["surface"][
                "properties"
            ]["coverage"]["enum"].append("partial"),
            "source-count kind": lambda value: value["$defs"]["source_count"][
                "properties"
            ]["source_kind"]["enum"].append("invented_test"),
            "scope const": lambda value: value["$defs"]["scope"]["properties"][
                "rust_basis"
            ].update(const="invented_basis"),
        }

        for label, mutate in mutations.items():
            with self.subTest(label=label):
                changed = copy.deepcopy(schema)
                mutate(changed)
                self.assertNotEqual(validate_schema_contract(changed), [])


if __name__ == "__main__":
    unittest.main()
