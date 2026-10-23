# A4 run 6 — failed compilation, complete ledger retained

This one consolidated batch finished without a retry. Required steps: 17 passed,
10 failed, 2 were unrun because fixture generation failed. Both advisory steps
passed. The failed source is not scope-verified. Core/host native suites and MCU
checks failed during compilation; they provide no executed runtime assertions.
The provenance helper ran four native assertions, and the planned Python mutation
tooling passed. Supply chain, architecture, diagrams/drift, formatting and diff
integrity passed. Metadata validated 1029 records.

The compile failures reduce to five owning corrections: a still-used value-copy
accounting helper accidentally removed with the obsolete whole-state helper; two
RetainedGraph fields incorrectly rewritten as EngineState fields; one unused import;
a missing no_std vec macro import; and a test comparing VmTrap without PartialEq.
These were prepared locally only after this complete batch finished. They remain
unexecuted; the next consolidated batch uses the recorded standing authorization
for reviewed correction cycles.

Frozen source manifest SHA-256: `3a41ebb08ae3990a7754decd366af48632ec904bd59bacb1197f32f7e6be7681`.
`artifact-sha256.json` retains the 48-file raw artifact index; resolve its paths
under either directory in `external-root.json`. Full logs and the post-batch source
archive remain external, with all copies verified against the index. The archive
also includes generated diagrams produced after the source freeze. Small ledgers,
commands and the frozen manifest are retained here. `retained-sha256.json` covers
this README and all other retained files except itself. No A4 commit, push, rerun
or hardware execution occurred.
