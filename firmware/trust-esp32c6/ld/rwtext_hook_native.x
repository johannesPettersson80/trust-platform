/* The RAM code of the firmware that runs compiled programs (specification 44 §10.3, firmware feature
 * `native`). The program's own module runs from the program code region (`trust_code.x`); this list
 * holds the scan path of the kernel and the resource loop around it: the compiled program's scan, its
 * tasks' readiness and order, the compiled cycle and the loop's observer, the clock a scan reads and
 * the GPIO lines it drives. The interpreter's list (`ld/rwtext_hook.x`) is not used: its code is linked
 * only through vtables and never runs, and the RAM it took (about 100 KB) goes to the heap, which
 * holds an install's upload in one block (specification 34 PORT-05). Patterns match v0-mangled
 * paths, whose length prefixes keep module and function names exact. */

/* The kernel's program: the scan, the calls of the module's entry points, the bindings' sample and
 * publication, and the areas' bytes (`native::program`). */
*(.text.*18trust_runtime_core6native7program*)
*(.text.*18trust_runtime_core5cycle*)
*(.text.*18trust_runtime_core4task*)

/* The compiled cycle, the loop's scan step and observer, and the snapshot a scan publishes. */
*(.text.*22trust_runtime_services6runner8compiled*)
*(.text.*22trust_runtime_services6runner8observer*)
*(.text.*22trust_runtime_services6runner11supervision*)
*(.text.*17on_compiled_phase*)
*(.text.*14publish_images*)
*(.text.*13record_images*)

/* The clock a scan reads, with the base a reading extends and the allocation count, each read
 * without a critical section (as in the interpreter's list). */
*(.text.*20trust_platform_esp328hardware5clock*)
*(.text.*20trust_platform_esp3211cycle_clock*)
*(.text.*21trust_platform_memory5count*)
*(.text.*18trust_runtime_core5ports*16MonotonicInstant*)
*(.text.*4core4time*8Duration3new*)

/* The GPIO lines a scan reads and drives. */
*(.text.*20trust_platform_esp3210process_io*)
