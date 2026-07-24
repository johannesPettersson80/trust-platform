# Test-Catalog Proposal Schema Migration

Date: 2026-07-24

This is a mechanical compatibility repair for the committed, proposal-empty
`verification/test-catalog-proposals.toml`. The demand-refinement change in
commit `1f955d8e` updated the closed schema and generator, but the committed
quarantine document was not regenerated. That made the primary metadata gate
fail before it could inspect current verification state.

The repair preserves the original source commit, trace digest, candidate
report digest, generation timestamp, all 1,712 demand rows, and the empty
proposal queue. It performs only these generated transformations:

- rename demand counters from `tests_unblocked`/`code_facts_unblocked` to the
  observation-only `tests_touching`/`code_facts_touched`;
- rename unmapped-demand counters from
  `tests_affected`/`code_facts_missing` to the same observation vocabulary;
- reconstruct 867 structural demand-ranking rows from the committed demand
  code identities and the byte-exact code map at the recorded source commit.

The original candidate-report digest remains the identity of the source
candidate report. The new structural ranking is a deterministic refinement of
the already committed demand observations; it is not represented as a new
trace or candidate report. No candidate, proposal, review decision, accepted
catalog row, proof state, gap state, or product claim is created or changed.

An exact migration audit confirmed that every root field and every demand row
was byte-semantically preserved except for the two counter-name migrations,
and that every unmapped-demand row was preserved except for its two
counter-name migrations. The rendered TOML round-tripped to the generated
document and passed the current closed proposal schema. The primary metadata
validator then passed all 1,757 committed records locally and on the isolated
`trust-builder` checkout.
