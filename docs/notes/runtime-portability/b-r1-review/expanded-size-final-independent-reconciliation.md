# Final authorized expanded-size attempt: independent reconciliation

Independent reviewer `/root/b_independent_review` inspected retained logs, map, report and source identities only. No compilation, linker, tests, formatter, object-tool rerun or hardware access was performed. This note does not close Scope B.

The retained campaign records the first ordinary attempt failing with E0282/E0283 before a map and its safe comparison marked UNRUN. The remaining authorized attempt used the exact reviewed BytecodeError type annotation and safe ICF. Its source manifest contains 6,663 records, all matching current local files at inspection, SHA-256 94e91e74b5df04acd07bd7ff22e3016115da4b2c7fc2e1eba0cc85590c7ba4f4. Relative to the prior expanded freeze, the decoder annotation is the sole changed existing source path. No same-source successful ordinary measurement exists.

## Failed fit, accurately measured

The second attempt compiled sufficiently to reach the linker and produce a map, but no installable firmware ELF. Link exit101 and map-report exit0 are consistent. The map reports .text 435,476 bytes, .rodata 58,028 bytes, vector table404 bytes and .bss73,788 bytes. Aligned code end0x080887e0 minus origin0x08010000 gives493,536 bytes against L2capacity458,752: overflow34,784 bytes. The raw linker reports34,756 at .rodata/.data and34,784 after .gnu.sgstubs alignment, exactly as expected.

Relative to B1, code span decreased71,936 bytes and flash-load bytes decreased71,952. Relative to size1, span decreased33,440 bytes. These are combined changes across source and linker configuration, not an isolated causal saving from ICF. The map's approximate symbol families include validator54,498, dispatch53,988, engine44,940, preparation35,366 and B-tree26,854 bytes. Its remaining shared sorting attribution is986 bytes. Large individual symbols include dispatcher37,904 and preparation28,608 bytes; these owner sizes identify where code resides, not amounts safely removable. Full device validation and all IEC functions remain required.

## Supporting inspection failure remains failed

Object inspection exit1 is genuine: the filename glob selected an LLVM-bitcode rcgu.o before the final ELF object, and readelf rejected the bitcode. The retained output subsequently shows the final ELF object's .llvm_addrsig section at row13,452. The link log records selected/removed identical sections, so address-significance emission and actual safe folding are observed despite the aggregate inspection defect. Keep the recorded step failed; do not overwrite it or call the entire inspection successful. Future tooling should distinguish ELF object magic from bitcode before invoking readelf, within an authorized batch.

Firmware flags retain cortex-m4, addrsig, save-temps, both reviewed linker scripts, map output, --icf=safe and folding output. Overflow checks and supported functions were not disabled. Static .bss does not establish live allocator or stack fit. No board backup/program/UART execution occurred.

Both authorized size attempts are consumed. No further measurement was launched or recommended as automatic continuation. A separately already-authorized software validation batch may proceed with this failed link retained as the failed hardware prerequisite; board steps remain unrun and Scope B remains incomplete. Individual retained artifact hashes accompany this note.
