# M1 to M2 measured footprint handoff

Read-only analysis of retained maps and reports; no edits, builds, checks or new measurements. Source remains frozen for the running software batch.

M1 upper span 474,400 B becomes 476,256 B: **+1,856 B**. M2 exceeds L2 by 17,504 B and misses the additional 16 KiB headroom floor by 33,888 B. Occupied flash load grows from 489,516 to 490,896 B: **+1,380 B**. This is not all extra upper code: .text grows 1,244 B, upper .rodata grows 616 B, while sector-0 .firmware_rodata shrinks 480 B. Upper padding falls 4 B, accounting for the 1,856 B span delta. No F401 fit or hardware success is established.

## Disjoint measured text families

| Family | M1 bytes | M2 bytes | Delta |
|---|---:|---:|---:|
| adapter | 2,154 | 2,258 | +104 |
| allocator | 304 | 306 | +2 |
| btree | 26,854 | 25,822 | -1,032 |
| decode | 15,226 | 16,414 | +1,188 |
| dispatch | 53,988 | 54,698 | +710 |
| encode | 0 | 0 | +0 |
| engine | 44,940 | 43,762 | -1,178 |
| firmware | 15,876 | 17,016 | +1,140 |
| libm | 17,020 | 17,044 | +24 |
| other | 141,926 | 141,080 | -846 |
| preparation | 35,366 | 35,350 | -16 |
| sort | 986 | 562 | -424 |
| stdlib | 25,966 | 24,942 | -1,024 |
| validator | 54,498 | 57,102 | +2,604 |

Families use the existing report's disjoint attribution; literal pools/alignment remain residual. Generic owner classification is approximate. These are simultaneous-build deltas, not isolated savings per patch.

## Strongest identifiable changes

- **Assertions are a real local reduction.** Each of six comparison wrappers goes from 292 to 12 B; shared assert_compare and comparison_failure add 322 and 168 B. That grouping saves 1,190 B before nearby tiny changes. Messages and behavior remain unchanged by source review.
- **Shared sort genuinely shares the loop.** The family falls 986 to 562 B (424 B). Old typed heap_sort/sift sum 436+550 B; new facade, adapter closures and monomorphic driver/sift sum 132+148+108+174 B. This is far below prior broad sort-family estimates because B-R1 already removed the expensive standard sorting implementations.
- **Primitive collection identities help, but do not eliminate the trees.** BTree family drops 1,032 B and engine drops 1,178 B. Examples: reserve_instance drops 476 B, initialize_frame 564 B, retire_frame 190 B. Native maps/set node operations, full Value-journal keys, payload drops and other still-distinct types remain. Removed monomorph counts cannot be multiplied by an average tree size: implementations and ICF aliases were already shared and ownership moves between helpers/callers.
- **Numeric narrowing adds helpers while wide helpers stay.** __fixdfti remains 218 B and __floattidf remains 278 B in both maps. New __aeabi_d2lz and __aeabi_d2ulz add 138+114 B. The remaining stdlib/helpers.rs scale_time uses result as i128, so there is a concrete unchanged owner keeping wide float-to-int conversion necessary. Named conversion real_to_int grows 160 B, signed_int_from_i64 grows 144 B, while the separately named signed_int_from_i128 disappears (264 B); disappearance can mean inlining, not removal of its semantics. The narrowing is behavior-correct, but this candidate has not removed all link roots for wide helpers.
- **Trace code shifted instead of disappearing.** Five trace_wire routines add 650 B; trace wrapper/closure add 54 B. Firmware prepare grows 372 B and run 80 B. Several old formatting implementations disappear from the broad other family (u64 decimal 266 B, i16 decimal 182 B, u32/u64 lowerhex 78/92 B), partially offsetting that. Console's old write_fmt 104 B is replaced by write_record 104 B, while its aggregate adapter family grows 104 B overall. Sum neither trace-family growth nor removed format functions alone as total trace savings.
- **Admission and dispatch offset the above reductions.** Validator +2,604 B, decoder +1,188 B and dispatch +710 B dominate. decode_section_data alone grows 1,016 B; execute_with_buffers grows 668 B. Validator boundaries reorganize heavily: validate_construction changes from a 13,052 B closure to a 12,248 B named function, validate_initializers appears separately at 7,804 B, and validate_with_limits shrinks 5,304 B. Those four together grow 1,696 B. This is compiler inlining/outlining and caller/callee redistribution, not 20 KiB of newly authored validation logic. B-R2 adds cold/noninline error constructors used throughout these callers; changed optimization boundaries are a plausible source of the growth, but the two aggregate maps cannot isolate causal bytes. section_diagnostic and preparation error conversion themselves are only 18 B each; their tiny outlined size does not prove a net benefit in all callers.
- **Read-only tables/payloads grow too.** Merged string input shrinks 499 B (9,902 to 9,403); anonymous read-only inputs grow 602 B (26,814 to 27,416). Anonymous identifiers are unstable between builds, so map names alone cannot distinguish new vtables from diagnostic locations or constant payload rearrangement. StableErrorCode display-table names change to as_str-table names at equal paired 332 B sizes; that renaming is not new table cost.

## What the evidence supports

The selected sharing changes produced real local savings, but their estimates did not account for remaining helper roots, wrapper/adapter costs, and whole-program compiler layout changes. The full combined implementation is 1,380 B larger in occupied flash than M1. Source-level simplification, smaller type variety and correct behavior are separate claims from a smaller final image. No safe function cut, new measurement request or further optimization plan is implied by this handoff.
