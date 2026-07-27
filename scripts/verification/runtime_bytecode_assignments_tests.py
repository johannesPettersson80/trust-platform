"""Focused tests for runtime bytecode semantic assignment rules."""

from __future__ import annotations

import unittest

from scripts.verification.runtime_bytecode_assignments import (
    _behavior_for_assertion,
    _default_behavior,
    _is_setup,
)


class RuntimeBytecodeAssignmentTests(unittest.TestCase):
    def test_rules_select_codec_encoder_metadata_and_validator_behaviors(
        self,
    ) -> None:
        self.assertEqual(
            ("RT_BYTECODE_CONTAINER_CODEC_001", 1),
            _default_behavior(
                "crates/trust-runtime/tests/bytecode_sections.rs",
                "string_table_padding",
            ),
        )
        self.assertEqual(
            ("RT_BYTECODE_ENCODER_PROJECTION_001", 5),
            _default_behavior(
                (
                    "crates/trust-runtime/tests/bytecode_encoder/"
                    "bytecode_encoder_part_04.rs"
                ),
                "encoder_emits_control_flow_jumps",
            ),
        )
        self.assertEqual(
            ("RT_BYTECODE_METADATA_APPLY_001", 3),
            _default_behavior(
                "crates/trust-runtime/tests/bytecode_metadata.rs",
                "version_gate",
            ),
        )
        self.assertEqual(
            ("VM_SEAM_VALID_001", 4),
            _default_behavior(
                "crates/trust-runtime/tests/bytecode_validation.rs",
                "jump_validation",
            ),
        )
        self.assertEqual(
            ("VM_SEAM_REF_001", 0),
            _default_behavior(
                "crates/trust-runtime/tests/bytecode_validation.rs",
                "var_meta_rejects_local_ref_outside_every_pou_range",
            ),
        )

    def test_stack_boundary_assertions_select_accept_and_reject_partitions(
        self,
    ) -> None:
        path = "crates/trust-runtime/src/bytecode/validate/resource_limits.rs"
        name = "stack_limit_accepts_boundary_and_rejects_next_value"
        self.assertEqual(
            ("RT_BYTECODE_STACK_BOUNDARY_001", 0),
            _behavior_for_assertion(
                path=path,
                name=name,
                expression=(
                    "validate_operand_stack_depth (VM_MAX_OPERAND_STACK) "
                    '.expect ("fixed stack boundary must pass")'
                ),
            ),
        )
        self.assertEqual(
            ("RT_BYTECODE_STACK_BOUNDARY_001", 1),
            _behavior_for_assertion(
                path=path,
                name=name,
                expression=(
                    "assert ! (validate_operand_stack_depth "
                    "(VM_MAX_OPERAND_STACK + 1) . is_err ())"
                ),
            ),
        )

    def test_validation_fixture_mechanics_are_not_product_oracles(self) -> None:
        path = "crates/trust-runtime/tests/bytecode_validation.rs"
        self.assertTrue(
            _is_setup(
                path=path,
                name="jump_validation",
                expression='module . encode () . expect ("encode")',
            )
        )
        self.assertFalse(
            _is_setup(
                path=path,
                name="jump_validation",
                expression="decoded . validate () . unwrap_err ()",
            )
        )

    def test_unknown_bytecode_test_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "no reviewed"):
            _default_behavior(
                "crates/trust-runtime/tests/bytecode_validation.rs",
                "not_reviewed",
            )


if __name__ == "__main__":
    unittest.main()
