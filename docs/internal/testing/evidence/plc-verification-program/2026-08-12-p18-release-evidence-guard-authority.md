# P18 release-evidence guard authority

Date: 2026-08-12  
Active rows: `VERIF-P18-003`, `VERIF-P18-005`

## Scope

The release manifest, annotated-tag preflight, GitHub workflow/release query,
Latest-marker, and required-asset guards now share one explicit S0 catalog
authority: `REL_RELEASE_EVIDENCE_GUARD_001`. The invariant is intentionally
separate from `REL_VERSION_001`: focused mocked API and manifest tests do not
claim that the public release chain has actually completed.

The catalog-visible wrapper is
`TEST_RELEASE_EVIDENCE_CONTRACT_GUARD_001` at scanner identity
`DISC_AD12BFBD95A4ACF053BE`. Its two case partitions accept one exact release
identity and reject malformed, missing, duplicated, wrong-event, wrong-branch,
wrong-SHA, or otherwise non-exact evidence.

## Cross-area decision

`scripts/release_evidence_contract_gate.sh` remains owned by the
`verification` area because top-level gate scripts are verification tooling.
Its expected results are nevertheless governed by the release-area invariant
and the exact public release-evidence oracle. This cross-area link is accepted
only for the wrapper entrypoint: it executes the release contract suite and
writes the prover-bound case artifact, while the underlying release scripts
remain release-owned. No authority is inferred from path proximity.

## Risk review

The wrapper's current body digest is
`sha256:e3a14c4907f4bf3967bb8938079a806de03d0f3dfde272404f59ffd130f03d14`.
Its environment/filesystem, state-mutation, and subprocess signals are expected:
it reads the four `TRUST_VERIFY_*` stamps, invokes the focused Python suite, and
writes one JSON artifact only after all tests pass. Partial proof environments
fail closed. The wrapper performs no network call itself and cannot mark a
GitHub release as published.

## Test-first evidence

- Expected assertion red:
  `python3 -m unittest scripts.verification.release_evidence_contract_gate_tests`
  failed because the gate omitted
  `TEST_RELEASE_EVIDENCE_CONTRACT_GUARD_001.json`.
- Green after the minimum artifact writer:
  the same command passed 1/1.
- Focused contract command:
  `scripts/release_evidence_contract_gate.sh` passed 50/50 locally and 50/50
  on the isolated `trust-builder` validation checkout.
- Focused metadata suites passed 39/39 for artifact, case-provenance, and gate
  inventory contracts; suite/gate routing passed 36/36; exact slice metadata
  validation reported zero findings.

Canonical agent source was `/home/johannes/projects/trust-platform` on branch
`plc-verification-program`, base HEAD
`18e2173257dbb621704440ec21dbabdcc55020a6`. The isolated remote destination
was `/home/johannes/projects/trust-platform-p18-release-ec413525`, detached
validation SHA `a9240cf8f44dcb2a843ec46dfa5b8548050466f0` before the final metadata
amendment. `AGENTS.md` and the complete `.codex/skills/**` tree matched the
canonical local source hashes before validation.

## Remaining debt

`prove.py lock --baseline` and the complete-path `plan_tests.py` run both stop
at the repository's pre-existing global Phase 18 metadata ledger (2,798 later
findings after slice-local suite references were corrected). No proof record
was minted or bypassed. The new invariant therefore remains honestly `S0` and
`mapped`, with targeted lock proof, reachability, mutation sensitivity, and
broad remote proof listed as missing. `VERIF-P18-003` and `VERIF-P18-005`
remain globally open, and no `just fmt`, `just clippy`, `just test-all`, or
other P18-016 broad candidate gate ran.
