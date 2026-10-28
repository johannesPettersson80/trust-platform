//! The ESP32-C6's `memcmp` in RAM (checklist P1-7; specification 34 Appendix D.2).
//!
//! The firmware compares byte slices and texts, such as a metric's name with a recorded one, by
//! calling `memcmp`. The ROM's, at 0x4000_04B4 (`esp-rom-sys`), was called about four times in each
//! cycle of the bench program `t2q`, for 2 to 8 bytes (firmware p2r, 22 October 2026). The link
//! points `memcmp` here instead (`--defsym=memcmp=trust_memcmp` in `.cargo/config.toml`):
//!
//! - A comparison of at most 32 bytes compares byte by byte and returns the difference of the first
//!   bytes that differ, as unsigned bytes, or 0 when none differs.
//! - A comparison of more than 32 bytes is the ROM's, reached by a tail call with the arguments as
//!   they came.
//!
//! Like any `memcmp` it keeps no state. Every size from 0 to 48 at every pair of offsets within a
//! word, equal and with one byte greater or less at every position, both ways, was checked under
//! qemu-riscv32 (`oc-bench/memset-test-p2s`).

core::arch::global_asm!(
    ".section .rwtext.trust_memcmp, \"ax\", @progbits",
    ".globl trust_memcmp",
    ".type trust_memcmp, @function",
    ".p2align 2",
    "trust_memcmp:",
    "    li   t1, 32",
    "    bltu t1, a2, 9f",
    "1:  beqz a2, 8f",
    "    lbu  t0, 0(a0)",
    "    lbu  t1, 0(a1)",
    "    bne  t0, t1, 7f",
    "    addi a0, a0, 1",
    "    addi a1, a1, 1",
    "    addi a2, a2, -1",
    "    j    1b",
    "7:  sub  a0, t0, t1",
    "    ret",
    "8:  li   a0, 0",
    "    ret",
    "9:  li   t0, 0x400004b4",
    "    jr   t0",
    ".size trust_memcmp, . - trust_memcmp",
);
