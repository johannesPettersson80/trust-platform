//! The ESP32-C6's `memcpy` in RAM (checklist P1-7; specification 34 Appendix D.2).
//!
//! The firmware is built for size, so the compiler copies a value of more than a few bytes, such as
//! a 32-byte `Value` or a 96-byte event, by calling `memcpy`. The ROM's, at 0x4000_04AC
//! (`esp-rom-sys`, `ld/esp32c6/rom/additional.ld`), checks and loops for about 60 to 100 cycles even
//! for a few words, and its copies took 17.6% of the bench program `v1p`'s cycle (firmware p2c, 21
//! October 2026). The link points `memcpy` here instead (`--defsym=memcpy=trust_memcpy` in
//! `.cargo/config.toml`, which the linker reads after esp-rom-sys's assignment of the ROM's address):
//!
//! - A copy of at most 128 bytes whose two addresses lie at the same offset within a word copies
//!   the bytes before the destination's first whole word one by one, then its `n` bytes of whole
//!   words through a table of word loads and stores in rising order, and the bytes after its last
//!   whole word one by one. A table entry is a load and a store of four bytes each (`norvc`): the
//!   copy moves both addresses `128 - n` bytes down and jumps into the table `2 * n` bytes before its
//!   end, so that the table's last `n / 4` entries copy the words from the first to the last.
//! - A copy of at most 16 bytes whose addresses lie at different offsets copies byte by byte.
//! - A copy of 17 to 128 bytes whose addresses lie at different offsets copies the bytes before the
//!   destination's first whole word one by one, then makes each whole word of the destination from
//!   the two source words it straddles: it reads the source word by word from the word that holds
//!   its next byte, and shifts the last two words read together. It copies the bytes after the last
//!   whole word one by one. A whole-word read can take up to three bytes before or after the source
//!   range, from a word that holds bytes of the range, so it reads no memory the copy could not.
//!   Such copies went on to the ROM's before, and in the bench program `t2q`, where most of them
//!   move a program call's result of 59 to 79 bytes, they took about 9.5% of the busy samples
//!   (firmware p2l, 22 October 2026).
//! - A copy of more than 128 bytes is the ROM's, reached by a tail call with the arguments as they
//!   came.
//!
//! Like any `memcpy` it returns the destination and keeps no state. It copies from the first byte to
//! the last, as the ROM's does, so a copy to a lower address that overlaps its source, which C leaves
//! undefined but the Wi-Fi radio's closed libraries make, gives what `memmove` gives (the table
//! copied from the last word to the first until checklist L3 found that the radio then failed every
//! WPA2 handshake, 25 October 2026). `tests/memory_routines/run.sh` checks every size from 0 to 160
//! at every pair of offsets within a double word under qemu-riscv32, apart and overlapping a source
//! above the destination.

