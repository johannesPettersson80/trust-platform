# B-R2 execution record

Owner authorization: “implement it” for the revised checklist, 10 October 2026.
All currently supported PLC functions remain, including float/text directions and
assertions. Stable Rust 1.95.0, overflow checks, full admission and fault recording stay.
No commit/push or automatic rerun. Two size-only measurement attempts (M1, M2) and one
final consolidated software/hardware batch are authorized. Both M1/M2 and the single final software batch completed. Flash fit failed, two Clippy gates failed, and the conditional board phase is unrun. All native behavior suites passed. No execution allowance remains.

Source: active `feat/runtime-portability-b`, base df427259cc387a7a79fb81132be23e48ea1493d4.
M1 local snapshot: `/home/johannes/projects/trust-platform-portability-br2-m1`, detached
at the same base, with B-R1 dirty source copied before runtime changes. Canonical
agent files come from `/home/johannes/projects/trust-platform`, manually copied and
verified before work. The builder snapshots will be separately identified/frozen.

## Allocation

- Root owns sector-0 layout, canonical build/remapping, ELF/map tools, documentation,
  measurement scripts, source reconciliation and final batch.
- Collection agent owns typed primitive engine map/set wrappers, unchanged full-domain
  journal representation reused for retained globals, updated capacity charges and tests.
  It also owns selected cold diagnostic helpers, preserving error codes/messages.
- Numeric/registry agent owns shared assertion comparison, proved 64-bit numeric slices,
  behavior-based registration tests, one non-generic sort driver and trace writer.
- Every authored Rust/source slice receives review by a different agent before freezing.
  Source review is not compilation or behavior evidence.

## Validation allocation (commands frozen before execution)

M1: format snapshot once; review reconciliation; canonical locked firmware release
build with safe ICF/path remapping; retain map/config/error; footprint report. No tests
or device action. A completed capacity-miss map permits the preplanned M2 source wave.
A compiler/tooling failure stops execution. M2: complete all source/test authoring,
independent review, final formatting/provenance then source freeze; canonical build
once, retained ELF/map/config and footprint report. No further firmware relink.

Final batch reuses the exact candidate ELF and B-R1 native allocation: core allfeatures,
portable and i686; both MCU library graphs; hosted units/affected integration/runtime
vertical; platform/xtask/native firmware library tests; affected/portable/firmware
Clippy; host/Windows warnings; both lock audits; architecture then canonical diagrams;
format/diff/provenance; ELF and trace inspection. Hardware only after software and
flash/headroom prerequisites: backup, safe sector installation, unchanged three
artifacts, real-time traces, heap/MSP/IRQ/timing, output safe states and IWDG reset.
Preserve every failed/unrun step. No full release guard or publication in this scope.

## Source review checkpoint

The nine layout/build/inspector/diagram paths passed independent source review. M1
formatting changed four xtask files mechanically; root reconciled the complete diff.
The fourteen collection/cold-error paths passed a different-agent source review.
Numeric/sort/trace review found a UART record-boundary regression before execution;
the owning trace implementation was corrected and independently accepted before the final source freeze. No review is test proof.

## M1 result

The authorized placement-only attempt reached the linker and produced a complete map.
The canonical build returned failure solely on flash capacity; footprint reporting passed.
Upper span: 474,400 B against 458,752 B, a 15,648 B overflow. Sector-0 immutable
section: 14,720 B; vectors 404 B and padding 12 B, leaving 1,248 B. Total flash
load: 489,516 B. Against retained safe ICF, upper span improved by 19,136 B and
actual load shrank by 4,392 B. Relocated bytes are not counted as engine shrinkage.
M2 requires 32,032 B additional upper-span reduction to meet the 16 KiB margin.
No ELF or hardware result exists. The planned source wave proceeds; this is not a retry.

Selected source changes are typed raw-ID maps/sets and full-domain journal reuse,
shared charged-sort control flow, assertion comparison, cold error construction,
proved numeric/time narrowing, registry behavior tests and a byte-identical bounded
UART writer. Name caches and wide date/text/ULINT intermediates remain: their removal
needs additional domain proof, and no 128-bit builtin saving is claimed. No broad
panic elimination or speculative dispatcher split is included. All PLC capabilities stay.

