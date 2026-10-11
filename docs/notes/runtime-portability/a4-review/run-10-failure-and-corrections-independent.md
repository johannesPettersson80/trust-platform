# Run 10 independent reconciliation and correction review

Canonical instructions and skills still match the primary checkout. Reviewer
introduced no implementation and ran no builds, tests, formatters or validators.

All 50 raw artifact hashes match locally and on the builder. Artifact index:
`74adad677d41bacb6408c32c0903f388dfd276b27c7bbdf1ef315d5910820c4a`.
The 627-record frozen manifest has digest
`f04283f6b39067c84478b3e675572b4fd796b994baa4d2832fc00a4c908e4844`;
archived post-batch bytes match those frozen records. The ledger has 28 required
passes, one required failure and two passing advisories. Recounted core totals
are 316 all-features, 276 portable and 197 i686, all passing; hosted units pass
3,841. Hosted integration has 534 passes and two failures. No hardware claim follows.

Both failures are in source_free_blocks_and_io. Generic counter instantiation
returned TypeMismatch before execution. The hierarchical negative fixture failed
while parsing %IX1.2.99, so it never exercised rejection of an unbound address.

Five-file correction review:

- Shared type policy now accepts only NULL or the eight signed/unsigned IEC integer
  value variants for the internal 0x0100 descriptor. It leaves values and concrete
  widths unchanged, including ULINT extrema, and follows the existing alias path.
  Its integer classifier excludes bit strings, Boolean, real and enumeration values.
  Construction's existing scalar policy already admitted precisely NULL/integers;
  the correction aligns the later shared assignment check with that contract.
- Native tests cover NULL plus minimum/maximum of all eight integer variants through
  the direct descriptor and two alias levels. Negative tests cover other value
  classes, including nonfinite reals, and assert TypeMismatch. alloc::vec/Vec imports
  already support the portable test build.
- Inspection of CTU/CTD/CTUD lifecycle confirms native execution binds an initially
  NULL CV to PV's integer variant, then accepts only matching PV/CV variants. This
  correction does not permit silently changing an already-bound counter's width.
  Initialization checks and engine assignment route through the corrected shared
  policy. The original generic-counter ST fixture and its trace assertions remain
  unchanged; it was not replaced by a concrete typed counter to avoid the failure.
- The unknown hierarchical address is now syntactically valid %IX9.2.3. Its assertion
  specifically requires InvalidIoAddress from the admitted-binding API; the bound
  input/output trace is unchanged. This strengthens the intended negative case.
- Specification 12 section 11.5.4 describes this internal native-state contract and
  the changelog records the generic-counter construction correction. Unsupported
  ordinary generic declarations and legacy 1.x remain excluded.

No suppression, default-limit increase or weakened assertion was introduced.
No additional reachable counter lifecycle defect was found by source inspection.
The five before/after identities are pinned in
run-10-corrections-independent-source.json. This review does not claim the corrected
counter or hierarchical tests have executed; the complete corrected candidate still
needs its authorized consolidated validation batch.
