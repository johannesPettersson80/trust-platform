ASSERT(SIZEOF(.vector_table) <= 16K, "vectors overlap independent application sector");
ASSERT(ADDR(.text) >= 0x08010000, "firmware overlaps application/checkpoint sectors");
ASSERT(__sheap <= _stack_end, "static data and 72KiB heap exceed 112KiB budget");
ASSERT(_stack_start - _stack_end == 16K, "MSP reservation changed");
__firmware_load_end = MAX(__veneer_limit, MAX(__erodata, SIZEOF(.data) == 0 ? __erodata : LOADADDR(.data) + SIZEOF(.data)));
ASSERT(__firmware_load_end <= 0x08100000, "firmware load image exceeds the 1 MiB flash");
