# A4 run 8 — core assertions and hosted boundary failures

The consolidated batch completed without retries: 19 required steps passed,
8 failed, 2 fixture-dependent steps were unrun, and both advisories passed.
Core all-features executed 313 passing and 2 failing assertions; portable core
273 passing and 2 failing; i686 196 passing and 1 failing. Each first two lanes
filtered one numeric fixture assertion because host fixture generation failed.
Hosted unit/integration assertions did not execute. Both MCU library checks,
architecture, diagrams, formatting and supply chain passed. This is not firmware
link or hardware evidence.

Failures reduce to four owning files: a stale stable-code inventory count,
a stale program-root error expectation, an obsolete hosted sizeof re-export,
and five unnecessary borrowed-slice references in a hosted test helper.
Corrections were prepared only after the complete batch finished.

Frozen source: 598 records, SHA-256
`f008f9f574853ebb3b0c9d8dfc65bf566676c636a4862e72fd38903adf0f362f`.
All 48 raw artifacts were hash-verified after copying from the builder. Resolve
the original index through external-root.json. Logs and source archives remain
outside git. retained-sha256.json covers this record, excluding itself.
No A4 commit, push or hardware run occurred.
