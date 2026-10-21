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
 * interpreter loop and the per-scan path stay here (the call machinery runs from flash since
 * 21 October 2026, below). The standard functions (`stdlib`, about 40 KB; the function blocks' native
 * code runs from RAM, below), and REAL to text, string
 * semantics, value display, date and time and sorting (about 12 KB) run from flash, where each
 * instruction costs about 50 ns more (`~/projects/.artifacts/l3/wifi-budget.md` §3.2; measured on
 * the C6 after the move). */

/* The dispatcher and its callees: the bytecode interpreter, the value model it evaluates and the
 * process-image path it publishes through. The call sessions of the general path's user calls
 * (`vm::dispatch::session`, about 8 KB) run from flash with the rest of the call machinery since
 * 21 October 2026 (checklist P1-7): the two patterns take every `vm::dispatch` item but those whose
 * next name has seven characters and begins with `s`, which is `session` alone. */
*(.text.*18trust_runtime_core2vm8dispatch[!7]*)
*(.text.*18trust_runtime_core2vm8dispatch7[!s]*)
*(.text.*18trust_runtime_core2vm13dispatch_refs*)
*(.text.*18trust_runtime_core2vm12dispatch_ops*)
*(.text.*18trust_runtime_core2vm11type_policy*)
*(.text.*18trust_runtime_core2vm6module*)
*(.text.*18trust_runtime_core2vm6errors*)
*(.text.*18trust_runtime_core2vm5stack*)
/* The value operations a scan evaluates: the value model, references, text and conversion. The
 * physical view of overlapping structures (`value::overlap`, about 18 KB) runs from flash: only a
 * program with a STRUCT OVERLAP uses it, and the RAM it held is the margin the stack guard and the
 * slices after S4-6 need (18 October 2026; spec 34 D.2). */
*(.text.*18trust_runtime_core5value5types*)
*(.text.*18trust_runtime_core5value11storage_abi*)
*(.text.*18trust_runtime_core13program_model*)
*(.text.*18trust_runtime_core8io_image*)
*(.text.*18trust_runtime_core10io_address*13flat_byte_end*)

/* The identities of the variable store and its execution frames, whose activation and release
 * every program call runs. The value store of C1's `Value` half (references, instances, live
 * entries and direct slots) runs from flash since 21 October 2026: a scan reads and writes the flat
 * store below (checklist P1-7). C1's prepared cell layouts (`memory::layout`) are load-time and stay
 * in flash. */
*(.text.*18trust_runtime_core6memory16execution_frames*)
*(.text.*18trust_runtime_core6memory10InstanceId*)
*(.text.*18trust_runtime_core6memory*14MemoryLocation*)
*(.text.*18trust_runtime_core6memory*7FrameId*)
*(.text.*18trust_runtime_core6memory*6IoArea*)
*(.text.*18trust_runtime_core6memory*12InstanceData*)
*(.text.*18trust_runtime_core6memory*19ExecutionActivation*)

/* The flat store every scan reads and writes since C1 (checklist P1-7; measured 21 October 2026 by
 * sampling the C6's program counter, `~/projects/.artifacts/runtime-portability-c6/oc-bench/profile-1`
 * and `-2`): finding a variable's bytes and its member, reading and writing its leaf, and the names
 * and regions they look up held about 69% of the scan's samples, all from flash. They take the room
 * of the call machinery (`vm::call`, about 51 KB, 1.4% of the samples) and of value reference, size
 * and partial-access code, `SIZEOF` and reference attempts (about 14 KB, no samples), and of the
 * frame records, the edge inputs and the dispatcher's helpers (about 3 KB, under 1% of the samples),
 * which run from flash instead; the RAM code keeps the size the stack window leaves it. */
*(.text.*18trust_runtime_core6memory11cell_access*)
*(.text.*18trust_runtime_core6memory5cells*)
*(.text.*18trust_runtime_core6memory4flat*)
/* The regions' scan functions: the free functions and `CellRegions`. The builders of the store's
 * shape and its initial values (`PersistentExtent`, `StoreShape`, `Leaves`, `DefaultWriter`, about
 * 6 KB, no samples) run at construction, from flash. */
*(.text.*_18trust_runtime_core6memory7regions[0-9]*)
*(.text.*_18trust_runtime_core6memory7regions*11CellRegions*)
*(.text.*18trust_runtime_core6memory4view*)
*(.text.*18trust_runtime_core6memory14instance_table*)
*(.text.*18trust_runtime_core2vm5names*)

*(.text.*18trust_runtime_core5cycle*)
*(.text.*18trust_runtime_core4task*)
*(.text.*18trust_runtime_core7numeric*)
/* The clock a scan reads at each deadline check: the reading, the instants it gives and the held
 * time the device's deadline subtracts (checklist P1-6; `profile-p1e`: 14 samples from flash). */
*(.text.*20trust_platform_esp328hardware5clock*)
*(.text.*18trust_runtime_core5ports*16MonotonicInstant*)
*(.text.*22trust_runtime_services6runner6engine9held_time*)
*(.text.*4core4time*8Duration3new*)
/* A refused option or result drops its error on the scan path. Cloning, formatting and coding an
 * error (about 9 KB, no samples) run only on a fault and run from flash. */
