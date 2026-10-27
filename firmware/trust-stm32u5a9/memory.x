/* STM32U5A9 (specification 34, Appendix D.5; design §4.3 and §4.4). Bank 1 of the 4 MiB flash holds the
 * vectors page and the firmware; bank 2 holds the installed application and the two checkpoint slots,
 * and this image never reaches it (profile.x asserts it).
 *
 * FLASH starts at the bank's first byte: cortex-m-rt places `.vector_table` at ORIGIN(FLASH), and the
 * Cortex-M33 fetches its initial SP and reset vector from the first bytes of the bank. The vectors take
 * the first 8 KiB page, and the firmware starts at 0x0800_2000 (`_stext`), the region of
 * chips/stm32u5a9.toml `[layout] firmware`.
 *
 * RAM, by region (every address is reachable by the CPU):
 *   HEAP_A   0x2000_0000  1 MiB     heap region A: no section, the allocator's (memory::initialize)
 *   RAM      0x2010_0000  640 KiB   statics: .data, .bss, .uninit, the link buffers and the runner
 *   HEAP_B   0x201A_0000  754 KiB   heap region B, SRAM5 below the stack: no section in S1
 *   STACK    0x2025_C800  78 KiB    main stack at the top of SRAM5; the initial SP is 0x2027_0000
 */
MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 2048K
  HEAP_A : ORIGIN = 0x20000000, LENGTH = 1024K
  RAM : ORIGIN = 0x20100000, LENGTH = 640K
  HEAP_B : ORIGIN = 0x201A0000, LENGTH = 754K
  STACK : ORIGIN = 0x2025C800, LENGTH = 78K
}

_stext = 0x08002000;
_stack_start = 0x20270000;
_stack_end = 0x2025C800;
