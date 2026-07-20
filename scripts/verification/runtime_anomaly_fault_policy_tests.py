"""Tests for the Phase 8 test-only fault-interface policy."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.runtime_anomaly_fault_policy import (
    validate_runtime_anomaly_fault_policy,
)


class RuntimeAnomalyFaultPolicyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "crates/trust-runtime/src").mkdir(parents=True)
        (self.root / "Cargo.toml").write_text("[workspace]\n")
        (self.root / "crates/trust-runtime/Cargo.toml").write_text(
            '[package]\nname = "trust-runtime"\nversion = "0.0.0"\n[features]\ndefault = []\n'
        )
        (self.root / "crates/trust-runtime/src/lib.rs").write_text("pub fn run() {}\n")

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_known_good_product_surface_has_no_production_fault_hook(self) -> None:
        self.assertEqual([], validate_runtime_anomaly_fault_policy(self.root))

    def test_fault_injection_cargo_feature_is_rejected(self) -> None:
        (self.root / "crates/trust-runtime/Cargo.toml").write_text(
            '[package]\nname = "trust-runtime"\nversion = "0.0.0"\n'
            '[features]\nfault-injection = []\n'
        )

        failures = validate_runtime_anomaly_fault_policy(self.root)

        self.assertTrue(any("production fault-hook feature" in item for item in failures))

    def test_public_fault_injection_symbol_is_rejected(self) -> None:
        (self.root / "crates/trust-runtime/src/lib.rs").write_text(
            "pub fn inject_fault() {}\n"
        )

        failures = validate_runtime_anomaly_fault_policy(self.root)

        self.assertTrue(any("public production fault-hook symbol" in item for item in failures))

    def test_comments_and_string_literals_cannot_impersonate_hooks(self) -> None:
        (self.root / "crates/trust-runtime/src/lib.rs").write_text(
            '// pub fn inject_fault() {}\nconst NOTE: &str = "pub fn inject_fault()";\n'
        )

        self.assertEqual([], validate_runtime_anomaly_fault_policy(self.root))


if __name__ == "__main__":
    unittest.main()
