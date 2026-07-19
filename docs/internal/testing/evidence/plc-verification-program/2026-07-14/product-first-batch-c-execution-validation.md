# Product-first Batch C execution validation

Date: 2026-07-14

Validated product source: `c11c1ff3ae1ecc300e707d10655571f5bbed6d9d`

Report source: `adfb33fa1e5edccda220d1524da05689ad2d2351`

## Result

Batch C converted two written editor-safety gaps into nine cataloged focused
tests. The new boundary tests exposed two product defects:

1. a cancelled workspace-diagnostic request stopped collection but returned a
   successful partial or empty report instead of `ContentModified`;
2. the shared LSP line index recognized only LF, so bare CR did not advance the
   line and the CR byte in CRLF was counted as a UTF-16 character.

Workspace diagnostics now recheck their request ticket before, during, and
after collection. The line index now treats LF, CRLF, and bare CR as line
endings and clamps offsets inside a terminator to the preceding line end. The
focused red and green results are recorded in
`workspace-diagnostic-cancellation-fix.md` and
`lsp-position-encoding-fix.md`.

The product checkpoint also contains a behavior-preserving `?` rewrite in the
existing LSP refactor-action path. The strict pre-push clippy gate required
that mechanical change; it did not alter product behavior.

## Test census

The mechanical scanner measured three new Rust facts:

- Rust facts: 3,101 to 3,104;
- all scanner facts: 3,896 to 3,899;
- mapped scanner facts: 93 to 102;
- unmapped scanner facts: 3,803 to 3,797.

The mapped count grew by nine because six existing focused LSP tests and the
three new facts received reviewed catalog identities. The live catalog join
passed with 107 committed catalog records against 3,899 scanner facts. No
scanner, validator, schema, suite, gate, or workflow behavior changed.

## Broad validation

Before the broad run, the `trust-builder` preflight reported 26 GiB available
under `/home/johannes`. Only the generated 60 GiB Batch C Cargo target was
removed. The retry reported 81 GiB available under `/home/johannes` and 5.5
GiB under `/tmp`.

At a clean detached checkout of the validated product source,
`./scripts/prepush_ci_gate.sh` passed all eight stages:

- test-path hygiene;
- IEC conformance log validation;
- formatting;
- strict `-D warnings` clippy;
- `trust-lsp` tests: 162 passed, 10 ignored;
- runtime cross-target warning check;
- runtime mesh/TLS stability: 8 of 8 iterations;
- Windows `trust-lsp` test compilation.

The final broad command then exited zero in 544.68 seconds:

```text
ssh trust-builder 'cd /tmp/trust-platform-batch-c-pos-2 && \
  export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-batch-c-b0768ada" \
  TMPDIR="$HOME/.cache/codex-targets/trust-platform-batch-c-b0768ada/tmp" && \
  just test-all'
```

The first isolated OpenOT attempt could not resolve the required sibling
checkout because `/tmp/open-ot-ref` was absent. That was checkout topology, not
a product-test failure. Pointing the isolated checkout at the existing tracked
sibling `$HOME/projects/open-ot-ref` allowed the unchanged test command to
pass. The temporary link and generated Cargo target were removed afterward;
the validation checkout remained byte-clean.

This is broad validation, not approved broad proof. The ordinary LSP tests do
not emit bound same-run case artifacts, so no proof evidence was created and
both invariants remain at S0.

## Report refresh

All 15 existing report generators were run independently from a pristine
checkout at the report source with timestamp
`2026-07-14T20:11:00+02:00`. The report source differs from the broadly
validated product checkpoint only by two updated test-census assertions. Each
generated pair passed its owning at-rest validator and the checkout returned
clean after every generator.

| Report | JSON SHA-256 |
| --- | --- |
| test-class completeness | `65bcaa99d7a75b32762c2a40220fa355c8ac40a466e97ee1e709fd57be19d1eb` |
| coverage-matrix gaps | `57a30bda316f6c0d80643d8e549ceb3d795f558a60e13be28b22109ceeaea63f` |
| malformed-input coverage | `e1b36b372e242eced84485131b7351960a8a6b35123d6abc3fa1541e35400c84` |
| unmapped-test debt | `09b594c2fa37386622fb9c396c9685d0ce58289558b008df731339a83f3e8dc9` |
| test-refactor assessment | `92c122caea2b7da75fcaaa540a71c3637d0bcf05cde8e0005562d472d0a3daad` |
| ignored-test inventory | `d08b9a818a38b057d33f9e998dcfefe84a6d1a53713adf02eeca34fadde6b1b4` |
| Phase 5 suite audit | `699001bfc9354817fc9f6f6f9ef5d5f3908b170eaa4465820da9446fc55156bf` |
| invariant-seed audit | `0e2b2349c1adbd6cb7bfe891085b33d4d6791d5501e43486093cf723675db181` |
| specification completeness | `f90f1065424de3c6dc311ccf221b4826400b53b66f5564c3609bde8201d1df69` |
| requirement/oracle audit | `bf319c3fc880335de3f02b60322970a13866c4de603b40e2e248fded22429cbb` |
| conformance alignment | `cceb6d4601346f816f53c5fe71283c9f5545369db030feb6636280bac4a3a22b` |
| runtime-anomaly audit | `33d4459102f671658d9941e8c64525fee88d7905d3f71004c26c4f21bf147686` |
| fuzz-program audit | `80e47ff07fb2dba66b55a7f59d9d8349bab93c9a5aa4324028ecc227df090cfd` |
| mutation-survivor report | `b5d8459c68e4d7ee19a171639bca1ceff2020ece5b205c36816c56d8f5d295b9` |
| specification-source audit | `81004846dbd3e53ea6adfa4c89e35f670ba29da0022b13551665df7a98c8ff6c` |

The refreshed reports record 3,899 scanner facts, 102 mapped facts, 3,797
unmapped facts, 32 of 53 invariant specifications still missing, 20 of 53
invariants with eligible oracles, 21 unlinked conformance cases, nine runtime
anomaly test gaps, six fuzz-surface gaps, and zero mutation survivors among the
two measured mutants. These remain visible debt; the refresh creates no proof
or adequacy claim.

## Remaining work

- `EDIT_DIAG_CANCEL_001` and `EDIT_LSP_POS_001` remain `S0/gap_open` because
  ordinary Rust tests are not producer-authentic proof.
- The next product vertical must come from a written unresolved gap and begin
  with the smallest focused missing test that can falsify current behavior.
- Broad Rust gates and the 15-report refresh are deferred until the next
  completed product batch.

## Boundaries

- No validator, schema, board row, lifecycle document, suite, approved proof
  producer, workflow, skill, or agent instruction changed.
- The two written gaps closed only after their focused red tests reproduced and
  their fixes passed.
- No invariant was promoted and no public or release claim was created.
- Runtime behavior was untouched; product changes were limited to the LSP
  cancellation and line-index defects plus the behavior-preserving clippy fix.

## SOLID/KISS/DRY review

- Cancellation remains owned by the existing request-ticket boundary; no
  parallel cancellation state was added.
- All position conversions continue through the one shared line index; the
  line-ending correction was not duplicated across LSP features.
- Tests remain in their owning diagnostic and position modules and reuse the
  production handler/index paths.