core::arch::global_asm!(
    ".section .rwtext.trust_memcpy, \"ax\", @progbits",
    ".globl trust_memcpy",
    ".type trust_memcpy, @function",
    ".p2align 2",
    "trust_memcpy:",
    "    li   t1, 128",
    "    bltu t1, a2, 9f",
    "    mv   a3, a0",
    "    xor  t0, a0, a1",
    "    andi t0, t0, 3",
    "    beqz t0, 1f",
    "    li   t1, 16",
    "    bgeu t1, a2, 5f",
    "10: andi t0, a3, 3",
    "    beqz t0, 11f",
    "    lbu  t1, 0(a1)",
    "    sb   t1, 0(a3)",
    "    addi a1, a1, 1",
    "    addi a3, a3, 1",
    "    addi a2, a2, -1",
    "    j    10b",
    "11: andi t0, a1, 3",
    "    slli t3, t0, 3",
    "    li   t4, 32",
    "    sub  t4, t4, t3",
    "    sub  a1, a1, t0",
    "    lw   t5, 0(a1)",
    "    li   t6, 4",
    "12: bltu a2, t6, 13f",
    "    lw   t1, 4(a1)",
    "    srl  t5, t5, t3",
    "    sll  t2, t1, t4",
    "    or   t5, t5, t2",
    "    sw   t5, 0(a3)",
    "    mv   t5, t1",
    "    addi a1, a1, 4",
    "    addi a3, a3, 4",
    "    addi a2, a2, -4",
    "    j    12b",
    "13: add  a1, a1, t0",
    "    j    5f",
    "1:  andi t0, a3, 3",
    "    beqz t0, 2f",
    "    beqz a2, 8f",
    "    lbu  t1, 0(a1)",
    "    sb   t1, 0(a3)",
    "    addi a1, a1, 1",
    "    addi a3, a3, 1",
    "    addi a2, a2, -1",
    "    j    1b",
    "2:  andi t2, a2, -4",
    "    addi t0, t2, -128",
    "    add  a1, a1, t0",
    "    add  a3, a3, t0",
    "    la   t0, 4f",
    "    slli t1, t2, 1",
    "    sub  t0, t0, t1",
    "    jr   t0",
    ".option push",
    ".option norvc",
    "    lw   t1, 0(a1)",
    "    sw   t1, 0(a3)",
    "    lw   t1, 4(a1)",
    "    sw   t1, 4(a3)",
    "    lw   t1, 8(a1)",
    "    sw   t1, 8(a3)",
    "    lw   t1, 12(a1)",
    "    sw   t1, 12(a3)",
    "    lw   t1, 16(a1)",
    "    sw   t1, 16(a3)",
    "    lw   t1, 20(a1)",
    "    sw   t1, 20(a3)",
    "    lw   t1, 24(a1)",
    "    sw   t1, 24(a3)",
    "    lw   t1, 28(a1)",
    "    sw   t1, 28(a3)",
    "    lw   t1, 32(a1)",
    "    sw   t1, 32(a3)",
    "    lw   t1, 36(a1)",
    "    sw   t1, 36(a3)",
    "    lw   t1, 40(a1)",
    "    sw   t1, 40(a3)",
    "    lw   t1, 44(a1)",
    "    sw   t1, 44(a3)",
    "    lw   t1, 48(a1)",
    "    sw   t1, 48(a3)",
    "    lw   t1, 52(a1)",
    "    sw   t1, 52(a3)",
    "    lw   t1, 56(a1)",
    "    sw   t1, 56(a3)",
    "    lw   t1, 60(a1)",
    "    sw   t1, 60(a3)",
    "    lw   t1, 64(a1)",
    "    sw   t1, 64(a3)",
    "    lw   t1, 68(a1)",
    "    sw   t1, 68(a3)",
    "    lw   t1, 72(a1)",
    "    sw   t1, 72(a3)",
    "    lw   t1, 76(a1)",
    "    sw   t1, 76(a3)",
    "    lw   t1, 80(a1)",
    "    sw   t1, 80(a3)",
    "    lw   t1, 84(a1)",
    "    sw   t1, 84(a3)",
    "    lw   t1, 88(a1)",
    "    sw   t1, 88(a3)",
    "    lw   t1, 92(a1)",
    "    sw   t1, 92(a3)",
    "    lw   t1, 96(a1)",
    "    sw   t1, 96(a3)",
    "    lw   t1, 100(a1)",
    "    sw   t1, 100(a3)",
    "    lw   t1, 104(a1)",
    "    sw   t1, 104(a3)",
    "    lw   t1, 108(a1)",
    "    sw   t1, 108(a3)",
    "    lw   t1, 112(a1)",
    "    sw   t1, 112(a3)",
    "    lw   t1, 116(a1)",
    "    sw   t1, 116(a3)",
    "    lw   t1, 120(a1)",
    "    sw   t1, 120(a3)",
    "    lw   t1, 124(a1)",
    "    sw   t1, 124(a3)",
    ".option pop",
    "4:  addi a1, a1, 128",
    "    addi a3, a3, 128",
    "    andi a2, a2, 3",
    "5:  beqz a2, 8f",
    "    lbu  t1, 0(a1)",
    "    sb   t1, 0(a3)",
    "    addi a1, a1, 1",
    "    addi a3, a3, 1",
    "    addi a2, a2, -1",
    "    j    5b",
    "8:  ret",
    "9:  li   t0, 0x400004ac",
    "    jr   t0",
    ".size trust_memcpy, . - trust_memcpy",
);
