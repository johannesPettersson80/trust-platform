# Independent Scope B run-1 evidence and correction review

This is read-only source/evidence reconciliation by `/root/b_independent_review`; no build, test, formatter or hardware command was run. No source file was edited. The review writes only this external record and its accompanying identity manifest.

## Batch outcome confirmed

The raw ledger contains 22 steps: 18 exit zero and 4 fail. Formatting preparation has explicit `preparation`/`complete` time labels rather than measured UTC start/end fields; do not present these two rows as timestamped execution observations. The failed steps are native-tools (101), firmware-link (101), clippy (101) and supply-chain (1).

- Native bundle suite: 1 passed, no failures/ignored/filtered.
- Runtime GPIO and mandatory runtime vertical: five binaries, 3 + 1 + 20 + 1 + 4 = 29 passed, no failures/ignored/filtered. The GPIO source test actually executed both input levels at all four call depths.
- native-tools produced a compilation error for the wrong include_str path. There are no native test-result lines in that log: do not claim platform arithmetic or xtask assertions executed successfully merely because packages compiled.
- Clippy reports the same include-path error and manual_range_contains in the ELF inspector.
- F401 adapter cargo check and firmware Clippy passed. Neither proves firmware linkage or physical execution.
- Link failed because FLASH overflowed, reaching a maximum reported overflow of 205,024 bytes. `unrun.txt` explicitly leaves ELF inspection and the physical run unrun. There is no firmware ELF or UART board result in the retained run.
- Supply-chain reports RUSTSEC-2026-0110 against bare-metal 0.2.5 via cortex-m and bare-metal 1.0.0 via stm32f4xx-hal. This remains a failed gate, not a resolved dependency decision. The log separately warns about yanked spin 0.9.8; that warning is not the reported fatal advisory.

## Map evidence

The failed linker map independently agrees with the overflow: vector table 404 bytes; .text 487,876 bytes at 0x08008000; .rodata 77,580 bytes at 0x0807f1c8; final aligned flash location 0x080920e0, which is 205,024 bytes beyond the 0x08060000 firmware boundary. Static .bss is 73,788 bytes, consisting of the declared 73,728-byte heap arena plus 60 bytes, with zero-size .data/.uninit. This static map does not prove live heap fit or native stack headroom. No hardware run occurred.

## Prepared corrections confirmed, not validated

1. The trace oracle now uses concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/..."). For xtask, that resolves to the actual checked-in fixture. Restoring only the former include_str expression reproduces the run-1 frozen file hash exactly; no trace assertions changed.
2. The ELF RAM interval now uses (RAM.start..0x2001_8000).contains(&start). For u64 it has precisely the former inclusive-lower/exclusive-upper semantics and implements the Clippy suggestion. Restoring only the original comparison reproduces the frozen file hash exactly.

Both changes are source-reviewed only. No rerun was requested or executed by this reviewer. FLASH fit and supply-chain disposition remain unresolved, so Scope B is not complete.

## Identity reconciliation

The frozen source manifest contains 6,607 file records, including 2,041 Rust files. At reconciliation time every recorded path existed. Only four recorded paths differed: generated architecture SVG, diagram manifest, xtask/src/portability/trace.rs and xtask/src/portability/elf.rs. The first two are the documented batch render outputs, and the latter two are the exact post-batch corrections above. Thus 2,039 frozen Rust file identities remain unchanged. Root documentation closeout is still being updated and may subsequently differ; those later edits are not runtime validation.

The accompanying independent-run-1-reconciliation.json records individual raw artifact hashes, failed map hash, frozen/current changed identities and ledger disposition. It is an independent snapshot of retained evidence, not an assertion of fresh execution or exact-SHA release approval. The standalone firmware dependency graph remains separate from the root workspace graph, as documented in the original review.
