# B-R4 run1 linked-image inspection

Independent reviewer `/root/br1_static_registry`; read-only artifact analysis while the validation batch remains running and source stays frozen. No compiler, test, formatter, linker, ELF inspector rerun, or board operation launched. Copied retained measurement artifacts and disassembled the existing ELF using builder `/usr/bin/llvm-objdump-21 --disassemble --demangle`. Remote checkout HEAD df427259cc387a7a79fb81132be23e48ea1493d4 and canonical instruction hashes matched the prepared batch. Builder checkout is detached; local development branch remains feat/runtime-portability-b.

Upper code span is 442,688 bytes in 458,752 capacity: 16,064 bytes free, 320 bytes short of the unchanged 16,384 requirement. Relative to B-R3 run3, text grows 240 bytes, rodata 248 bytes and padding falls 8 bytes, so occupied span grows 480 bytes. Sector zero remains 16,160 occupied / 224 free; repacking that remaining space alone cannot recover 320 bytes. BSS remains 73,788 bytes. The image linked but failed the retained inspector's headroom gate; this does not authorize hardware execution.

## Fixed own native frame sizes

Sizes count the prologue's SP subtraction plus pushed saved registers. They exclude callees, interrupts, runtime paths, alignment at call sites and actual high-water measurement.

| Function | B-R4 bytes | B-R3 bytes where established |
| --- | ---: | ---: |
| execute_with_buffers | 2344 | 2864 |
| evaluate_initializer | 968 | 1312 |
| finish_initializer | 688 | formerly inlined |
| initialize_frame | 248 | 248 |
| finish_call | 1208 | new continuation completion |
| finish_stack_result | 344 | not compared |
| EngineState::execute_pou | 608 | not compared |
| instantiate_with_services | 1824 | not compared |
| RuntimeState::execute_cycle | 256 | 256 |
| execute_cycle_inner | 912 | 912 |
| initialize_instance | 256 | not compared |
| firmware main_fixture | 2480 | old combined runner 3736 |
| firmware numeric_fixture | 2416 | old combined runner 3736 |
| firmware gpio_fixture | 3584 | old combined runner 3736 |
| firmware outer __cortex_m_rt_main | 152 | not compared |
| firmware prepare | 2096 | not compared |

The separate phase frames replace the old single combined runner frame; they are not all simultaneously active. Ordinary native user-call recursion helpers execute_vm_target and execute_native_vm_pou_call are absent from this linked firmware. start_call is inlined in the shared dispatcher. Synchronous construction/default/initializer nesting remains and must still be physically measured; these local sizes do not establish a complete native-stack bound.

## Minimal follow-up candidate

runner.rs contains eight separate B1,FAIL formatting sites, plus main.rs's runner failure. A concrete shared cold non-inlined trace::failure(console, phase, code) can preserve the exact bytes, single bounded Console transaction, ignored UART-error semantics, existing safe_off order and fault returns while avoiding repeated formatting setup/static descriptors. The current measured main/numeric/GPIO code symbols are 1736/1572/8436 bytes. No exact saving is claimed; 320 bytes is the required measured recovery and the next link needs its own authorization. finish_stack_result already has a 376-byte shared text symbol, so blindly adding noinline there is not justified by this map. No functionality, resource allowance, partition or acceptance margin should be removed.
