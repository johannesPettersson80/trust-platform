# Final independent A4 acceptance: run 11

**Verdict:** the complete external-review correction scope is verified by run 11.
No remaining implementation or evidence blocker was found. This reviewer did not
write implementation and independently reconciled source, raw logs, archived bytes
and the earlier review coverage. This is A4 software-scope acceptance, not a release
approval, hardware qualification or guarantee that no defect can exist.

## Identity and evidence

Branch: feat/runtime-portability-a4. Base:
77b91381fcf1b1850611f88b397bbfe8d45e3523. A4 remains uncommitted.
The builder used Rust/Cargo 1.95.0. Its completed batch has 29 required passes and
two advisory passes, with no failed or unrun steps. All 50 indexed raw artifacts
match their SHA-256 hashes locally and on the builder. Artifact-index identity:
`2999e0b178e3290de3a43dbb80ea938340832e316dd00421286f22a23763eb56`.
The complete 643-record frozen source manifest identity is
`abc68ced75de1b39e1510028c4ce9669dd435ce310fa5be2892d9e2d4599e83b`.
The full post-batch source archive matches the frozen records.

Recounted native results:

| Suite | Passed | Failed / ignored / filtered |
| --- | ---: | --- |
| Core all features, including doctests | 318 | 0 / 0 / 0 |
| Core no default features, including doctests | 278 | 0 / 0 / 0 |
| i686 musl selected core suites | 199 | 0 / 0 / 0 |
| Hosted runtime unit suite | 3,841 | 0 / 0 / 0 |
| Hosted integration, 67 binaries | 536 | 0 / 0 / 0 |

The required runtime vertical is included. Clippy, Linux/Windows cross-target
warning checks, supply chain, architecture, diagram rendering/drift, formatting,
diff integrity and tooling checks pass. F401 and C6 evidence consists of core
library compilation and feature graphs; neither board executed this candidate.

All current Rust and STBC bytes match the validated freeze both locally and on
the builder. The complete final Rust identity manifest covers all 239 dirty Rust
paths/deletions: 35 deletions, three byte-identical moves, 29 adapted moves,
84 modified files and 88 authored additions/extractions. No current Rust path is
unlisted. Historical review hashes remain preserved in its coverage chains.

The only Rust formatting difference from the final pre-batch review map is
vm/type_policy/tests.rs. Its exact pre-format bytes were reconstructed from the
retained run-10 source and original reviewed block, independently reproducing
SHA-256 `f8a76b810c72dd3866130b12196d8533af55a954d037e1892fc4dcceb5d39da7`.
Every formatting diff hunk was inspected: line wrapping and trailing commas only;
values, types, strings and assertions are unchanged. The labeled reconstruction
is retained as a review artifact, not presented as a contemporaneous archive.

## External-review closure

| Item | Accepted correction and evidence |
| --- | --- |
| H1 / RH1 execution cost | Prepared declaration/type/owner/initializer/edge indexes replace repeated scans. Physical-slot journals use indexed deduplication and resolve native OUT aliases at actual write time. Inputs stage bound destinations, outputs stage bound windows. Default-limit native tests pass for 3,000 declarations/2,000 stores, 2,000 distinct input bindings and a sparse 1 MiB marker image. No limit increase. |
| M2 / RM2 reference allocation | Borrowed policy and storage traversal replace temporary owned paths. The hosted test counts actual allocator calls for 128 warmed scalar field load/store iterations and asserts zero. This does not claim globally allocation-free RUN. |
| M3 / RM3 fault identity | Dedicated constant/staging/visibility/lifetime/root/alias/profile/preparation/decode identities are mapped and tested. Stable-code inventory, readonly/root and alias regressions pass; genuine type/null failures retain their identities. |
| L1 / RL1 evidence storage | Raw archives live outside the checkout; scratch is ignored. All 255 historical external-location records independently match size and hash. Original indexes and failed ledgers remain preserved. |
| L2 / RL2 caches | Portable contended-cache test executes successfully: borrow contention falls back to storage/cache miss without panic, and cloning retains state. |
| L3 / RL3 deadlines | Shared stride and explicit boundaries pass helper/instruction clock-count tests, mid-cycle progress/fault/output withholding, final output publication, engineering-write and restart-publication regressions. These are policy tests, not measured MCU interrupt latency. |
| L4 / RL4 single budget | One context-owned allowance covers dispatcher, helpers, initializers and hosted tiers. Nested entry cannot replenish exhausted fuel; core and hosted parity/limit suites pass. |
| L5 / RL5 cleanup | Obsolete construction predicate/dead branches are removed; the still-used clone accounting helper is retained. The trace header no longer carries stale session verification state. |
| L6 / RL6 board boundary | Scope A4 closure is software integration. The specification/checklist reserve physical artifact replay and board measurements for B/E; this report does not close them. |
| L7 / RL7 coverage | Real STBC 2.0 execution passes TOF/TP, CTU/CTD/CTUD, rising/falling triggers, bistables, F_EDGE, hierarchical I/O, mid-cycle deadline and forged dynamic initializer-store rejection. Generic-counter fixtures remain generic. |
| Q1 / RQ1 state structure | EngineState composes lifetime, construction, image and execution-resource groups. Complete core/host suites pass after the ownership changes. |
| Q2 / RQ2 common representation | Named record predicates and one PartialAccess::from_wire decoder replace scattered interpretation; closed-tag/width and runtime access tests pass. |
| Q3 / RQ3 registry ownership | PreparedModule owns the reused registry; allocation-free registry demand is charged before construction. The registration demand test and restart/instantiation tests pass. |
| Q4 / RQ4 facade | Portable mutable internals are private; HIR-only hosted adapters retain compatibility. Negative doctests pin private VM fields and disallow using RuntimeState as raw mutable ReferenceContext. Portable builds and documented public codec APIs pass lint. |
| Q5 / RQ5 review coverage | Full 239-path current identity links initial full review, focused run-6 through run-10 reviews, formatting provenance and this independent final reconciliation. Review and tested bytes are explicitly distinguished. |

## Fixtures and post-batch documentation

program-v2.stbc remains 6,164 bytes with SHA-256
`0a09a7190f26bbe3481ca7ae5b7f16a130ae4dcb2ee21937b456dd8581a87e5a`.
numeric-v2.stbc remains 5,488 bytes with SHA-256
`66f64b2793e7a45988c9ff93bbac5dbf219282fb576f0b3e63103ec9c0a05588`.
The actual saved-artifact and numeric trace assertions pass without compiler/source
input to the fresh core consumer. Generated diagram bytes match the freeze.

After run 11, only the first comment in expected-a4-trace.csv changed to point to
the execution ledger. The tested whole-file hash is
`01dcc96b546bb5ba403f770f769f4abc50f037471273eb8eeead232924348732`;
the current whole-file hash is
`f58fa24d4bb4b0d8fed473b9cbccc1565ac58c698d90178fff55081801245c54`.
All bytes after the first line are identical, including remaining comments, the
header and newlines. Their SHA-256 is
`3294c112aeb7ecea51928a5b0a8648e502f5ae8779181513accf90d09ba6fc09`.
The native trace reader explicitly excludes lines starting with #. This is recorded
as a documentation-only fixture delta, not falsely called whole-file equality.
Other post-batch closeout edits are documentation/evidence only.

Historical failures remain failures; run 11 supplies the complete current success.
No extra tests, builds, validators or formatters were run by this reviewer. No A4
commit, push, exact-SHA release guard, native Windows/macOS execution, firmware link
or physical F401/C6 execution is certified by this report.
