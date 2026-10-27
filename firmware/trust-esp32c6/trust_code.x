/* The program code region (specification 44 §10.3): RAM between the static data and the main stack,
 * where neither of esp-hal's no-execute ranges (the data up to `_noinit_end`, the stack from
 * `_stack_end`) reaches, so that the CPU executes the compiled program the loader copies into it.
 * `trust_platform_esp32::profile::PROGRAM_CODE_BYTES` sizes it; a firmware that runs no compiled
 * program leaves it empty. */
SECTIONS {
  .trust_code (NOLOAD) : ALIGN(64)
  {
    _trust_code_start = ABSOLUTE(.);
    *(.trust_code .trust_code.*)
    _trust_code_end = ABSOLUTE(.);
  } > RWDATA
}
INSERT AFTER .noinit;
