# Run8 prepared-owner linked-image review

Accepted to proceed through the authorized focused software gates and, only if all prerequisites pass, guarded physical replay. This is source-bound linked evidence, not physical acceptance or a worst-case stack proof. Reviewer /root/br1_static_registry ran no compiler, test, formatter, linker or board command; only disassembled the existing ELF.

Frozen source a95bcff98b5d4a51e70acf7e29cf0113b585ac69bcfc39c4cc5fbac6b1e8337c; ELF a0e6ed2d7f725fd17746d30c229d02e5b0125c00c494015f3be40322e293ec1e. Prior five-file source and fourteen-entry script identities match exactly. Upper free 18,720 bytes passes the unchanged 16,384-byte threshold.

Own fixed native frames (including saved registers):

| Owner | Run7 | Run8 | Delta |
| --- | ---: | ---: | ---: |
| main_fixture | 2488 | 1416 | -1072 |
| numeric_fixture | 2408 | 1344 | -1064 |
| gpio_fixture | 3584 | 2520 | -1064 |
| prepare | 2096 | 2888 | +792 |
| instantiate_with_services | 1824 | 1824 | 0 |
| execute_with_buffers | 1872 | 1872 | 0 |
| recursive construct | 384 | 384 | 0 |

Preparation returns before deep instantiation. Its additional transient frame therefore does not erase the fixture-frame reduction during initialization. The combined main+prepare own frames fall by 280 bytes; no claim is made that all preparation callees are a complete bound.

At 0x08013eea, r0=4; at 0x08013eec, r1=0x324; 0x08013ef0 calls MeasuredHeap::alloc. These are alignment 4 and payload 804 bytes. On success, the prepared owner is copied into that allocation (12 bytes then 0x318 bytes); null branches to the existing handle_alloc_error. This is one instrumented allocation before the PREP/MEM records, retained through the borrowed EngineState lifetime. Actual allocator rounded usage/peak, rather than the payload alone, must be read from hardware MEM evidence.

The previously identified main counter/history/array path loses 1072 bytes, taking its fixed-frame sum from 13944 to 12872. Applying that delta to run7's measured 14668 yields an illustrative 13596 used / 2788 remaining, 740 above the required 2048 margin. This extrapolation is not acceptance: leaf/interrupt placement, other fixtures, depth-four calls and allocator effects still require physical measurements. Run7 actually exceeded an earlier fixed-frame estimate by 724 bytes, so a small static apparent margin cannot replace the guarded trace.

No functions, admitted logical depth, heap/MSP regions, flash margin, fault behavior or trace records were reduced. Proceeding to measured guarded replay is justified; Scope B stays open until all required traces and margins pass.
