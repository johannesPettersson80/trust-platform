/* STM32F407: 1 MiB flash (sectors 4 x 16 KiB, 1 x 64 KiB, 7 x 128 KiB), 128 KiB SRAM at
 * 0x20000000 and 64 KiB core-coupled memory (CCM, CPU only, no DMA) at 0x10000000.
 *
 * The flash holds the vectors, the checkpoint slots, the firmware and the installed application
 * (owner decision O7), so that the bundle no longer sits in a 16 KiB F401 leftover:
 *   sector 0 (16 KiB)       vectors
 *   sector 1 (16 KiB)       unused
 *   sectors 2..3 (16 KiB)   independent future checkpoint slots
 *   sectors 4..9 (704 KiB)  firmware: 64 KiB + 5 x 128 KiB, from 0x08010000
 *   sectors 10..11 (256 KiB) STBC application bundle, installed separately
 * The chip has room for everything after the vectors, so sector 0 holds the vectors alone and
 * no firmware data.
 *
 * RAM: statics and the 72 KiB allocator arena from the bottom of SRAM, the 16 KiB main stack at
 * its top. The CCM is declared and left unassigned until the services runner uses it. */
MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 1024K
  RAM : ORIGIN = 0x20000000, LENGTH = 128K
  CCMRAM : ORIGIN = 0x10000000, LENGTH = 64K
}

_stext = 0x08010000;
_stack_start = 0x20020000;
_stack_end = 0x2001C000;
