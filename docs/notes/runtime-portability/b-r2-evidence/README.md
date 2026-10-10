# B-R2 evidence

Two authorized size measurements and one consolidated validation batch, 10 October 2026.
The final frozen source manifest is `230f05c6020e90a8d08be922a913b1d03758dfd70c64a9086e78f26c6571847b` (409 paths).
Base: `df427259cc387a7a79fb81132be23e48ea1493d4`, Rust 1.95.0.

M1 upper span 474,400 B (15,648 B over); M2 476,256 B (17,504 B over).
Both produced complete maps but no installable ELF. M2 still needs 33,888 B
removed to meet the 16 KiB upper-region margin. Every PLC function, overflow check
and full admission remains enabled. Raw maps/archives remain outside git under
`/home/johannes/projects/.artifacts/runtime-portability-b/b-r2/`; map hashes and
compact attribution reports are retained here. Placement and actual size reduction
are counted separately.

The final ledger has 37 PASS rows (including two advisory), two failed Clippy rows
and one unrun ELF inspection. The hardware phase is unrun because no ELF fits.
All executed native behavior suites passed; compile-fail doctest diagnostics in
logs are expected passing assertions. These are overlapping suite totals, not
unique-test counts:

| Suite | Passed |
|---|---:|
| core-all-features | 366 |
| core-portable | 326 |
| core-i686 | 246 |
| runtime-unit | 3841 |
| runtime-integration | 538 |
| native-tools | 91 |
| native-bundle | 5 |
| provenance-helper-tests | 4 |

Both audit graphs, architecture, diagrams/drift, cross-target warnings, formatting,
artifact equality and provenance passed. No rerun or board command occurred.
After the batch, root prepared five expression corrections in two Rust files for
Clippy (inclusive-range spelling and four redundant u64 casts). They are separately
reviewed and **unvalidated**; the frozen-source results apply to the pre-correction
bytes. See the execution record for the current-source distinction.

Source reviews and their original/formatted hash identities are retained. Pending
review notes are historical and superseded by accepted reviews. Scripts are copies,
not instructions to rerun. `artifact-sha256.json` covers every retained file except
itself, including this README. No map, binary, source tarball or raw source patch is
included in the tracked evidence.
