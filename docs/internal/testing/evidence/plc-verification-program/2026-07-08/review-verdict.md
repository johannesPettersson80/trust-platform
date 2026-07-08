# External Review Verdict: truST PLC Verification Program

- Reviewer: Fable (Claude Fable 5), in-session review.
- Date: 2026-07-08.
- Input: the seven-document set under
  `docs/internal/testing/checklists/plc-verification-program/` (pre-fold state
  of 2026-07-08 ~02:00), fact-checked against the repo: referenced test
  targets, gate scripts, CI workflows, crate layout, and gitignore behavior.
- Verdict: `clear-with-edits`.
- Gate: `VERIF-REVIEW-004` cleared 2026-07-08 after fold verification
  (`fold-verification.md`).

## Verdict Summary

The program architecture is sound: specification inventory before test
inventory, stop gates that target real failure modes (parity-as-oracle,
ignored-test hiding, invented-behavior tests, source-only safety proof), a
malformed-input taxonomy that encodes this repo's actual bug history, and the
correct first pilot (bytecode/VM seam). Every referenced test target and gate
script was verified to exist. Fifteen findings were raised; seven blocked
Phase 1. All fifteen are folded and verified; six residual details were fixed
by the reviewer during fold verification.

## Findings and Dispositions

Severity: C = critical/blocking, H = high, M = medium, L = low.

