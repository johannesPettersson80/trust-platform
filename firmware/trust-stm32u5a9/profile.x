ASSERT(ADDR(.vector_table) == 0x08000000 && SIZEOF(.vector_table) <= 8K, "vectors overlap the firmware page");
ASSERT(ADDR(.text) >= 0x08002000, "firmware overlaps the vectors page");
ASSERT(__sheap <= ORIGIN(HEAP_B), "static data reaches heap region B");
ASSERT(_stack_end == ORIGIN(HEAP_B) + LENGTH(HEAP_B), "heap region B does not end at the stack window");
ASSERT(_stack_start - _stack_end == 32K, "MSP reservation changed");
__firmware_load_end = MAX(__veneer_limit, MAX(__erodata, SIZEOF(.data) == 0 ? __erodata : LOADADDR(.data) + SIZEOF(.data)));
ASSERT(__firmware_load_end <= 0x08200000, "firmware image reaches the application region");
