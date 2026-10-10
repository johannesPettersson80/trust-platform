# A4 run 9 — hosted compatibility test import

The complete batch finished: 26 required steps passed, three failed, and both
advisories passed. Core all-features: 316 passed; portable: 276; i686: 197;
hosted unit: 3,841. No assertions were filtered or ignored in these suites.
Both MCU library checks and fixture generation/parity passed. Integration tests,
Clippy and the cross-warning gate failed on one integration test importing
ensure_global_call_depth through the newly private VM root instead of its public
hosted compatibility facade. Integration assertions did not execute.

Frozen source: 612 records, SHA-256
`0b37c8fbaacd6f101e916f793cc549f3c211b8beebd2062a04a44a72eaf162f2`.
All 50 raw artifacts match their hashes locally and on the builder. Resolve the
raw index through external-root.json. The post-batch archive contains the complete
tracked/untracked source, including files unchanged from the base. Retained hashes
include this README, excluding the hash index itself.
No A4 commit, push, firmware link or hardware execution occurred.
