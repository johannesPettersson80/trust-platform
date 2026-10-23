# Independent B-R2 collection/cold-error source review

Verdict: no blocking finding in the 14-file B-R2 diff reviewed against the isolated M1 snapshot.
This is source acceptance, not compilation, test, linked-size or hardware evidence.

Canonical source `/home/johannes/projects/trust-platform` AGENTS and complete skills match
both active B and M1 checkouts. Active branch is `feat/runtime-portability-b`, base
`df427259cc387a7a79fb81132be23e48ea1493d4`; M1 is detached at the same base with the
isolated B-R1 implementation. Reviewed bytes are bound by source-identity.json.

## Findings and checks

- Primitive map/set adapters retain every u32 identity bit and derived numeric order.
  Frame and instance identities remain typed at the adapter boundary. Missing lifetime,
  persistent lifetime, and a live frame remain three distinct states.
- Owned instance snapshot still clones exactly one vector, owns its iteration state,
  and retains insertion order and duplicates. `use<>` prevents accidental lifetime
  capture while promotion/rollback mutates the engine. Removal retains the same current
  owner check and clears storage/templates/initialized/once state as before.
- Retained globals reuse the complete (MemoryLocation, usize) key shape. The key is
  always Global for retained values; journal domains remain unchanged. Checked u32
  conversion is lossless on the supported 32/64-bit targets. Existing conservative
  charging is retained and increased for the widened key before insertion.
- Identity tests cover zero, maximum u32, persistent/untracked/owned states, independent
  rollback snapshots, domain separation, duplicate handling, and actual fixture-backed
  promotion followed by both source and destination retirement.
- Cold boundaries move diagnostic rendering without changing variant selection,
  code or messages. Field-index errors remain before operand pop in dispatch; neither
  cleanup nor success control flow moves. Full long-name and saturated-range error
  assertions cover the factored helper. No no_std import or obvious visibility issue
  found in the reviewed tests; this is not a compiler claim.
- No functions, conversion routes, numeric bounds or overflow checks were removed.

Own B-R2 numeric, sort and trace implementation is deliberately excluded. Earlier B-R1
code authored by this reviewer is not independently re-certified: only the subsequent
B-R2 changes in those files were reviewed here. No builds, tests, formatter, link,
remote sync or repository edits were executed during this review.

Manifest SHA-256: `1449e8e00185ee3c945feb85890ab4b10cd4222f28cc81bfe7ab944c641bfe8a`.
