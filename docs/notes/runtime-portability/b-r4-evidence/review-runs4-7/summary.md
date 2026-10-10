# B-R4 runs 4–7: independent evidence reconciliation

Read-only reconciliation from retained ledgers, test outputs, manifests, ELF hashes and board artifacts. Canonical agent files were recopied and verified (21 matches); active checkout is feat/runtime-portability-b at df427259cc387a7a79fb81132be23e48ea1493d4. No source/docs edits, validation runs, link or hardware commands. This report covers runs 4–7 only.

| Run | Recorded allocation/outcome | Upper span / free | Frozen manifest SHA-256 |
| --- | --- | --- | --- |
| 4 | 40 required + 2 advisory PASS; allocated full software batch; no recorded board attempt | 442,112 / 16,640 B | `63138bc047e54224a9ea2edd970cdb323db24d74c4bda12f7c4dddf9fb8f7298` |
| 5 | 12 required + 1 advisory PASS; preparation/link/ELF measurement only | 440,128 / 18,624 B | `2fb19a24c1d344bf3e991b984a422ba7558314f4eb95ea290a0a77ca263da928` |
| 6 | 12 required + 1 advisory PASS; preparation/link/ELF measurement only; phase-split change later reverted | 440,160 / 18,592 B | `6a8823e1b138ec193157ed08c0935da87548b377eddebeb2a7399c93d5f713b2` |
| 7 | 39 required + 2 advisory PASS; board verifier required FAIL (exit 1) | 440,160 / 18,592 B | `e1213d088ec94eb0767cc4ad88c4f402218f06df91caaed29d0f670909aa3aff` |

Each manifest has 1,431 records. Its recomputed hash matches review-approved.txt. Each ELF matches its inspector report; sector 0 has 224 B free throughout. Run 7 measurement-reviewed.txt equals its exact manifest hash. Neither run 5 nor run 6 has a native validation ledger/log or board artifact directory. Do not count their passing links as software or hardware validation.

## Recounted tests

| Allocation | Run 4 | Run 7 |
| --- | ---: | ---: |
| Core all features | 401 (including 4 doctests) | 401 (including 4 doctests) |
| Core no-default features | 362 (including 2 doctests) | 362 (including 2 doctests) |
| i686 musl core, 8 harnesses | 282 | 282 |
| Hosted runtime unit suite | 3,841 | Not rerun |
| Hosted integrations | 542 across 68 binaries | 170 across 20 binaries |
| xtask / platform adapter | 85 / 6 | 85 / 6 |
| Firmware library | 2 | 2 |
| Provenance helper Rust tests | 4 | 4 |
| Mutation tooling / contract Python tests | 16 / 20 | 16 / 20 |

All executed tests above report zero failed and zero ignored. The 20 run-7 integration names are retained in runtime-integration.command.txt and include the runtime vertical. Its actual log has 20 result summaries. Run 7 has no runtime-unit ledger row or output file; 3,841 unit passes remain run-4 evidence. Both completed software batches also pass F401/C6 library checks, all allocated Clippy lanes, cross-target warnings, supply-chain, architecture, rendered diagrams/drift, formatting, diff, pack and artifact parity. Their result.txt values of software_required_failed_or_unrun=0 describe software, not run-7 hardware success.

## Exact source reuse chain

- Run 4 → 5: only dispatch/continuation.rs (start_call boundary) and b-r4-execution.md change among manifest records.
- Run 5 → 6: only engine/mod.rs (shallow state-phase experiment) and b-r4-execution.md change.
- Run 6 → 7: engine/mod.rs is restored exactly to its run-4/run-5 hash; construction/values.rs and engine/initialization/values.rs gain the reviewed metadata/type-entry boundaries; b-r4-execution.md changes.
- Therefore run 4 → 7 contains three core-source changes and the execution note only. Full path/hash pairs are in reconciliation.json. The three core changes are the reviewed function-boundary attributes/comments, not new runtime behavior. Run-4 hosted-unit and wider integration coverage is historical reuse; it is not falsely relabelled as executed on run-7 bytes. Run 7 reruns the entire core feature/32-bit corpus and its explicitly affected hosted allocation.
- Both lockfile hashes are identical across all four manifests. Main and numeric STBC artifacts compare byte-for-byte to HEAD in run-4/run-7 parity logs; GPIO fixture hashes stay fixed. No fixture assertion or admission limit is weakened.

## Run 7 hardware failure

Installed board ELF matches measured ELF f11b73ae4b6138d253df2aeea386c77c4c5e88d6a493b94d581325699e5f7caf. Sector-0 and upper-firmware before/after application-write dumps match; checkpoint before/after install dumps match. UART hash matches its retained pin.

UART records main-prepare stack 10,328 B used / 6,056 free, then main-instantiate 14,668 used / 1,716 free (heap peak 22,408 B). The required 2,048 B headroom is missed by 332 B, and the next record is B1,FAIL,runner,stack-headroom. There are no main-cycle, numeric, GPIO/depth or watchdog-completion records. Capture exit 124 accompanies the stopped/idle failure path; the independent verifier explicitly fails with “insufficient measured stack headroom.” OpenOCD halt output is retained. Hardware acceptance remains failed; successful installation and complete software gates do not close it.

These are scope-specific software checks, not an exact-SHA release guard or proof of native Windows/macOS execution. Evidence file hashes are retained in evidence-identity.json. No claims are made about later run-8 source or results.
