# Code-Spec Refresh Quarantine Risk Review

## Accepted path-coherent risk batch

Review date: 2026-07-24

Reviewer and owner: `verification`

Accepted implementation paths:

- `scripts/verification/code_spec_refresh.py`
- `scripts/verification/code_spec_refresh_cli.py`
- `scripts/generate_code_spec_refresh_proposals.py`

Authority:

- specification:
  `SPEC_VERIFICATION_PROGRAM_CONTRACT_001#code-to-spec-refresh-proposal-quarantine`
- invariant: `VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001`
- focused gate: `TEST_VERIFICATION_METADATA_GATE_001`

Decision: every scanner fact in the three accepted paths implements the
observation-only proposal quarantine. The batch may be generated as
`specified` against the authority above. The scanner-generated risk signals
are accepted for their exact generated code identities and body digests until
2026-10-24.

The accepted risks describe metadata-tool execution, collection mutation,
filesystem access, subprocess inspection, authorization boundaries, finding
classification, and error/status handling. They do not authorize product
state mutation, semantic inference, accepted-map writes, gap closure, proof
promotion, or bypass of the later reviewed transaction.

The proposal artifact remains an untrusted report observation. This review
does not assert that report facts are live or source-derived. A later review or
acceptance transaction must rerun the canonical live extractors and require an
exact fact, finding, mapping, and input-digest reconstruction before it may
present or apply any candidate.

All code identities, signature/body digests, acknowledgement identities,
record counts, and TOML rows are generator-owned. This review selects the
coherent paths and authority once; it does not hand-author their derived
metadata.

The existing `scripts/verification_metadata_gate.sh` entrypoint remains in its
historical verification gap. Its body and acknowledgement digest may be
mechanically refreshed because the only change is invoking the focused
quarantine self-test; this review does not promote that broader gate to the new
invariant.