M1 map SHA-256: `bbbb44505304181e2593f11c16b2a7ad6ff93c14f4045c462e59b98bb7300dac`.

Installer review found that OpenOCD image-wide erase can erase holes between ELF
sections. The authored procedure now uses explicit sector 0 and 4–7 erases, writes
the firmware without automatic erase, and erases application sector 1 separately.
It compares checkpoint bytes across installation and both firmware regions around
application installation. Source: [OpenOCD Flash Commands](https://openocd.org/doc/html/Flash-Commands.html).
No device command has run.

## Final result and stop boundary

The complete source was frozen as 409 records, SHA-256
`230f05c6020e90a8d08be922a913b1d03758dfd70c64a9086e78f26c6571847b`.
Independent reviews cover placement/build/inspection, collection/error boundaries,
numeric/sort/trace changes, the corrected UART transaction and installation scripts.
Formatting was separately reconciled; both lockfiles and three saved artifacts stayed unchanged.

| Measurement | Upper span | Upper overflow | Sector-0 free | Actual flash load |
|---|---:|---:|---:|---:|
| M1: placement/remapping | 474,400 B | 15,648 B | 1,248 B | 489,516 B |
| M2: complete source candidate | 476,256 B | 17,504 B | 1,728 B | 490,896 B |

M2 reached the linker and failed only capacity assertions; map reporting passed.
There is no ELF to install. It is 33,888 B short of the 16 KiB upper-margin criterion.
Relative to M1, BTree/engine/sort/stdlib local savings were outweighed by growth in
validator/decoder/dispatch/firmware. The whole image grew by 1,380 B and upper span by
1,856 B; no isolated causal saving is inferred from these overlapping compiler effects.
The linked wide numeric helpers remain; additional narrow helpers appeared. See the
independent measured attribution in the evidence directory. Do not represent this source
wave as a net footprint success or silently keep optimizing under a spent measurement budget.

The final consolidated batch ran from 18:23:14 through 18:28:54 UTC after its single
preparation. Its 40-row ledger has 35 required PASS, two required FAIL, one required
UNRUN and two advisory PASS. The failures are affected Clippy (one inclusive-range
expression) and portable Clippy (four redundant u64 casts). ELF inspection is unrun;
physical installation/UART/heap/MSP/watchdog are also unrun. No device command occurred.
Architecture, canonical diagram rendering/drift, host/Windows compile warnings, both
supply-chain graphs, format/diff, all artifact pins and advisory metadata passed.
Native passed counts are 366 core all-features, 326 portable, 246 i686, 3,841 hosted
unit, 538 integration across 68 binaries, 91 native tooling, five firmware-library
and four provenance-helper tests. They overlap; they are not a unique-test count.
No behavior test failed or was ignored.

The i686 allocator payload peaks remain: preparation maximum 28,069 B, instantiation
23,033 B and scan 25,311 B. These are host32 payload measurements, not F401 allocator
overhead, stack/IRQ evidence or allocation-free RUN proof. No ARM stack number exists.

After the batch ended, five expression-only Clippy corrections were prepared in
`xtask/src/portability/elf.rs` and `crates/trust-runtime-core/src/stdlib/conversions/string.rs`.
An independent review confirms preserved semantics and the exact before/after hashes.
**Those corrections have not been compiled, linted or tested.** The final diagram SVG
and manifest were copied from the successful rendering. Later changes otherwise concern
this evidence, checkpoint and byte-preservation attributes. No retry, commit or push ran.

Current checkpoint: implementation authored, reviews retained, native behavior evidence
passed on the frozen candidate, but scope not verified/closed because fit and lint gates
remain open. The two measurement attempts and final batch are consumed. Further links
or validation require new owner authorization under the explicit no-automatic-reruns rule.
Do not infer permission for another run from these prepared corrections. Function cuts,
overflow-off builds, unrestricted ICF and toolchain changes remain forbidden.

[Retained evidence index](b-r2-evidence/README.md) includes commands, ledgers, reviews,
frozen manifest, map digests, compact footprint reports and original test outputs.
Raw maps and source archives remain outside git in the task artifact directory.

M2 map SHA-256: `8e309b32489157aabc4ac3986a1f8a5b2d2adf9ee2a708c41c745022b874ccb0`.
