/* Scan-time code executes from HP SRAM (spec 34 D.2 and design §4.3: "keep less code in RAM: only
 * the dispatch loop and its hot callees"). Only the scan's hot path moves: the bytecode
 * interpreter and the engine's scan modules and functions. Load-time code (preparation, the
 * constant-pool decoding, the prepared cell layouts, validation, construction, `EngineState::build`,
 * the engine's initializers and its restart and retain paths) and the control path (the runner, its
 * requests, its services and JSON, and the platform glue) execute from flash. Patterns match
 * v0-mangled paths, whose length prefixes keep module and function names exact. The USB-Serial/JTAG
 * interrupt handler and the FIFO servicing it calls are in RAM through esp-hal's `#[ram]` attribute
 * (design §2.2), not through a pattern here. The ROM supplies soft-float and memory routines.
 *
 * Wi-Fi (checklist L3, spec 34 ARCH-10 and D.2) needs about 200 KiB of the C6's RAM, so only the
 * interpreter loop, the call machinery and the per-scan path stay here. The standard functions and function blocks (`stdlib`, about 45 KB), and REAL to text, string
 * semantics, value display, date and time and sorting (about 12 KB) run from flash, where each
 * instruction costs about 50 ns more (`~/projects/.artifacts/l3/wifi-budget.md` §3.2; measured on
 * the C6 after the move). */

/* The dispatcher and its callees: the bytecode interpreter, the value model it evaluates and the
 * process-image path it publishes through. */
*(.text.*18trust_runtime_core2vm4call*)
*(.text.*18trust_runtime_core2vm8dispatch*)
*(.text.*18trust_runtime_core2vm13dispatch_refs*)
*(.text.*18trust_runtime_core2vm15dispatch_sizeof*)
*(.text.*18trust_runtime_core2vm12dispatch_ops*)
*(.text.*18trust_runtime_core2vm11type_policy*)
*(.text.*18trust_runtime_core2vm6module*)
*(.text.*18trust_runtime_core2vm6errors*)
*(.text.*18trust_runtime_core2vm4edge*)
*(.text.*18trust_runtime_core2vm6frames*)
*(.text.*18trust_runtime_core2vm7helpers*)
*(.text.*18trust_runtime_core2vm17reference_attempt*)
*(.text.*18trust_runtime_core2vm5stack*)
/* The value operations a scan evaluates: the value model, references, text and conversion. The
 * physical view of overlapping structures (`value::overlap`, about 18 KB) runs from flash: only a
 * program with a STRUCT OVERLAP uses it, and the RAM it held is the margin the stack guard and the
 * slices after S4-6 need (18 October 2026; spec 34 D.2). */
*(.text.*18trust_runtime_core5value5types*)
*(.text.*18trust_runtime_core5value9reference*)
*(.text.*18trust_runtime_core5value4size*)
*(.text.*18trust_runtime_core5value14partial_access*)
*(.text.*18trust_runtime_core5value11storage_abi*)
*(.text.*18trust_runtime_core13program_model*)
*(.text.*18trust_runtime_core8io_image*)
*(.text.*18trust_runtime_core10io_address*)

/* The variable store the scan reads and writes: references and direct slots, execution frames,
 * instances, live entries and their identities. C1's prepared cell layouts (`memory::layout`, and
 * the cell and text-store conversion they belong to) are load-time and stay in flash. */
*(.text.*18trust_runtime_core6memory10references*)
*(.text.*18trust_runtime_core6memory16execution_frames*)
*(.text.*18trust_runtime_core6memory9instances*)
*(.text.*18trust_runtime_core6memory4live*)
*(.text.*18trust_runtime_core6memory7storage*)
*(.text.*18trust_runtime_core6memory10InstanceId*)
*(.text.*18trust_runtime_core6memory*14MemoryLocation*)
*(.text.*18trust_runtime_core6memory*7FrameId*)
*(.text.*18trust_runtime_core6memory*6IoArea*)
*(.text.*18trust_runtime_core6memory*12InstanceData*)
*(.text.*18trust_runtime_core6memory*19ExecutionActivation*)

*(.text.*18trust_runtime_core5cycle*)
*(.text.*18trust_runtime_core4task*)
*(.text.*18trust_runtime_core7numeric*)
*(.text.*18trust_runtime_core5error*)

/* The engine's scan modules: the resource cycle and its hooks, the process image, the variable
 * access policy and the assignment and path-write paths a scan runs. `initialization` (the
 * initializer walk), `restart`, `retain_apply`, `retained_graph` and the `std`-gated `owned` and
 * `reconfiguration` are load-time or control-path and stay in flash. */
*(.text.*18trust_runtime_core2vm6engine5cycle*)
*(.text.*18trust_runtime_core2vm6engine10cycle_port*)
*(.text.*18trust_runtime_core2vm6engine2io*)
*(.text.*18trust_runtime_core2vm6engine3api*)
*(.text.*18trust_runtime_core2vm6engine6access*)
*(.text.*18trust_runtime_core2vm6engine10allocation*)
*(.text.*18trust_runtime_core2vm6engine10assignment*)
*(.text.*18trust_runtime_core2vm6engine7context*)
*(.text.*18trust_runtime_core2vm6engine12destinations*)
*(.text.*18trust_runtime_core2vm6engine5entry*)
*(.text.*18trust_runtime_core2vm6engine10identities*)
*(.text.*18trust_runtime_core2vm6engine4keys*)
*(.text.*18trust_runtime_core2vm6engine17literal_admission*)
*(.text.*18trust_runtime_core2vm6engine7ordered*)
*(.text.*18trust_runtime_core2vm6engine18output_transaction*)
*(.text.*18trust_runtime_core2vm6engine10path_write*)
*(.text.*18trust_runtime_core2vm6engine6policy*)
*(.text.*18trust_runtime_core2vm6engine8readonly*)
*(.text.*18trust_runtime_core2vm6engine15reference_types*)
*(.text.*18trust_runtime_core2vm6engine7scratch*)
*(.text.*18trust_runtime_core2vm6engine8services*)

/* The engine's scan entry points, named so that `build` and the rest of the construction path stay
 * in flash. */
*(.text.*18trust_runtime_core2vm6engine*13execute_cycle*)
*(.text.*18trust_runtime_core2vm6engine*24execute_cycle_with_hooks*)
*(.text.*18trust_runtime_core2vm6engine*11execute_pou*)
*(.text.*18trust_runtime_core2vm6engine*20execute_prepared_pou*)
*(.text.*18trust_runtime_core2vm6engine*6borrow*)
*(.text.*18trust_runtime_core2vm6engine*21engineering_operation*)

/* The allocator and the critical section the link and the wait share. The transcendental math of
 * `libm` (SIN, EXP, SQRT and their kin, about 18 KB) runs from flash for the same margin; the
 * arithmetic of REAL and LREAL is the ROM's soft float. */
*(.text.*14embedded_alloc* .text.*21linked_list_allocator*)
*(.text.*16critical_section* .text.*8esp_sync*)
