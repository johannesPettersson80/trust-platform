//! The ESP32-C6's `memset` in RAM (checklist P1-7; specification 34 Appendix D.2).
//!
//! The firmware is built for size, so the compiler fills a buffer or an array of more than a few
//! bytes, such as the compact loop's operand bits or a USB receive buffer, by calling `memset`. The
//! ROM's, at 0x4000_04A8 (`esp-rom-sys`), was called about three times in each cycle of the bench
//! program `t2q`, for 24 to 128 bytes (firmware p2r, 22 October 2026). The link points `memset`
//! here instead (`--defsym=memset=trust_memset` in `.cargo/config.toml`, after esp-rom-sys's
//! assignment of the ROM's address):
//!
//! - A fill of at most 16 bytes stores byte by byte.
//! - A fill of 17 to 128 bytes stores the bytes before the destination's first whole word one by
//!   one, jumps into a table of word stores of the byte repeated four times at the entry of its
//!   last whole word and runs to the table's end, and stores the bytes after its last whole word
//!   one by one. A table entry is a store of four bytes (`norvc`), so the entry of `n` bytes of
//!   whole words lies `n` bytes before the table's end.
//! - A fill of more than 128 bytes is the ROM's, reached by a tail call with the arguments as they
//!   came.
//!
//! Like any `memset` it returns the destination and keeps no state. Every size from 0 to 160 at
//! every destination offset within a double word, with three fill values, was checked under
//! qemu-riscv32 (`oc-bench/memset-test-p2s`).

core::arch::global_asm!(
    ".section .rwtext.trust_memset, \"ax\", @progbits",
    ".globl trust_memset",
    ".type trust_memset, @function",
    ".p2align 2",
    "trust_memset:",
    "    li   t1, 128",
    "    bltu t1, a2, 9f",
    "    mv   a3, a0",
    "    andi a1, a1, 0xff",
    "    li   t1, 16",
    "    bgeu t1, a2, 5f",
    "    slli t0, a1, 8",
    "    or   a1, a1, t0",
    "    slli t0, a1, 16",
    "    or   a1, a1, t0",
    "1:  andi t0, a3, 3",
    "    beqz t0, 2f",
    "    sb   a1, 0(a3)",
    "    addi a3, a3, 1",
    "    addi a2, a2, -1",
    "    j    1b",
    "2:  andi t2, a2, -4",
    "    la   t0, 4f",
    "    sub  t0, t0, t2",
    "    jr   t0",
    ".option push",
    ".option norvc",
    "    sw   a1, 124(a3)",
    "    sw   a1, 120(a3)",
    "    sw   a1, 116(a3)",
    "    sw   a1, 112(a3)",
    "    sw   a1, 108(a3)",
    "    sw   a1, 104(a3)",
    "    sw   a1, 100(a3)",
    "    sw   a1, 96(a3)",
    "    sw   a1, 92(a3)",
    "    sw   a1, 88(a3)",
    "    sw   a1, 84(a3)",
    "    sw   a1, 80(a3)",
    "    sw   a1, 76(a3)",
    "    sw   a1, 72(a3)",
    "    sw   a1, 68(a3)",
    "    sw   a1, 64(a3)",
    "    sw   a1, 60(a3)",
    "    sw   a1, 56(a3)",
    "    sw   a1, 52(a3)",
    "    sw   a1, 48(a3)",
    "    sw   a1, 44(a3)",
    "    sw   a1, 40(a3)",
    "    sw   a1, 36(a3)",
    "    sw   a1, 32(a3)",
    "    sw   a1, 28(a3)",
    "    sw   a1, 24(a3)",
    "    sw   a1, 20(a3)",
    "    sw   a1, 16(a3)",
    "    sw   a1, 12(a3)",
    "    sw   a1, 8(a3)",
    "    sw   a1, 4(a3)",
    "    sw   a1, 0(a3)",
    ".option pop",
    "4:  add  a3, a3, t2",
    "    andi a2, a2, 3",
    "5:  beqz a2, 8f",
    "    sb   a1, 0(a3)",
    "    addi a3, a3, 1",
    "    addi a2, a2, -1",
    "    j    5b",
    "8:  ret",
    "9:  li   t0, 0x400004a8",
    "    jr   t0",
    ".size trust_memset, . - trust_memset",
);
