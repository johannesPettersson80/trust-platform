/* Scan-time code executes from HP SRAM (spec 34 D.2 and design §4.3: "keep less code in RAM: only
 * the dispatch loop and its hot callees"). Only the scan's hot path moves: the bytecode
 * interpreter and the engine's scan modules and functions. Load-time code (preparation, the
 * constant-pool decoding, the prepared cell layouts, validation, construction, `EngineState::build`,
 * the engine's initializers and its restart and retain paths) and the control path (the runner, its
 * requests, its services and JSON, and the platform glue) execute from flash. Patterns match
 * v0-mangled paths, whose length prefixes keep module and function names exact. The USB-Serial/JTAG
 * interrupt handler and the FIFO servicing it calls are in RAM through esp-hal's `#[ram]` attribute
 * (design §2.2), not through a pattern here. The ROM supplies soft-float and memory routines. */

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
/* The value operations a scan evaluates: the value model, references, text and conversion, and the
 * admitted physical view of overlapping structures. The physical-layout Builder
 * (`value::overlap::layout`, `prepare_layouts` and its walkers) is preparation-only and stays in
 * flash. */
*(.text.*18trust_runtime_core5value5types*)
*(.text.*18trust_runtime_core5value9reference*)
*(.text.*18trust_runtime_core5value10float_text*)
*(.text.*18trust_runtime_core5value16string_semantics*)
*(.text.*18trust_runtime_core5value4size*)
*(.text.*18trust_runtime_core5value7display*)
*(.text.*18trust_runtime_core5value14partial_access*)
*(.text.*18trust_runtime_core5value8datetime*)
*(.text.*18trust_runtime_core5value11storage_abi*)
*(.text.*18trust_runtime_core5value7overlap10checkpoint*)
*(.text.*18trust_runtime_core5value7overlap5index*)
*(.text.*18trust_runtime_core5value7overlap6writer*)
*(.text.*18trust_runtime_core5value7overlap4cost*)
*(.text.*18trust_runtime_core5value7overlap6scalar*)
*(.text.*18trust_runtime_core5value7overlap9read_bits*)
*(.text.*18trust_runtime_core5value7overlap10write_bits*)
*(.text.*18trust_runtime_core5value7overlap13compare_paths*)
*(.text.*18trust_runtime_core5value7overlap9path_work*)
*(.text.*18trust_runtime_core5value7overlap11path_suffix*)
*(.text.*18trust_runtime_core5value7overlap15struct_path_mut*)
*(.text.*18trust_runtime_core5value7overlap16read_struct_path*)
*(.text.*18trust_runtime_core5value7overlap21write_struct_path_raw*)
*(.text.*18trust_runtime_core5value7overlap11index_order*)
*(.text.*18trust_runtime_core5value7overlap10sort_order*)
*(.text.*18trust_runtime_core5value7overlap*14PhysicalLayout*)
*(.text.*18trust_runtime_core5value7overlap*13PhysicalValue*)
*(.text.*18trust_runtime_core5value7overlap*11Enumeration*)
*(.text.*18trust_runtime_core5value7overlap*4Leaf*)
*(.text.*18trust_runtime_core5value7overlap*5Child*)
*(.text.*18trust_runtime_core5value7overlap*5types11StructValue*)
*(.text.*18trust_runtime_core6stdlib*)
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
*(.text.*18trust_runtime_core8datetime*)
*(.text.*18trust_runtime_core5error*)
*(.text.*18trust_runtime_core4sort*)

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

/* The allocator, the critical section the link and the wait share, and the ROM's floating point. */
*(.text.*14embedded_alloc* .text.*21linked_list_allocator*)
*(.text.*16critical_section* .text.*8esp_sync*)
*(.text.*4libm4math*)