*(.text.*9drop_glue*18trust_runtime_core5error12RuntimeError*)

/* The engine's scan modules: the resource cycle and its hooks, the process image, the variable
 * access policy and the assignment and path-write paths a scan runs. `initialization` (the
 * initializer walk), `restart`, `retain_apply`, `retained_graph` and the `std`-gated `owned` and
 * `reconfiguration` are load-time or control-path and stay in flash. So, since 21 October 2026
 * (checklist P1-7), do the lifecycle marks (`keys`), the admission of a queued write's literal
 * (`literal_admission`) and the types of general-path references (`reference_types`), about
 * 5.2 KB that no cycle of the bench programs runs (the host's instruction counts per cycle,
 * `scan-profile-9`), whose room the standard blocks and the conversions above take. A sample count
 * of the chip does not decide this: the lifecycle sets (`identities`) were never sampled, but a
 * program call runs them, and from flash they cost each scan its cache misses (`profile-p1e`). */
*(.text.*18trust_runtime_core2vm6engine5cycle*)
*(.text.*10cycle_port10CycleHooks*)
*(.text.*10cycle_port10CyclePhase*)
*(.text.*18trust_runtime_core2vm6engine10cycle_port*11EngineState*)
*(.text.*18trust_runtime_core2vm6engine2io*)
*(.text.*18trust_runtime_core2vm6engine6access*)
*(.text.*18trust_runtime_core2vm6engine10allocation*)
*(.text.*18trust_runtime_core2vm6engine16allocation_count*)
*(.text.*18trust_runtime_core2vm6engine10assignment*)
*(.text.*18trust_runtime_core2vm6engine7context*)
*(.text.*18trust_runtime_core2vm6engine12destinations*)
*(.text.*18trust_runtime_core2vm6engine5entry*)
*(.text.*18trust_runtime_core2vm6engine10identities*)
*(.text.*18trust_runtime_core2vm6engine7ordered*)
*(.text.*18trust_runtime_core2vm6engine18output_transaction*)
*(.text.*18trust_runtime_core2vm6engine10path_write*)
*(.text.*18trust_runtime_core2vm6engine6policy*)
*(.text.*18trust_runtime_core2vm6engine8readonly*)
*(.text.*18trust_runtime_core2vm6engine7scratch*11FrameLocals*)
*(.text.*18trust_runtime_core2vm6engine7scratch*16ReferenceBuffers*)
*(.text.*18trust_runtime_core2vm6engine7scratch*18BookkeepingScratch*)
*(.text.*18trust_runtime_core2vm6engine7scratch*11EngineState*)
*(.text.*18trust_runtime_core2vm6engine8services*)

/* The engine's scan entry points, named so that `build` and the rest of the construction path stay
 * in flash, and the per-instruction work charge, a provided method of the execution context. The
 * cycle port's engineering writes (`CycleAccess`), the scratch plan, I/O address parsing and the
 * runtime state's teardown (about 9 KB, no samples) run from flash (21 October 2026, `profile-3`:
 * `execute_counted_cycle` took 61 samples from flash). */
*(.text.*18trust_runtime_core2vm6engine*13execute_cycle*)
*(.text.*18trust_runtime_core2vm6engine*21execute_counted_cycle*)
*(.text.*16ExecutionContext21charge_execution_work*)
*(.text.*18trust_runtime_core2vm6engine*24execute_cycle_with_hooks*)
*(.text.*18trust_runtime_core2vm6engine*11execute_pou*)
*(.text.*18trust_runtime_core2vm6engine*20execute_prepared_pou*)
*(.text.*18trust_runtime_core2vm6engine*6borrow*)
*(.text.*18trust_runtime_core2vm6engine*21engineering_operation*)

/* The standard function blocks' native code (`stdlib::fbs`, about 4 KB) and a conversion call's
 * binding (`vm::call::stdlib`, about 2 KB), which every block call and conversion of compact code
 * runs (checklist P1-5; `profile-p1c`: 31 samples from flash). The conversions themselves (about
 * 19 KB) run from flash. */
*(.text.*18trust_runtime_core6stdlib3fbs*)
*(.text.*18trust_runtime_core2vm4call6stdlib*)

/* The frame stack's push and pop, and the prepared declarations a call and a native block look up
 * (`profile-3`: 14 and 37 samples from flash). */
*(.text.*18trust_runtime_core2vm6frames*10FrameStack3pop*)
*(.text.*18trust_runtime_core2vm6frames*10FrameStack4push*)
*(.text.*18trust_runtime_core2vm8prepared*14PreparedModule11declaration*)
*(.text.*18trust_runtime_core2vm8prepared*14PreparedModule12declarations*)
*(.text.*18trust_runtime_core2vm8prepared*14PreparedModule15presence_offset*)
*(.text.*18trust_runtime_core2vm8prepared7indexes6groups*)

/* The allocator and the critical section the link and the wait share. The transcendental math of
 * `libm` (SIN, EXP, SQRT and their kin, about 18 KB) runs from flash for the same margin; the
 * arithmetic of REAL and LREAL is the ROM's soft float. */
*(.text.*14embedded_alloc* .text.*21linked_list_allocator*)
*(.text.*16critical_section* .text.*8esp_sync*)
