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
 * interpreter loop, the per-scan path and the per-call path of a standard call stay here (the rest of
 * the call machinery runs from flash since 21 October 2026, below). The standard functions (`stdlib`, about 40 KB; the function blocks' native
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
/* The kinded loop's leaf widths and its operands' bits and cells (`vm::compact::kinded`); the pass
 * that writes the kinded code runs at preparation, from flash. */
*(.text.*7compact6kinded5width*)
*(.text.*7compact6kinded7bits_of*)
*(.text.*7compact6kinded12cell_of_bits*)
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
/* The process image's reads, writes, checks and codecs; the codecs' construction from the
 * bindings' types and the collection of the prepared bindings, which preparation runs (about
 * 2.8 KB), run from flash (21 October 2026, for the kinded loop's room). */
*(.text.*18trust_runtime_core8io_image[!8]*)
*(.text.*18trust_runtime_core8io_image8coercion*)
*(.text.*18trust_runtime_core8io_image8prepared*12IoValueCodec6decode*)
*(.text.*18trust_runtime_core8io_image8prepared*12IoValueCodec6encode*)
*(.text.*18trust_runtime_core8io_image8prepared*12IoValueCodec7address*)
*(.text.*9enumerate*8io_image8prepared17PreparedIoBinding*)
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
 * which run from flash instead; the RAM code keeps the size the stack window leaves it. The
 * physical view of overlapping structures in the flat store (`memory::cell_access::cell_overlay`
 * and `memory::cells::convert::overlay`, about 8.5 KB) runs from flash as `value::overlap` does:
 * only a program with a STRUCT OVERLAP uses it, and no cycle of the bench programs runs it
 * (21 October 2026, checklist P1-7). The patterns take `memory::cell_access` and `memory::cells`
 * but the names that begin `12cell_overlay` and `7convert7overlay`. */
*(.text.*18trust_runtime_core6memory11cell_access[!1]*)
*(.text.*18trust_runtime_core6memory11cell_access1[!2]*)
*(.text.*18trust_runtime_core6memory11cell_access12[!c]*)
*(.text.*18trust_runtime_core6memory5cells[!7]*)
*(.text.*18trust_runtime_core6memory5cells7[!c]*)
*(.text.*18trust_runtime_core6memory5cells7convert[!7]*)
*(.text.*18trust_runtime_core6memory5cells7convert7[!o]*)
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
 * time the device's deadline subtracts (checklist P1-6; `profile-p1e`: 14 samples from flash), with
 * the base a reading extends and the allocation count, each read without a critical section. */
*(.text.*20trust_platform_esp328hardware5clock*)
*(.text.*20trust_platform_esp3211cycle_clock*)
*(.text.*21trust_platform_memory5count*)
*(.text.*18trust_runtime_core5ports*16MonotonicInstant*)
*(.text.*22trust_runtime_services6runner6engine9held_time*)
*(.text.*4core4time*8Duration3new*)
/* A refused option or result drops its error on the scan path. Cloning, formatting and coding an
 * error (about 9 KB, no samples) run only on a fault and run from flash. */
*(.text.*9drop_glue*18trust_runtime_core5error12RuntimeError*)

/* The engine's scan modules: the resource cycle and its hooks, the process image, the variable
 * access policy and the assignment and path-write paths a scan runs. `initialization` (the
 * initializer walk), `restart`, `retain_apply`, `retained_graph`, `retain_capture` (the capture of
 * the retained values at a due save) and the `std`-gated `owned` and `reconfiguration` are
 * load-time or control-path and stay in flash. So, since 21 October 2026
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
 * 19 KB) run from flash. The three patterns take every `stdlib::fbs` item but those of `members`
 * and `registry`, whose tables and names preparation reads; a block's place lookup
 * (`BlockPlaces::member`, 100 bytes) did not inline into its reads and writes and ran from flash
 * (`profile-p1n-v1p`: 10 samples), so it has a pattern of its own, as have the leaf lookup and the
 * store's reads and writes of a member's leaf bytes, which a BOOL or TIME member takes (P1-4). The
 * places of a block's members, and the module's POU and native symbol lookups that a block call
 * and a conversion make (checklist P1-4; `profile-p1k-v1p`: 9 samples from flash with the engine
 * view a cycle takes, `RuntimeState::engine`). */
*(.text.*18trust_runtime_core6stdlib3fbs[!78]*)
*(.text.*18trust_runtime_core6stdlib3fbs7[!m]*)
*(.text.*18trust_runtime_core6stdlib3fbs8[!r]*)
*(.text.*11BlockPlaces6member*)
*(.text.*11BlockPlaces4leaf*)
*(.text.*12store_access*16read_place_bytes*)
*(.text.*12store_access*17write_place_bytes*)
*(.text.*18trust_runtime_core2vm4call6stdlib*)
*(.text.*2vm8prepared15builtin_members*14PreparedModule13builtin_table*)
*(.text.*2vm8prepared15builtin_members*14PreparedModule14builtin_places*)
*(.text.*11collections*9SortedMap*10VmPouEntry*8position*)
*(.text.*2vm7symbols*13NativeSymbols3get*)
*(.text.*2vm6engine3api*12RuntimeState6engine*)

/* The frame stack's push and pop, and the prepared declarations a call and a native block look up
 * (`profile-3`: 14 and 37 samples from flash); the declaration groups' lookups, not their building. */
*(.text.*18trust_runtime_core2vm6frames*10FrameStack3pop*)
*(.text.*18trust_runtime_core2vm6frames*10FrameStack4push*)
*(.text.*18trust_runtime_core2vm8prepared*14PreparedModule11declaration*)
*(.text.*18trust_runtime_core2vm8prepared*14PreparedModule12declarations*)
*(.text.*18trust_runtime_core2vm8prepared*14PreparedModule15presence_offset*)
*(.text.*18trust_runtime_core2vm8prepared7indexes6groups*3get*)

/* The call code that every standard call of the general path runs, back in RAM with the 24 KiB the
 * main heap gave to code (the owner, 21 October 2026, "Nothing else gets slower"; spec 34 D.2: a
 * call-heavy program from flash was a third slower on 19 October). Chosen by the host's instruction
 * counts per cycle of the perf bench (`scan-profile-14`): the native call's preparation (about
 * 19 KB, 55,000 instructions a cycle), the bindings' target reads and the text-call check, the frame
 * slots and the flattening of a general-path reference, the conversion checks and the deadline
 * check, each of them from flash before. The bindings' builders and the rest of the call machinery,
 * the conversions themselves and an array element's offset (`array_offset_i64`, 1.5 KB) stay in
 * flash, for the room of the stack window (p1s2, 21 October 2026). */
*(.text.*18trust_runtime_core2vm4call29prepare_native_call_with_args*)
*(.text.*18trust_runtime_core2vm4call8bindings20resolve_vm_arg_value*)
*(.text.*18trust_runtime_core2vm4call8bindings19is_current_location*)
*(.text.*18trust_runtime_core2vm4call8bindings20read_vm_target_value*)
*(.text.*18trust_runtime_core2vm4call8bindings7targets*13VmWriteTarget17caller_block_slot*)
*(.text.*18trust_runtime_core2vm4call8bindings7targets*13VmWriteTarget14reference_view*)
*(.text.*18trust_runtime_core2vm4call8bindings7targets*13VmWriteTarget4peek*)
*(.text.*18trust_runtime_core2vm4call8bindings7targets*13VmWriteTarget4read*)
*(.text.*18trust_runtime_core2vm13dispatch_text12text_call_of*)
*(.text.*18trust_runtime_core2vm13dispatch_text12text_call_at*)
*(.text.*18trust_runtime_core2vm6engine12frame_blocks*)
*(.text.*18trust_runtime_core2vm6engine15flat_references*17flatten_reference*)
*(.text.*12store_access*27recycle_displaced_reference*)
*(.text.*16ExecutionContext24check_execution_deadline*)
*(.text.*18trust_runtime_core6stdlib4time13is_split_name*)
*(.text.*18trust_runtime_core6stdlib4time21is_runtime_clock_name*)
*(.text.*18trust_runtime_core6stdlib11conversions4util21is_conversion_allowed*)
*(.text.*18trust_runtime_core6stdlib11conversions9bitstring22integer_to_bit_string*)

/* An operation's run of charges and a program call's edge inputs, which every compact operation and
 * every program call reach (`profile-p1j-v1p`: 9 and 5 samples from flash). */
*(.text.*18trust_runtime_core2vm6budget*15ExecutionBudget10charge_run*)
*(.text.*18trust_runtime_core2vm8prepared7indexes*14PreparedModule11edge_inputs*)

/* The statement-start lookup that the statement hook's check makes at every instruction while
 * debugging is enabled (checklist P1-10; `profile-p1j-perf`: 434 samples from flash, and
 * `profile-p1l-perf`: 354 more in `statement_start` itself). */
*(.text.*11collections*9SortedMap*15statement_index13PouStatements*8position*)
*(.text.*4core5sliceSm13binary_search*18trust_runtime_core)
*(.text.*14PreparedModule15statement_start*)

/* The small functions of every cycle that `profile-p1k-v1p` sampled from flash (checklist P1-6 and
 * P1-7; 2 to 14 samples each, about 2.5 KB together): the runner's events, metrics' records (in
 * whole nanoseconds since p1x) and observer,
 * the device services' watchdog arming, the clock and the allocation count, the vectors and copies
 * the cycle's staging makes, the critical section, a smol_str's text, an integer conversion's
 * integer branch and a bound copy's admission. */
*(.text.*22trust_runtime_services6runner5state*7Records*10push_event*)
*(.text.*22trust_runtime_services7metrics*6record*)
*(.text.*22trust_runtime_services7metrics*12record_cycle*)
*(.text.*22trust_runtime_services7metrics*11record_task*)
*(.text.*22trust_runtime_services7metrics*11record_call*)
*(.text.*22trust_runtime_services7metrics*23record_scan_allocations*)
*(.text.*22trust_runtime_services6runner4span*)
*(.text.*22trust_runtime_services6runner8observer*12ScanObserver*4task*)
*(.text.*22trust_runtime_services6runner8observer*12ScanObserver*5since*)
*(.text.*22trust_runtime_services6runner13online_change*11ChangeState6expiry*)
*(.text.*22trust_runtime_services6runner6engine8services*14DeviceServices*3arm*)
*(.text.*22trust_runtime_services4chip10process_io8read_bit*)
*(.text.*13trust_esp32c620allocator_operations*)
*(.text.*20trust_platform_esp328hardware4wait11now_instant*)
*(.text.*5alloc7raw_vec*11RawVecInner17try_reserve_exact*)
*(.text.*5alloc7raw_vec*11RawVecInner10deallocate*8smol_str)
*(.text.*5alloc3vec*3VechE17try_reserve_exact*)
*(.text.*5alloc3vec*3VechE15append_elements*18trust_runtime_core)
*(.text.*5alloc3vec*3VechE4push*18trust_runtime_core)
*(.text.*2vm6frames*10FrameStack16reserve_for_push*)
*(.text.*4core5slice20copy_from_slice_implhE*)
*(.text.*4core4time*8Duration14saturating_sub*)
*(.text.*8smol_str*4Repr6as_str*)
*(.text._critical_section_1_0_acquire .text._critical_section_1_0_release)
*(.text.*6stdlib11conversions8dispatch15convert_integer*)
*(.text.*6stdlib11conversions7numeric14convert_to_int*)
*(.text.*6stdlib11conversions7numeric19signed_int_from_i64*)
*(.text.*6stdlib11conversions7numeric21unsigned_int_from_u64*)
*(.text.*6stdlib11conversions4util13value_type_id*)
*(.text.*2vm7helpers24borrowed_value_is_cloned*)
*(.text.*2vm7helpers26materialize_borrowed_value*)
*(.text.*2vm4call8bindings24clone_value_with_profile*)

/* More small functions of every cycle that `profile-p2l-t2q` and `profile-p2l-v1q` sampled from
 * flash, where a build that moves their addresses moved the cycle by several µs (checklist P1-6,
 * 22 October 2026; about 620 bytes together): the link's line end, the hold port of the scan's
 * start, the Live Values' idle check, the byte vector reservations, the I/O address list's clear,
 * the retained capture's due check, the wait alarm's interrupt clear and the byte slices' compare;
 * and the event store's drop of its oldest event where it lies, five times a cycle (about 300
 * bytes; `drop_glue::<RuntimeEvent>` was in flash in firmware p2o). */
*(.text.*13LineAssembler*8line_end*)
*(.text.*14DeviceServices*9hold_port*)
*(.text.*15LiveValueQueues*7is_idle*)
*(.text.*11RawVecInner11try_reserve*)
*(.text.*3VechE11try_reserve*)
*(.text.*9IoAddressE5clear*)
*(.text.*13RetainCapture16interval_elapsed*)
*(.text.*8systimer*5Alarm*15clear_interrupt*)
*(.text.*14SlicePartialEqhE17equal_same_length*)
*(.text.*9vec_deque5drain*12RuntimeEvent*)
*(.text.*9drop_glueNtNtCs*22trust_runtime_services6events12RuntimeEventE*)

/* The runner's loop around each cycle (checklist P1-7, 21 October 2026): the boundary, into which
 * the scan's timed window inlines (its hooks' setup, the scan's allocation count, the cycle's end
 * event and its metric), the shell's wait and frame polling, esp-hal's alarm arming and interrupt
 * dispatch, the slice range indexing and the vector buffer drops the cycle calls, 7.1 KB. From
 * flash they took 18% of the busy samples of `t2q` (t2p with a 2 ms task, firmware `xnocopy`),
 * and a code change that moved them in the flash cache added 23 µs to every cycle against p1x.
 * `Runner::run` (5.8 KB, 2% of the samples) stays in flash. */
*(.text.*22trust_runtime_services6runner*13take_boundary*)
*(.text.*22trust_runtime_services6runner5shell*10wait_until*)
*(.text.*22trust_runtime_services6runner5shell*10next_frame*)
*(.text.*22trust_runtime_services6runner5shell*13expire_change*)
*(.text.*22trust_runtime_services6runner5shell*12link_sending*)
*(.text.*22trust_runtime_services6runner5shell*10wake_token*)
*(.text.*7esp_hal5timer*8AnyTimer*16enable_interrupt*)
*(.text.*7esp_hal5timer8systimer*5Alarm10set_enable*)
*(.text.*7esp_hal9interrupt5riscv2rt17handle_interrupts*)
*(.text.*__esp_hal_internal_wait_alarm*)
*(.text.*5range7RangeToj*9index_mut*)
*(.text.*5alloc7raw_vec*6RawVec*4Drop4drop*)

/* The allocator and the critical section the link and the wait share. The transcendental math of
 * `libm` (SIN, EXP, SQRT and their kin, about 18 KB) runs from flash for the same margin; the
 * arithmetic of REAL and LREAL is the ROM's soft float. */
*(.text.*14embedded_alloc* .text.*21linked_list_allocator*)
*(.text.*16critical_section* .text.*8esp_sync*)
