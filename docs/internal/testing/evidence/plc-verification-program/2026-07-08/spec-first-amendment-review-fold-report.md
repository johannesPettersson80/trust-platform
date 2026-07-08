# Spec-First Amendment Review Fold Report

Date: 2026-07-08
Status: required review edits folded
Scope: document/checklist updates only

## Summary

The follow-up review found the spec-first planner amendment faithful but not yet
precise enough to implement. The load-bearing issues were all metadata and
checklist precision gaps: TOML container convention, stringly delta semantics,
producer allowlist, red-to-green pairing, waiver references, and several
undefined Phase 1B mechanisms.

This fold keeps the same design and tightens the record model and board rows.
No product code, tests, CI, or skills were changed.

## Findings Folded

1. TOML container convention
   - Added `TOML Container Convention` to `metadata-model.md`.
   - Invariants are now documented as one record per file under
     `verification/invariants/<area>/<ID>.toml`.
   - Flat registries keep plural wrappers such as `[[spec_sources]]`,
     `[[spec_gaps]]`, and `[[evidence]]`.
   - Added `VERIF-P1-021`.

2. Structured delta semantics
   - Replaced string blobs such as `target_set_to_input_siblings_unchanged` with
     `delta = { target = ..., siblings = ..., retain = ..., process_image = ...,
     diagnostics = ..., status = ... }`.
   - Added closed vocabularies for every v1 delta key.
   - Added `VERIF-P1-022`.

3. Producer allowlist
   - Changed high-risk proof from a `producer = "manual"` blocklist to an
     allowlist: `prove.py vN` or suite-recorded approved gates only.
   - Defined producer vocabulary and clarified that `codex`, `claude`, `fable`,
     and `manual` are provenance values, not high-risk red/green proof
     producers.
   - Added `approved_proof_producers` to suite records.
   - Added `VERIF-P1-023`.

4. Red-to-green pairing
   - Added required `prove.py green` pairing: same test, same case-file digest,
     formerly-red case IDs now green, no previously-green regression, no
     unwaived skips.
   - Added optional evidence fields for `proof_kind`, `failure_kind`,
     `case_artifact_path`, `case_artifact_digest`, `case_file_digest`,
     `paired_red_evidence_ref`, and `per_case_summary`.
   - Updated `VERIF-P1B-008`.

5. Waiver and risk-change mechanism
   - Added `decision_ref` requirement for `not_applicable` coverage cells and
     waived matrix rows.
   - Planner risk-change reports now require `--baseline <rev>` locally or the
     pull-request merge base in CI.
   - Added `VERIF-P1-024`.

6. Error-code stability precondition
   - Added `VERIF-P1B-003A` to inventory stable pilot-surface error identifiers
     before behavior rows pin error codes.
   - The row explicitly allows a small gated product-change exception if stable
     typed identifiers do not exist.

7. Phase 1B mutation dependency
   - Added `VERIF-P1B-013` to pull the bytecode-validator mutation shard forward.
   - Updated `VERIF-P1B-012` to remain open across phases until mutation survivor
     proof exists.
   - Cross-referenced the pulled-forward slice in `VERIF-P10-001`.

8. Enforcing ratchet
   - Added `VERIF-P1B-014` to move from report-only to enforcing mode only after
     burn-in, pilot red/green proof, waiver/risk reports, and an override
     procedure.

9. Case artifact ownership split
   - The helper artifact now owns observed case facts only.
   - `prove.py` owns command, commit, platform, timing, failure-kind
     classification, and evidence creation.
   - Optional `TRUST_VERIFY_*` helper stamps must be verified by `prove.py`.

10. Evidence prove fields
    - Added the optional proof fields listed under finding 4.

11. Naming drift
    - Aligned the decision-table example and case example with the seeded
      `VM_SEAM_SUBRANGE_001` invariant name.

12. Nits and consistency
    - Synced README storage summary with `matrix.toml` and `cases/`.
    - Fixed the self-test fixture punctuation by keeping new fixtures in the
      same bullet list.
    - Reworded taxonomy waivers to require `decision_ref`.

## Files Changed

- `docs/internal/testing/checklists/plc-verification-program/README.md`
- `docs/internal/testing/checklists/plc-verification-program/metadata-model.md`
- `docs/internal/testing/checklists/plc-verification-program/policy.md`
- `docs/internal/testing/checklists/plc-verification-program/test-taxonomy.md`
- `docs/internal/testing/checklists/plc-verification-program/implementation-board.md`
- `verification/README.md`

## Remaining Design Boundary

The fold does not implement the verification system. It only makes the plan
implementable. `VERIF-STOP-012` still blocks updating skills or agent
instructions until the bytecode/VM pilot proves the planner, cases, prover,
report-only CI, enforcing ratchet, and mutation shard.

## Requested Review

Please verify that findings 1 through 12 are folded faithfully and that Phase 1
plus Phase 1B may start without another design round.
