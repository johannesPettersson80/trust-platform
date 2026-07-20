# LSP document-close specification gap closeout

Date: 2026-07-16

Focused source: `25f967ee66937a50cc219720c5ea9d27dbb788eb`

## Contract and defect

`SPEC_GAP_EDITOR_DOCUMENT_CLOSE_001` asked what durable source replaces an
unsaved buffer after `textDocument/didClose`, which caches must be invalidated,
and how dependent diagnostics are reconciled. The normative answer is now in
`docs/specs/14-lsp.md` under "Document Close and Durable Project Truth".

The five-case hand-authored trace found a real product defect. The readable-file
reload path restored disk content but retained semantic-token and pull-diagnostic
cache entries derived from the discarded unsaved buffer. `prove.py v1` recorded
the single-case red at `fef5ad88cb5a7439e0ccfcd28c3b762091494354`, the
minimal cache-eviction fix at `606666edbb886b0b62f91090ff3129d84d359d3a`,
and a paired five-case green with the same historical execution contract.

## Current contract proof

The invariant and case file were then rebound to the normative
`SPEC_LSP_CONTRACT_001` source. Producer-authentic current-contract lock
baseline and compare rows cover:

- readable tracked-file reload;
- unreadable or non-file removal;
- discarded semantic-token and diagnostic cache invalidation;
- dependent semantic and diagnostic recomputation; and
- push-clear versus pull-recompute protocol behavior.

The independently cataloged existing disk-reload regression test also passes
against the same written section. The gap closes because the lifecycle is
written, both affected tests are mapped, the defect has durable red/green
evidence, and the final normative contract has a stable lock pair.

## Honest posture

`EDIT_DOC_CLOSE_001` reaches G1 only. A causal approved broad-remote proof is
not part of this closeout, so G2 and validated status remain open.

## Boundaries

- No suite, workflow, validator, schema, proof-producer authorization, or CI
  enforcement changed.
- The only product change is the reviewed `didClose` cache invalidation fix.
- This closeout resolves only `SPEC_GAP_EDITOR_DOCUMENT_CLOSE_001`.
