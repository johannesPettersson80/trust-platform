"""Focused tests for generated Rust assertion inventories."""

from __future__ import annotations

import copy
import hashlib
import tempfile
import unittest
from pathlib import Path

from scripts.verification.mapping_assertions import (
    invoke_conformance_assertion_inventory,
    invoke_st_assertion_inventory,
    invoke_vscode_assertion_inventory,
    validate_assertion_batch,
    validate_proposal_assertion_inventories,
)


class MappingAssertionInventoryTests(unittest.TestCase):
    def test_exact_inventory_binds_generated_assertion_to_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = "crates/example/tests/contract.rs"
            source = root / path
            source.parent.mkdir(parents=True)
            source.write_text("#[test]\nfn contract() { assert!(true); }\n")
            expression = "assert ! (true)"
            expression_digest = (
                "sha256:" + hashlib.sha256(expression.encode()).hexdigest()
            )
            identity = (
                f"{path}\0contract\0contract\0macro:assert\0"
                f"2\0{expression_digest}"
            ).encode()
            assertion_id = (
                "ASSERT_" + hashlib.sha256(identity).hexdigest()[:24].upper()
            )
            document = {
                "schema_version": 1,
                "inventories": [
                    {
                        "schema_version": 1,
                        "path": path,
                        "test_name": "contract",
                        "source_digest": "sha256:"
                        + hashlib.sha256(source.read_bytes()).hexdigest(),
                        "assertions": [
                            {
                                "assertion_id": assertion_id,
                                "kind": "macro:assert",
                                "function": "contract",
                                "line": 2,
                                "expression_digest": expression_digest,
                                "expression": expression,
                            }
                        ],
                        "unresolved_assertion_helpers": [],
                    }
                ],
            }

            inventories = validate_assertion_batch(
                root,
                document,
                [
                    {
                        "discovery_id": "DISC_00000000000000000001",
                        "path": path,
                        "name": "contract",
                    }
                ],
            )

        inventory = inventories["DISC_00000000000000000001"]
        self.assertEqual(assertion_id, inventory["assertions"][0]["assertion_id"])
        self.assertRegex(inventory["inventory_digest"], r"^sha256:[0-9a-f]{64}$")

    def test_stale_expression_digest_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = "test.rs"
            source = root / path
            source.write_text("#[test]\nfn contract() { assert!(true); }\n")
            document = {
                "schema_version": 1,
                "inventories": [
                    {
                        "schema_version": 1,
                        "path": path,
                        "test_name": "contract",
                        "source_digest": "sha256:"
                        + hashlib.sha256(source.read_bytes()).hexdigest(),
                        "assertions": [
                            {
                                "assertion_id": "ASSERT_" + "A" * 24,
                                "kind": "macro:assert",
                                "function": "contract",
                                "line": 2,
                                "expression_digest": "sha256:" + "0" * 64,
                                "expression": "assert ! (true)",
                            }
                        ],
                        "unresolved_assertion_helpers": [],
                    }
                ],
            }

            with self.assertRaisesRegex(ValueError, "expression digest is stale"):
                validate_assertion_batch(
                    root,
                    document,
                    [
                        {
                            "discovery_id": "DISC_00000000000000000001",
                            "path": path,
                            "name": "contract",
                        }
                    ],
                )

    def test_proposal_assertions_must_equal_fresh_generated_inventory(self) -> None:
        inventory = {
            "source_digest": "sha256:" + "1" * 64,
            "inventory_digest": "sha256:" + "2" * 64,
            "assertions": [
                {
                    "assertion_id": "ASSERT_" + "A" * 24,
                    "kind": "macro:assert",
                    "function": "contract",
                    "line": 2,
                    "expression_digest": "sha256:" + "3" * 64,
                    "expression": "assert ! (true)",
                }
            ],
            "unresolved_assertion_helpers": [],
        }
        proposal = {
            "id": "MAP_CAND_FIXTURE",
            "discovery_id": "DISC_00000000000000000001",
            "test_source_digest": inventory["source_digest"],
            "assertion_inventory_digest": inventory["inventory_digest"],
            "assertions": copy.deepcopy(inventory["assertions"]),
            "unresolved_assertion_helpers": [],
        }

        self.assertEqual(
            [],
            validate_proposal_assertion_inventories(
                proposals=[proposal],
                inventories={proposal["discovery_id"]: inventory},
            ),
        )
        proposal["assertions"][0]["expression"] = "forged"
        self.assertTrue(
            any(
                "assertions do not match fresh inventory" in failure
                for failure in validate_proposal_assertion_inventories(
                    proposals=[proposal],
                    inventories={proposal["discovery_id"]: inventory},
                )
            )
        )

    def test_structured_text_inventory_is_scoped_to_the_exact_test_block(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = "crates/example/tests/project/src/tests.st"
            source = root / path
            source.parent.mkdir(parents=True)
            source.write_text(
                "TEST_PROGRAM First\n"
                "ASSERT_TRUE(\n"
                "  Value = INT#1\n"
                ");\n"
                "END_TEST_PROGRAM\n"
                "TEST_PROGRAM Second\n"
                "ASSERT_FALSE(Value = INT#1);\n"
                "END_TEST_PROGRAM\n"
            )
            inventories = invoke_st_assertion_inventory(
                root,
                [
                    {
                        "discovery_id": "DISC_00000000000000000001",
                        "discovery_source_kind": "structured_text_test",
                        "path": path,
                        "name": "First",
                    }
                ],
            )

        assertions = inventories["DISC_00000000000000000001"]["assertions"]
        self.assertEqual(1, len(assertions))
        self.assertEqual("ASSERT_TRUE", assertions[0]["function"])
        self.assertNotIn("ASSERT_FALSE", assertions[0]["expression"])

    def test_vscode_inventory_uses_typescript_ast_for_exact_literal_test(self) -> None:
        root = Path(__file__).resolve().parents[2]

        inventories = invoke_vscode_assertion_inventory(
            root,
            [
                {
                    "discovery_id": "DISC_00000000000000000002",
                    "discovery_source_kind": "vscode_test",
                    "path": (
                        "editors/vscode/src/test/suite/"
                        "runtime-shared-utils.test.ts"
                    ),
                    "name": "parseControlEndpoint rejects invalid tcp endpoints",
                }
            ],
        )

        assertions = inventories["DISC_00000000000000000002"]["assertions"]
        self.assertEqual(3, len(assertions))
        self.assertTrue(
            all(assertion["function"] == "assert.strictEqual" for assertion in assertions)
        )

    def test_conformance_inventory_binds_the_expected_oracle_content(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = (
                "conformance/cases/timers/cfm_timer_001/manifest.toml"
            )
            manifest = root / manifest_path
            manifest.parent.mkdir(parents=True)
            manifest.write_text(
                'id = "cfm_timer_001"\ncategory = "timers"\ncycles = 1\n'
            )
            expected = (
                root
                / "conformance/expected/timers/cfm_timer_001.json"
            )
            expected.parent.mkdir(parents=True)
            expected.write_text(
                '{"case_id":"cfm_timer_001","trace":[{"q":true}]}\n'
            )

            inventories = invoke_conformance_assertion_inventory(
                root,
                [
                    {
                        "discovery_id": "DISC_00000000000000000003",
                        "discovery_source_kind": "conformance_case",
                        "path": manifest_path,
                        "name": "cfm_timer_001",
                    }
                ],
            )

        assertion = inventories["DISC_00000000000000000003"]["assertions"][0]
        self.assertEqual("golden:conformance_expected", assertion["kind"])
        self.assertIn('"q":true', assertion["expression"])


if __name__ == "__main__":
    unittest.main()
