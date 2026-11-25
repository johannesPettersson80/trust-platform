"""Specification 22, "CI reliability and release-gate artifact contract": the portable lane runs Clippy with
warnings denied for each chip's target, on the no_std core and on each chip's board adapter (the C6's with the
shared services), since the Linux Clippy job never compiles the code that only a chip's target compiles (G301).
"""

import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]


class PortableLaneLintsTest(unittest.TestCase):
    def setUp(self):
        workflow = yaml.safe_load((ROOT / ".github/workflows/ci.yml").read_text())
        self.job = workflow["jobs"]["portable-core"]
        self.commands = [step.get("run", "") for step in self.job["steps"]]

    def test_each_chip_target_is_linted_with_warnings_denied(self):
        # Worked from the lane's matrix: two chip targets, each linted on the no_std core and on its board
        # adapter, the C6's with the services. Catches a lane that goes back to `cargo check`, which let the C6
        # adapter's Clippy findings reach main (G301), a lint whose warnings pass, and a target left out.
        targets = {entry["target"]: entry["board_adapter"] for entry in self.job["strategy"]["matrix"]["include"]}
        self.assertEqual(
            targets,
            {
                "riscv32imac-unknown-none-elf": "trust-platform-esp32",
                "thumbv8m.main-none-eabihf": "trust-platform-stm32u5",
            },
        )
        lints = [command for command in self.commands if command.startswith("cargo clippy")]
        self.assertEqual(len(lints), 3, lints)
        for command in lints:
            self.assertTrue(command.endswith("-- -D warnings"), command)
        self.assertIn("-p trust-runtime-core --no-default-features --target ${{ matrix.target }}", lints[0])
        self.assertIn(
            "-p ${{ matrix.board_adapter }} -p trust-runtime-services --target ${{ matrix.target }}", lints[1]
        )
        self.assertIn("-p trust-platform-stm32u5 --target thumbv8m.main-none-eabihf", lints[2])
        checks = [command for command in self.commands if command.startswith("cargo check")]
        self.assertEqual(checks, [], "a step that only checks lets Clippy findings through")


if __name__ == "__main__":
    unittest.main()
