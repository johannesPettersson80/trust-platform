/* The main-stack region must hold the composition's 16 KiB window plus a 4 KiB margin: below the
 * window the boot-time stack paint reaches static data, as the S1b-5c firmware showed when the
 * allocator's `HEAP` static landed in the window (specification 34, D.2). The window is the same
 * 16 KiB `firmware/trust-esp32c6-reference/src/memory.rs` pins, and `cargo xtask portability
 * build-firmware esp32c6` checks the same rule from the linked sections. */
ASSERT(_stack_start - _stack_end >= 20K,
       "stack region must hold the 16 KiB stack window plus a 4 KiB margin (20 KiB)");
