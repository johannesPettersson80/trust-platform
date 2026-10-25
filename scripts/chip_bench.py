#!/usr/bin/env python3
"""The chip bench of CI (runtime portability checklist D2): installs the firmware that CI built for
a change on a real chip and checks that it boots and reports itself (specification 34 TARGET-04).

The chip is on the bench machine's USB port named by TRUST_CHIP_BENCH_PORT. Without that variable
no bench is configured: the run says so and passes, unless TRUST_CHIP_BENCH_REQUIRE is `true`. The
control token, when the chip's device settings hold one, comes from TRUST_CHIP_BENCH_TOKEN and is
never printed.

The install writes the parts where the chip file places them (`bootloader`, `partition_table`,
`firmware`) and erases the boot loader's selection (`firmware_select`), as `trust-runtime firmware
install` does, through `espflash`; the device settings and the program store are kept. After the
boot the chip must answer `capabilities` with the workspace version as its confirmed firmware, and
`status`.

Usage: chip_bench.py <chip> --firmware-dir DIR [--out DIR]
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# The chips the bench installs, with the firmware crate's boot loader and partition table.
CHIPS = {
    "esp32c6": {
        "espflash_chip": "esp32c6",
        "flash_size": "8mb",
        "bootloader": ROOT / "firmware/trust-esp32c6/bootloader/bootloader.bin",
        "partitions": ROOT / "firmware/trust-esp32c6/partitions.csv",
    },
}
BOOT_WAIT_S = 20.0
REPLY_WAIT_S = 5.0


def workspace_version() -> str:
    """The `[workspace.package] version` of the root Cargo.toml."""
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    return manifest["workspace"]["package"]["version"]


def layout(chip: str) -> dict[str, tuple[int, int]]:
    """The chip file's regions that an install writes or erases, as (start, bytes)."""
    file = tomllib.loads((ROOT / "chips" / f"{chip}.toml").read_text())
    regions = file["layout"]
    wanted = ("bootloader", "partition_table", "firmware", "firmware_select")
    return {key: (regions[key]["start"], regions[key]["kib"] * 1024) for key in wanted}


def install_commands(chip: str, port: str, folder: Path) -> list[list[str]]:
    """The espflash commands of an install of `folder` on `chip` at `port`: the selection erased,
    then each part written at its region's start."""
    regions = layout(chip)
    espflash = CHIPS[chip]["espflash_chip"]
    select_start, select_bytes = regions["firmware_select"]
    commands = [
        [
            "espflash", "erase-region", "--chip", espflash, "--port", port,
            hex(select_start), hex(select_bytes),
        ]
    ]
    for key, name in (
        ("bootloader", "bootloader.bin"),
        ("partition_table", "partitions.bin"),
        ("firmware", "firmware.bin"),
    ):
        commands.append(
            [
                "espflash", "write-bin", "--chip", espflash, "--port", port,
                hex(regions[key][0]), str(folder / name),
            ]
        )
    return commands


def run(command: list[str], log) -> None:
    log.write("$ " + " ".join(command) + "\n")
    log.flush()
    subprocess.run(command, check=True, stdout=log, stderr=subprocess.STDOUT)


def ask(port: str, kind: str, token: str | None) -> dict:
    """One control request on the chip's USB link; the reply, or an error if none came."""
    import serial  # pyserial

    request: dict = {"id": 1, "type": kind}
    if token:
        request["auth"] = token
    link = serial.Serial()
    link.port, link.baudrate, link.timeout, link.dtr, link.rts = port, 115200, 0.2, True, False
    link.open()
    try:
        link.reset_input_buffer()
        link.write((json.dumps(request) + "\n").encode())
        link.flush()
        end, buffer = time.monotonic() + REPLY_WAIT_S, b""
        while time.monotonic() < end:
            buffer += link.read(65536)
            start = buffer.find(b'{"id":')
            if start >= 0 and b"\n" in buffer[start:]:
                return json.loads(buffer[start:].split(b"\n")[0])
    finally:
        link.close()
    raise RuntimeError(f"no reply to {kind} within {REPLY_WAIT_S} s")


def check_capabilities(reply: dict, version: str) -> list[str]:
    """What is wrong with the capabilities reply of a chip that should run `version`, confirmed."""
    problems = []
    if not reply.get("ok"):
        return [f"capabilities refused: {reply.get('error')}"]
    firmware = reply.get("result", {}).get("firmware") or {}
    if firmware.get("version") != version:
        problems.append(f"firmware version {firmware.get('version')!r}, expected {version!r}")
    if firmware.get("confirmed") is not True:
        problems.append("the firmware did not confirm its boot")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("chip", choices=sorted(CHIPS))
    parser.add_argument("--firmware-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=Path("artifacts/chip-bench"))
    args = parser.parse_args()

    port = os.environ.get("TRUST_CHIP_BENCH_PORT", "").strip()
    required = os.environ.get("TRUST_CHIP_BENCH_REQUIRE", "false").strip() == "true"
    if not port:
        print("no chip bench is configured (TRUST_CHIP_BENCH_PORT is not set)")
        return 1 if required else 0
    token = os.environ.get("TRUST_CHIP_BENCH_TOKEN", "").strip() or None

    chip = CHIPS[args.chip]
    args.out.mkdir(parents=True, exist_ok=True)
    folder = args.out / "folder"
    folder.mkdir(exist_ok=True)
    with open(args.out / "steps.log", "w") as log:
        (folder / "bootloader.bin").write_bytes(chip["bootloader"].read_bytes())
        run(["espflash", "partition-table", "--to-binary", "-o", str(folder / "partitions.bin"), str(chip["partitions"])], log)
        run(
            [
                "espflash", "save-image", "--chip", chip["espflash_chip"], "--flash-size",
                chip["flash_size"], str(args.firmware_dir / "firmware.elf"), str(folder / "firmware.bin"),
            ],
            log,
        )
        for command in install_commands(args.chip, port, folder):
            run(command, log)
    time.sleep(BOOT_WAIT_S)
    capabilities = ask(port, "capabilities", token)
    status = ask(port, "status", token)
    (args.out / "capabilities.json").write_text(json.dumps(capabilities, indent=1))
    (args.out / "status.json").write_text(json.dumps(status, indent=1))
    problems = check_capabilities(capabilities, workspace_version())
    if not status.get("ok"):
        problems.append(f"status refused: {status.get('error')}")
    for problem in problems:
        print(f"chip bench: {problem}")
    if not problems:
        print(f"chip bench: {args.chip} runs firmware {workspace_version()}, confirmed")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