| ID | Sev | Finding | Disposition |
| --- | --- | --- | --- |
| V-01 | C | Evidence root was gitignored (`.gitignore:138` matched the planned root); "durable evidence" undefined, so VERIF-STOP-007 had a loophole | Folded: `.gitignore` negations for the program evidence tree; `VERIF-STOP-015`; durable-evidence definition + `git check-ignore` fail-closed rule in `metadata-model.md`; `VERIF-P1-013` |
| V-02 | C | Pilot scope named only `crates/trust-runtime/src/{bytecode,runtime/vm}/**`, missing `crates/trust-runtime-core/src/{bytecode,vm,value}/**` (core/host split; core has in-source tests only) | Folded: README pilot paths, seam-area Owns list, matrix rows 18-19, scanner note after the matrix |
| V-03 | C | `metadata-model.md` examples violated their own rules: spec-source `status = "draft"` vs status vocabulary; common fields (`area`, timestamp) missing from 4 of 7 shapes; no coverage field on the invariant record; `""` sentinels; fictional oracle path `docs/internal/architecture/bytecode-format-contract.md` shadowing the real `docs/specs/12-bytecode.md` | Folded: dedicated `source_status`, `last_reviewed` everywhere, `[[coverage.cells]]`, empty-string ban, real spec path + `VERIF-P1A-001`. Residual fixed by reviewer: TOML table-scoping bug in the invariant example (top-level keys after `[oracle]`/`[[coverage.cells]]` would parse as nested); keys reordered and a scoping note added |
| V-04 | C | `validated` had no enforced preconditions; a hand-edited `validated` with empty tests/evidence would pass the validator as specified | Folded: status/proof consistency rules in the fail-closed list; `VERIF-P1-017`; `VERIF-P6A-002A` fixtures. Residual fixed: `wrong_result` added to the coverage-cell blocking rules (`metadata-model.md`, `test-taxonomy.md`, `VERIF-P6-007`) |
| V-05 | C | No evidence record type; `evidence_refs` were unvalidatable free strings while evidence is chain link 5 of 6 | Folded: `evidence.schema.json`, `evidence-index.toml`, Evidence Record shape, `VERIF-P1-010`. Residual fixed: `dirty:<base-commit>` marker format; `lab_report` device-identity fields; `committed_file` git-tracked rule; `ci_artifact` retention note |
| V-06 | C | Public claims had no committed record type, leaving reverse traceability without a durable root set | Folded: public claims are spec-source records with `authority = "public_claim"` plus `claim_text`/`surface_ref`; barred as safety-critical oracles |
| V-07 | C | `crates/trust-debug` (DAP) and the write/force/release lifecycle appeared nowhere: no matrix row, no area owner, no seeds, despite shipped `trust-lsp.debug.io.write|force|release` | Folded: matrix row (Debug adapter/force lifecycle), runtime-safety + editor-area ownership, seeds `RT_SAFE_FORCE_001`, `DEBUG_AUTH_001`, `DEBUG_PAUSE_001`, `VERIF-P1A-004` |
| V-08 | H | Confirmed findings from the 2026-07-04/05 reviews were not imported as seeds: TP/TOF ET timer semantics, NaN/Inf ingress, runtime authz (IDE viewer file-read, Viewer-role control), OPC UA session-per-poll, online-change consistency | Folded: `VERIF-P4-000` import row; seeds `IEC_TIMER_001`, `RT_SAFE_NAN_001`, `SEC_AUTHZ_001`, `PROTO_OPCUA_001`, `RT_RELOAD_001`; NaN/Inf classes in protocol and API payload taxonomies |
| V-09 | M | Catalog records required `invariants` before the invariant registry phase existed | Folded: empty `invariants` legal only for `planned`/`gap_open` |
| V-10 | M | Stale-test detection was file-path level; a renamed test function kept its row as proof | Folded: test-name-level rule + `VERIF-P2-005A` |
| V-11 | M | Existing CI workflows and ~27 gate scripts were not absorbed into suite tiers; veryquick had no environment decision; Phase 11 ignored the existing device-in-loop harness | Folded: `VERIF-P5-000`, `VERIF-P5-000A`, `VERIF-P5-000B` |
| V-12 | M | Fault taxonomy omitted clock/time faults (a known bug class: warm-restart time reset) and allocation failure/OOM policy | Folded: `VERIF-P8-001` extended; `VERIF-P8-001A` spec-gap candidates. Residual fixed: `time_or_clock_fault` coverage dimension |
| V-13 | M | Grace periods invoked but never defined | Folded: `VERIF-P14-000`. Residual fixed: `VERIF-P3-006`/`P6-007` now cross-reference it |
| V-14 | M | `ux_accepted` (acceptance board) vs machine statuses had no mapping | Folded: `VERIF-P12-008`; `ui_journey_acceptance` test-class rule |
| V-15 | L | Polish batch: `resolution_status` vocabulary, owner aliases, `wrong_result` risk value, fold-verification step, AGENTS.md draft pointer, `includes` semantics, trust-plcopen/trust-dev ownership, coverage-dimension templates | Folded: resolution vocabulary, `VERIF-P14-000A/B/C`, `wrong_result` risk, `VERIF-P0-008`, AGENTS.md "Draft PLC Verification Program" section, new "PLCopen Import and Developer Tooling" area with seeds `PLCO_IMPORT_001`, `DEV_TEST_DISCOVERY_001`, `DEV_COMMIT_SCOPE_001` |

## Decisions Recorded (VERIF-REVIEW-003)

Disputed recommendations: none.

1. Public claims are represented as spec-source records with
   `authority = "public_claim"` plus `claim_text` and `surface_ref`; no
   separate claims register. Matches the review recommendation.
2. Spec sources carry a dedicated `source_status` field separate from the
   generic record `status`. Cleaner than the review proposal (a replacement
   status vocabulary); accepted.
3. Coverage is encoded as `[[coverage.cells]]` entries with `dimension`,
   `state`, and `rationale` rather than a flat table. Richer than the review
   proposal; accepted.
4. veryquick environment/recipe mapping is deferred to `VERIF-P5-000A` as a
   board row rather than fixed in `policy.md` now; accepted.
5. `IEC_TIMER_001` is seeded under Compiler and IEC although execution tests
   live in `trust-runtime` (`iec_timers.rs`, `fb_timers*.rs`); the area tag may
   be refined when the invariant record is authored.

## Unresolved Questions

None blocking. Deferred by design to their owning rows: veryquick command
mapping (`VERIF-P5-000A`), grace-period values (`VERIF-P14-000`), owner alias
resolution (`VERIF-P14-000A`), suite composition semantics (`VERIF-P14-000B`).
