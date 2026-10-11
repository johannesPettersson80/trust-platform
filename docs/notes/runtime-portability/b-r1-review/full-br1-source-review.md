# B-R1 full independent source review before freeze

Reviewer `/root/b_independent_review`; canonical bootstrap parity verified against the primary checkout. No production edits, builds, tests, formatting or probe operations performed. This expands the earlier CRC/preparation/registry review to the grouped indexes, shared sort, layout, host tooling and dual-lock supply gate. Root execution documentation and batch script remain in preparation; their final review is separate.

## Verdict and prerequisites

No additional definite implementation blocker found by source inspection. Before frozen --locked validation, refresh the standalone firmware lockfile: at this review snapshot it still lists crc32fast as a direct firmware dependency although the manifest removed it. The root has acknowledged that lock authoring and final formatting/policy line rebinding are queued. Do not treat this pre-format source review as compilation or hardware-fit evidence.

The requested run-1/frozen-source directory does not exist locally. Review used A4 HEAD for tracked diffs and run-1/raw/source-sha256.txt plus source.patch for the B1 baseline. The accompanying identity manifest records B1 hashes where available and current reviewed identities. No nonexistent source archive is claimed.

## Correctness and accounting

- CRC preserves the reflected IEEE algorithm and wire contract; std acceleration and no_std compact implementation share the public boundary. Test-only acceleration remains available for differential assertions. Firmware bundle checks use the same core checksum.
- The byte preparation route retains bounded decoding, supplied-byte artifact length, full validator and profile checks while omitting redundant serialization. Struct-built inputs retain bounded serialization before shared preparation. Route accounting assertions and malformed/padded artifact cases are authored.
- All 89 original standard registrations preserve names, fixed/variadic parameter metadata and implementation function paths. Immutable descriptors are case-insensitively searched in sorted families; custom owned registrations take priority, empty Default and new() remain distinct, clone isolation and conversion fallback remain. Public StdParams moves Vec payloads to Cow: runtime behavior is preserved, but source callers constructing variants require adaptation. In-tree constructors were reviewed.
- Shared heapsort preserves the prior validator's comparison/swap/charge sequence, including equal-key permutation and exhaustion-before-swap. It allocates no scratch. Both validation and preparation supply their own error/budget adapters. The algorithm remains n-log-n, and string comparison visits are charged. Tuple/slot/name keys retain their intended ordering.
- Groups append (key, ordinal, value), sort by key and original ordinal, then flatten into keys/ranges/values. This preserves per-key action, edge, retained and root wire order. Only physical declarations receive the previous slot-order sort. Pending/flat/key allocations and geometric growth are charged before reservation; pending capacity retires after finishing. Native tests cover reference BTreeMap order, reversed unique keys, budget exhaustion and fixture-wide grouped equivalence. No wholesale ordered-container replacement was introduced.
- The native allocator instrument delegates unchanged System pointer/layout pairs and uses nonallocating thread-local counters. Main, numeric and GPIO fixtures are all measured through preparation, instantiation, scans and retirement. Its result is requested payload accounting on the selected host layout, excluding MCU allocator overhead, fragmentation and native stack.

## Link layout and reporting

L2 linker and ELF inspector agree: sector 0 vectors; sector 1 application; sectors 2 and 3 separate future checkpoint erase units; sectors 4 through 7 provide 448 KiB firmware code. RAM remains 72 KiB heap, 16 KiB MSP, at most 8 KiB other statics. Reset vector Thumb/entry checks and load-region overlap rejection remain intact. No persistence implementation is claimed merely by reserving two sectors.

Map reporting parses explicit LLD output columns, verifies required sections, deduplicates exact symbol aliases and rejects overlapping non-alias ranges. Attribution is disjoint and explicitly approximate; unmatched bytes remain residual. Debug/RAM-only sections are excluded from flash-load totals. Zero-size flash alignment extends code span without adding load bytes, with a dedicated regression. Both old and new maps are compared against L2 capacity and labeled accordingly; old B1 overflow remains separate historical evidence. Only valid ELF inspection can authorize installability.

## Supply-chain orchestration

The gate audits root and standalone locks with cargo-deny and cargo-audit. Bash pipefail preserves audit failure through the JSON policy pipeline, and status accumulation allows all four independent graph outcomes to be observed. Orchestration tests use controlled subprocess stand-ins and cover failure of each graph invocation; they prove wiring, not real dependency safety. The firmware yanked allowlist is empty. The bare-metal informational exception has an owner, rationale, removal condition and expiry 2027-01-08, 90 days after 2026-10-10. Real audit evidence remains required in the batch. Root metadata still does not enforce standalone firmware dependency edges; locked firmware manifests/builds and explicit dual-lock checks provide separate evidence.

Primary API references consulted for compile-risk review: https://doc.rust-lang.org/std/borrow/enum.Cow.html (FromIterator<T> for Cow<[T]> is stable), https://embarkstudios.github.io/cargo-deny/cli/check.html and https://embarkstudios.github.io/cargo-deny/cli/index.html. These references do not replace compilation with the pinned tools.

All footprint savings, flash fit, native assertions and physical behavior remain unverified until authorized execution. Full final batch-script and post-format identity reconciliation remain pending.
