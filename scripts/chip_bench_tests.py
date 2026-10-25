#!/usr/bin/env python3
"""Tests of the chip bench's install plan and checks (runtime portability checklist D2): the values
are the ESP32-C6's chip file, `chips/esp32c6.toml`, worked out by hand."""

import unittest

import chip_bench


class InstallPlan(unittest.TestCase):
    def test_the_c6_install_erases_the_selection_then_writes_three_parts(self):
        """The C6's boot loader is at 0x0, its partition table at 0x8000, its first firmware slot
        at 0x10000, and the boot loader's selection is 8 KiB at 0xB000 (specification 34 Appendix
        D.2). Catches a part written at another region's address, and an install that keeps the
        selection, after which the boot loader may boot the other slot's older firmware."""
        commands = chip_bench.install_commands("esp32c6", "/dev/ttyACM0", chip_bench.Path("/f"))
        self.assertEqual(
            commands[0],
            ["espflash", "erase-region", "--chip", "esp32c6", "--port", "/dev/ttyACM0", "0xb000", "0x2000"],
        )
        written = [(command[-2], command[-1]) for command in commands[1:]]
        self.assertEqual(
            written,
            [("0x0", "/f/bootloader.bin"), ("0x8000", "/f/partitions.bin"), ("0x10000", "/f/firmware.bin")],
        )

    def test_a_reply_without_the_version_or_its_confirmation_is_reported(self):
        """The chip must run the workspace version, confirmed (specification 34 TARGET-04).
        Catches a bench that passes on a chip still running the firmware before the change, or one
        that the boot loader would roll back."""
        good = {"ok": True, "result": {"firmware": {"version": "1.2.3", "confirmed": True}}}
        self.assertEqual(chip_bench.check_capabilities(good, "1.2.3"), [])
        old = {"ok": True, "result": {"firmware": {"version": "1.2.2", "confirmed": True}}}
        self.assertEqual(
            chip_bench.check_capabilities(old, "1.2.3"),
            ["firmware version '1.2.2', expected '1.2.3'"],
        )
        trial = {"ok": True, "result": {"firmware": {"version": "1.2.3", "confirmed": False}}}
        self.assertEqual(
            chip_bench.check_capabilities(trial, "1.2.3"),
            ["the firmware did not confirm its boot"],
        )
        refused = {"ok": False, "error": "invalid auth token"}
        self.assertEqual(
            chip_bench.check_capabilities(refused, "1.2.3"),
            ["capabilities refused: invalid auth token"],
        )


if __name__ == "__main__":
    unittest.main()
