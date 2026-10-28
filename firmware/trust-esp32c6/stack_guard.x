/* The main-stack region must hold the composition's 78 KiB window plus a 4 KiB margin: below the
 * window the boot-time stack paint reaches static data, as the S1b-5c firmware showed when the
 * allocator's `HEAP` static landed in the window (specification 34, D.2). The window is the same
 * 78 KiB `trust_platform_esp32::profile::MAIN_STACK` gives `firmware/trust-esp32c6/src/memory.rs`,
 * and `cargo xtask portability build-firmware esp32c6` checks the same rule from the linked
 * sections. */
ASSERT(_stack_start - _stack_end >= 82K,
       "stack region must hold the 78 KiB stack window plus a 4 KiB margin (82 KiB)");
