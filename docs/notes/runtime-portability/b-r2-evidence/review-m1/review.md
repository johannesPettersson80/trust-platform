# B-R2 M1 independent source review

Accepted for the authorized first measurement; no remaining source-review blocker found in the nine pinned paths. This is read-only source review, not compilation, ELF fit, rendering, hardware, or test proof. No cargo, formatter, test, validator, linker, or renderer was executed.

Reviewed the isolated M1 composition against the active B copies, including the final 32-byte tail alignment, actual ELF alignment guard, executable .text/reset checks, and exact 16 KiB upper-margin test.

- The sector-0 selection precedes the ordinary link.x input wildcard; INSERT controls subsequent placement. Selected immutable input sections preserve vectors and the application/checkpoint erase regions. Linker bounds and alignment assertions fail closed. Current historical selected inputs have at most 8-byte alignment; 32-byte alignment deliberately spends eight additional start bytes versus the previous proposal and guards the named constant input families. Actual ALIGNOF and ELF alignment remain checked.
- The upper load end includes ordinary rodata, nonempty .data load bytes, and veneers. Empty .data does not drag a RAM address into the flash span. The inspector distinguishes sector-0 and upper loads, checks vector/reset/stack identities and allocated executable .text containing the entry, rejects reserved-region loads and writable/executable sector-0 segments, and requires 16 KiB upper headroom.
- Map totals remain compatible with historical maps lacking .firmware_rodata and with L1 text origins. Nonempty output loads are disjoint before padding subtraction; all classified loads are at or after their span origins. Thus per-region load sums cannot exceed their spans. Historical overflow is explicitly reported against the current L2 capacity, not asserted as historical installability. Empty flash alignment sections still reserve span without adding load bytes.
- The canonical Rust build entry parses checked-in target flags, preserves safe ICF/linker scripts and checked release profile, removes inherited release-profile/native compiler overrides, and passes space-containing remaps through Cargo encoded flags. The added include_str path resolves from xtask/src/portability to the root firmware directory. Path normalization is correctly not described as binary reproducibility proof.
- Native tests cover reserved gaps, overflow, overlap, immutable-section flags/alignment, exact upper-margin acceptance and one-byte-short rejection, map attribution/deltas and command flags. Their compilation and execution remain unverified until the authorized batch.
- The diagram addition uses the existing multiline note syntax; rendering remains pending.

Primary reference checked for linker semantics: https://lld.llvm.org/ELF/linker_script.html (INSERT occurs after input-section mapping; explicit output addresses and maximum input/output alignment requirements).

The identity JSON pins the final nine files. No repository source was edited by the reviewer; only required canonical bootstrap copies and this external record were written.
