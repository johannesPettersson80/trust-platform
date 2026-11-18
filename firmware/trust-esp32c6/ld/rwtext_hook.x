/* The RAM code of the firmware (specification 44 §10.3). The program's own module runs from the
 * program code region (`trust_code.x`); this list holds the scan path of the kernel and the resource
 * loop around it: the compiled program's scan, its tasks' readiness and order, the compiled cycle and
 * the loop's observer, the clock a scan reads and the process image it reads and drives. The RAM that
 * the bytecode engine's list took (about 100 KB) goes to the heap, which holds an install's upload in
 * one block (specification 34 PORT-05). Patterns match v0-mangled paths, whose length prefixes keep
 * module and function names exact.
 *
 * A pattern matches a path anywhere in a symbol, a generic argument's too, and a crate's name is
 * spelled out only where the symbol names the crate first. A pattern is therefore a module that the
 * scan path owns and whose types no control-path code takes as generic arguments: the C6's own GPIO
 * type, `Esp32Pins`, is an argument of every method of the loop, its stepped scans and its program
 * among them, so its two scan methods are in RAM by their `#[ram]` attribute instead of a pattern,
 * which put 8.9 KB of the runner's control path in RAM (checklist G217, 18 November 2026).
 * `cargo xtask portability build-firmware esp32c6` refuses an image whose runner control path lies in
 * RAM. */

/* The kernel's program: the scan, the calls of the module's entry points, the bindings' sample and
 * publication, and the areas' bytes (`native::program`). Building a program and its cold and warm
 * restarts (`native::start`) run once per install or restart and stay in flash. */
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

/* The process image a scan reads and drives (`ProcessImage`), over the C6's GPIO lines, whose two
 * scan methods (`Esp32Pins::is_high`, `Esp32Pins::drive`) are in RAM by `#[ram]`. The type is named
 * with its module: the module alone also names its `ChipDriver` trait, and with it every bus
 * driver's methods, which run from flash. */
*(.text.*22trust_runtime_services4chip10process_io*12ProcessImage*)
