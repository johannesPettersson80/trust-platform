# A4 run 2 evidence

Run 2 completed with required failures on 10 October 2026. See `ledger.tsv` and
individual command logs. No host runtime assertions executed: compilation failed.
Core all-features: 297 passed, one failed, one filtered; no-default: 259 passed,
one failed, one filtered; i686: 181 passed. Both MCU core library checks passed;
there is no firmware link or hardware evidence. The failed core case had an
incomplete positive admission fixture. Numeric artifact generation failed before
execution. Architecture, diagram render/drift, supply chain and metadata passed.

The batch was paused during compilation while another session finished its full
runtime tests, then resumed without restart. `resource-pause.txt` records this.
Elapsed time is not performance evidence.

The 303-record formatted source manifest SHA-256 is
`2a87b88edd0e38f58291339719d46cc81776bed7ea70db55126a66c96984d4c0`.
Every manifest record matched the builder after completion. `source-snapshot.tar.gz`
retains that changed source, excluding prior retained evidence to avoid recursive
archives; exclusions retain their original hashes in `snapshot-exclusions.json`.
The Git metadata-only index is intentionally excluded. The artifact index covers
this README and every retained file except the index itself.
