#!/usr/bin/env bash
# Runs harness.c against the firmware's memory routines under qemu-riscv32 (specification 34 D.2):
# the routines' assembly is taken from src/memcpy.rs, src/memset.rs and src/memcmp.rs as it stands,
# with the jumps to the ROM's routines sent to the harness's plain loops. Needs clang with the RISC-V
# target, ld.lld and qemu-riscv32 (qemu-user); the Raspberry Pi of the chip bench has them. Exit 0
# when every case holds, else the harness's number of the first case that fails.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
python3 - "$here/../../src" "$work/routines.S" <<'PY'
import re, sys
source, out = sys.argv[1], sys.argv[2]
lines = [".text"]
for name in ("memcpy", "memset", "memcmp"):
    text = open(f"{source}/{name}.rs").read()
    block = text[text.index("core::arch::global_asm!("):]
    block = block[: block.index("\n);")]
    for line in re.findall(r'^\s*"(.*)",$', block, re.M):
        if line.startswith((".section", ".type", ".size")):
            continue
        # The ROM's routine, at its address on the chip, is the harness's rom_<name> here.
        line = re.sub(r"li\s+t0, 0x4000[0-9a-f]{4}", f"la   t0, rom_{name}", line)
        lines.append(line)
open(out, "w").write("\n".join(lines) + "\n")
PY
clang --target=riscv32-linux-gnu -march=rv32imac -mabi=ilp32 -nostdlib -static -fuse-ld=lld \
  -O1 -ffreestanding -fno-builtin -o "$work/memory_routines" "$here/harness.c" "$work/routines.S"
qemu-riscv32 "$work/memory_routines"
