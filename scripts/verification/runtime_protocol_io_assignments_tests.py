"""Focused tests for runtime protocol/I/O semantic assignment rules."""

from __future__ import annotations

import unittest

from scripts.verification.runtime_protocol_io_assignments import (
    _behavior_override,
    _default_behavior,
    _is_non_oracle,
    _is_setup,
)


class RuntimeProtocolIoAssignmentTests(unittest.TestCase):
    def test_driver_policy_assertions_split_by_observed_projection(self) -> None:
        default = ("RT_CONNECTOR_REPORT_PROJECTION_001", 2)
        self.assertEqual(
            ("RT_CONNECTOR_REPORT_PROJECTION_001", 1),
            _behavior_override(
                path="crates/trust-runtime/tests/connectors_status.rs",
                name="io_driver_health_mapping_honors_error_policy",
                expression="assert_eq ! (ok . state , ConnectorState :: Ready)",
                default=default,
            ),
        )
        self.assertEqual(
            ("RT_CONNECTOR_REPORT_PROJECTION_001", 3),
            _behavior_override(
                path="crates/trust-runtime/tests/connectors_status.rs",
                name="io_driver_health_mapping_honors_error_policy",
                expression="assert_eq ! (faulted . state , ConnectorState :: Faulted)",
                default=default,
            ),
        )

    def test_fixture_mechanics_do_not_become_product_oracles(self) -> None:
        self.assertTrue(_is_setup('TcpListener :: bind ("127.0.0.1:0")'))
        self.assertTrue(_is_setup('state . lock () . expect ("state lock")'))
        self.assertTrue(_is_non_oracle('unreachable ! ("fixture path")'))
        self.assertFalse(_is_setup("assert_eq ! (value , 42)"))

    def test_path_and_name_rule_selects_exact_reviewed_behavior(self) -> None:
        self.assertEqual(
            ("RT_MODBUS_DRIVER_001", 2),
            _default_behavior(
                "crates/trust-runtime/tests/modbus_driver.rs",
                "modbus_rejects_unknown_function_config",
            ),
        )
        with self.assertRaisesRegex(ValueError, "no reviewed"):
            _default_behavior("crates/unknown.rs", "unknown_test")


if __name__ == "__main__":
    unittest.main()
