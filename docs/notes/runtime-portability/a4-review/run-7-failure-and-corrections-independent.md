# Run 7 independent failure reconciliation and correction review

Read-only reviewer; no implementation authored. Canonical AGENTS/CLAUDE and all
skills manually copied from primary and parity checked. A4 base remains
77b91381fcf1b1850611f88b397bbfe8d45e3523. No tests, builds, formatters, validators,
commits, pushes or reruns launched by this review.

## Evidence

All 48 indexed raw artifacts match locally and on the builder. Artifact index:
`8e54fc96d85a3bbae844f08bb361686c4cda68ff138ad051d257344c0a8ede78`.
The 584-record frozen manifest digest is
`fcf05bff2ed1c9fa0cec6b3308f3f1ad752bf81ebc9a9767459f60a426acd5c6`.
The archived post-batch source matches every frozen file record, including the
unchanged generated diagrams. Required ledger: 17 PASS, 10 compiler/warning FAIL,
2 fixture-prerequisite UNRUN; two advisories PASS. Only provenance helper (four)
and Python tooling (16 plus 20) assertions ran successfully. No core/host runtime
behavior assertions ran; MCU compilation failed. Run 6 compiler defects no longer
appear, but this is not a successful behavior or platform qualification.

## Corrections reviewed against the frozen source

Eight Rust files differ from the failed candidate; exact hashes are in
`run-7-corrections-independent-source.json`.

- `vm/budget.rs`: remaining-fuel diagnostics are compiled for HIR adapters or unit
  tests, preserving every native core assertion without unused production API.
- `vm/call/context.rs`: RegisterReadClone/RegisterReadMove profiling variants are
  HIR-only; all actual constructors/matches for these two events are in host tiers.
  Shared VM events remain portable.
- `vm/mod.rs`: debug-map module remains enabled for HIR or unit tests. Its portable
  tests exercise all source-location fields and already import alloc::vec.
- `vm/module.rs`: only stored hosted function-name/debug indexes and their imports/
  materialization are HIR-gated. The temporary function-name map remains available
  in every profile to resolve native function descriptors. ref_types remains
  unconditional. Legacy constructors/accessors already live behind HIR.
- `vm/prepared/budget.rs`: derived debugger/symbol-index demand is charged only when
  those indexes are actually built. Reference type mapping and temporary function
  resolution still consume preparation allowance in portable builds. There is no
  budget increase or weakening of admission.
- `engine/initialization/construction.rs`: remove the unused former scan predicate;
  indexed callers already replaced it, and no remaining call site references it.
- `vm/errors.rs`: move the native payload documentation to InvalidNativeCall,
  leaving the separate call-kind description on InvalidNativeCallKind.
- `vm/dispatch_refs.rs`: the review identified an additional explicit Clippy error
  omitted from the initial seven-file repair set. ValueRefView.path already borrows
  a slice; pass it directly instead of creating &&[RefSegment]. This is now fixed.

All distinct error classes in the preserved logs are accounted for. No suppressions,
weakened assertions or behavior changes are used to silence dead code: the feature
boundary matches the owning consumers, and standalone portable unit coverage stays
compiled. No remaining source-level blocker found in this focused correction review.

The complete workspace still has not compiled or executed its runtime suites for
this correction candidate. Host/downstream failures hidden by the earlier core
errors cannot be ruled out by source inspection. The next authorized consolidated
batch must provide that evidence; this review does not claim it in advance.
