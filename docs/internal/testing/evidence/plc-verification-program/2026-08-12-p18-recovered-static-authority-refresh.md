# Phase 18 recovered static authority refresh

Date: 2026-08-12

Source freeze: `512586ee7f5c6452548342caa00444269739f50e`

## Outcome

The recovered dirty Phase 18 worktree was frozen into an exact detached
candidate without modifying the primary checkout. On the clean
`trust-builder` worktree
`/home/johannes/projects/trust-platform-p18-frozen-512586ee`, the live test
scanner found 8,690 tests and the code-spec report found 17,465 code facts.
The test scan had zero diagnostics. The code-spec report partitioned all 76
identity findings into 15 missing mappings, 15 body-digest rebinds, six stale
identities, and one risk-acknowledgement review group, with zero unclassified
findings.

The reviewed identity transaction transferred all six exact
path/symbol/item-kind successors, accepted all 15 body rebinds after source
review, and registered nine singular-owner facts against their existing
historical gaps. The live-reconstructed compare-and-swap check passed with:

```text
transferred=6 removed=0 rebinds=15 direct=9
code_map_after=sha256:ecf8ab90991f9a3776694ac1a981d00e342013bb2655e50e0f77bf1e848d3c97
```

The transaction closes identity drift only. Direct registrations remain
`spec_gap`; they do not create semantic authority or execution proof.

## Identity and risk review

The accepted proposal and selection are bound by:

- code-spec report:
  `sha256:aea1b5bb9369d212335f098df9e196fad565561862c13ee3b43d12a813ed04c1`;
- refresh proposal:
  `sha256:a8d2c15c2156d2ec3cc2b07d5144fefaad913a01361a1554aa4f3106faa98d19`;
- operation set:
  `sha256:5124720da47a5a0c80c6fdc7339b5cbd10e807cccf4b6db2659361e0c81b1d46`;
- reviewed selection:
  `sha256:8a9bd474d89608d86c4da042890eecb812f601200c38d7c5a5fd0a350123b26f`;
- projected code map:
  `sha256:ecf8ab90991f9a3776694ac1a981d00e342013bb2655e50e0f77bf1e848d3c97`;
- projected risk ledger:
  `sha256:73fd084c82945e573c419d46dbfc4d870916678c25f8bb2cb5a14262c61834fc`;
- projected gap ledger:
  `sha256:0815cea873c4591c8617a0bc271ad6f19604a054d5a437a132da93628fea6c3a`.

Every stale mapping had exactly one live successor. Five stale identities had
an existing owner acknowledgement; those acknowledgements were transferred to
the exact successor body and renewed through 2026-11-12. The sixth successor
had no risk signal and required no acknowledgement. The 15 rebinds preserved
code identity, signature, owner, area, and semantic disposition.

## New test authority

Three scanner facts were absent from the denominator:

- `DISC_476BEA614D8DE1A430DE` and
  `DISC_536ACBF598E9855E95F2` are authorized by
  `SPEC_IEC_STANDARD_FUNCTIONS_001#runtime-clock-sources-time-current-dt`
  and `TRUST_RUNTIME_CLOCK_DISPATCH_001`. Their direct code facts are now
  specified and their catalog mappings close
  `SPEC_GAP_CURRENT_DT_CLOCK_BOUNDARY_001` at the authority boundary.
- `DISC_AAFB7787A203F8DDF706` is authorized by the clarified truST LSP
  configuration contract and
  `LSP_CONFIGURATION_NORMALIZATION_002#behavior:12`. The product contract
  also specifies alias-only lexical precedence as behavior 13; that separate
  behavior remains unproved and must receive its own P18-006 test before the
  invariant can close.

This LSP rule is product-specific and is not an IEC decision or deviation.

## Denominator and route closure

The current denominator is exhaustive over the same frozen scanner input
closure. All 1,967 scanner input files are byte-identical between the clean
source freeze and the primary checkout, so the frozen 8,690-fact scan remains
the exact source fact set for this metadata batch. The closed partition is:

```text
8,690 scanner facts
= 4,457 catalog_mapped
+ 4,233 reviewed_nonmapping
```

The reviewed route projection contains exactly the 4,233 nonmapping
identities, with no duplicates or identity mismatches. The generated execution
plan places those routes in 203 path-level batches and includes the 59 mapped
catalog companions that share those routed paths. The plan does not incorrectly
claim that all 4,457 mapped catalog rows are route companions.

The static closure inputs and outputs are bound by:

- scanner facts:
  `sha256:ec59d0cb311daa93c5128c82c20feab506e8b5a9136580de73a15e71478210fb`;
- catalog:
  `sha256:998dbda67d04273bc196a4e9765e776a7847d674530fa1f215f4eb82f6ce1f73`;
- denominator:
  `sha256:311702c1d6615bd18c5be0910ffefbc802dcfd662d1d5a3cafe115c3a8811335`;
- reviewed route inventory:
  `sha256:7d60448f0a02ba85bd463b37bb1ba890bd9e1566a4fc4587591d35bdf474af4a`;
- authority-route execution plan:
  `sha256:21a0baa6ec6304b336843e05d7e328ac933eb49d815e19b6180ba2398bd725d6`;
- final semantic code map:
  `sha256:decd802bcc500ea4572ed767fecee6c0777ca93aabb29e5d131547ee1175601c`;
- final specification-gap ledger:
  `sha256:bd5be8697ad732cbe4b9ff549b111059f20dba1cc434534b8084c27aa4de3091`.

The plan assertions proved exact scanner-to-denominator identity, disjoint
mapped/nonmapping dispositions, exact nonmapping-to-route identity, pairwise
disjoint plan selections, exact plan inventory coverage, and exact catalog
identity for every routed-path companion.

## Clean candidate validation

The 15-file static authority batch was reproduced in the clean remote-builder
candidate `649643786636d1034297a17f8809f7d8e539b337`, whose tree is
`b67ef2a46f975fcbf63b0ac5e956f22431324745`. Destination `AGENTS.md` and all
25 canonical skill files matched the primary checkout before the overlay.

The fresh candidate-bound code-spec extraction completed with 17,465 live
facts, zero findings, and zero blocking findings. Its report digest is
`sha256:87d4f5c80b2a12cf7a209c64d2324fa923aba6a6a1c68bab51ec4674f2f0cef4`.
Regenerating the denominator route inventory and execution plan inside that
same clean candidate reproduced the route and plan digests above and passed
the complete static identity assertion set.

## Proof boundary

This is specification, catalog, and metadata evidence only. No Rust test,
focused nextest batch, coverage campaign, mutation campaign, broad workspace
test, `just fmt`, `just clippy`, or `just test-all` ran. Focused execution,
bidirectional reachability, mutation sensitivity, and the final P18-016 gate
remain open.
