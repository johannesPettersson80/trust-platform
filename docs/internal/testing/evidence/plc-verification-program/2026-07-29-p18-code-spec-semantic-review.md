# Phase 18 code-spec semantic review

Date: 2026-07-29

Accepted metadata commit:
`587bd203aa81b0fd0a5ec5d79a625b97a902542a`

## Outcome

The complete code-spec semantic queue is closed. The initial clean
17,178-fact report contained 306 findings partitioned into 33 review groups:
ten cross-area decisions, 22 invariant-to-test relation groups, and one
test-to-invariant relation group.

The 22 invariant-to-test groups exposed a validator defect rather than 22
product mapping defects. A mapping that listed multiple invariants and
multiple tests was being validated as a Cartesian product. The accepted
metadata contract is bipartite: every listed test must link at least one
listed invariant, and every listed invariant must link at least one listed
test. It does not require every test to link every invariant. Correcting that
contract removed all 22 false review groups.

The remaining eleven groups received explicit semantic decisions:

- all ten cross-area groups were accepted because the mapped invariant's
  claim and source references cover the affected code paths;
- `TEST_RUNTIME_HARNESS_ERROR_CLASSIFICATION_001` was removed from the two
  restart-lifecycle mappings because it proves harness transport and boundary
  I/O error classification, not restart-mode parsing or restart execution.

At accepted commit
`587bd203aa81b0fd0a5ec5d79a625b97a902542a`, the clean detached builder
report contains 17,192 live facts, 17,192 mapped facts, and zero findings.
The post-review refresh proposal is empty.

This closes the code-spec identity and semantic-review queues only. The
accepted report still contains 15,506 `spec_gap` facts across 173 gap-open
coverage cells. It does not establish specification authority, test
execution, reachability, mutation sensitivity, adequacy, environment proof,
or final broad-gate proof.

## Semantic review

The cross-area decisions are anchored here so that every accepted code
mapping has one durable review reference. The review compared each affected
fact with the invariant claim, owner, and exact source references; it did not
infer specification or proof from path proximity.

| Review group | Invariant | Facts | Accepted source boundary |
| --- | --- | ---: | --- |
| `CODEMAP_GROUP_92C901C265CEEA67B789CB158B793CC29EC9DF118FBDCA955833C64B8379D0EF` | `RT_ADS_ONBOARDING_IDENTITY_001` | 16 | `crates/trust-runtime/src/host/ads/onboarding/identity.rs` |
| `CODEMAP_GROUP_BF48E2EB422ED6D4B14FB51294D3EC875AA74CA5D3ADB3665549FC3D60705FCC` | `RT_ADS_SYMBOL_IMPORT_APPLY_001` | 1 | ADS onboarding import and control import-handler modules |
| `CODEMAP_GROUP_281869A12B7076D58BB4221B713FAD47D72091013F007F44B4586A4D2D47DA86` | `RT_CONFIG_BUNDLE_LOAD_001` | 11 | `crates/trust-runtime/src/config/loaders.rs` |
| `CODEMAP_GROUP_5790E9CBC289B9107D94CE4986AAFCCA24F6ECFC8606351EF4D59DBCE1F119B8` | `RT_CONFIG_SCHEMA_001` | 84 | configuration contracts, parser, and runtime/I/O validation modules |
| `CODEMAP_GROUP_A2D4561EAF79FB0055F38025DA14500A79983E4FFF2323BDB6325E5A2E60C65C` | `RT_CONFIG_VALUE_BOUNDS_001` | 4 | runtime ADS, OPC UA, base, and I/O configuration validation modules |
| `CODEMAP_GROUP_AA42F163B63339B62F2F003242FAA41333772D8ECDC89C8EEDD76117BC693A1D` | `RT_RUNTIME_CLOUD_ACTION_POLICY_001` | 21 | runtime-cloud routing and profile-policy modules |
| `CODEMAP_GROUP_7469C3B2B6A6D3AA9CE7946BB140FD015A903E3A935412FE76AFA2EC83464C22` | `RT_RUNTIME_CLOUD_CONFIG_RECONCILE_001` | 12 | runtime-cloud configuration policy and web-state reconciliation modules |
| `CODEMAP_GROUP_24BF20781843E7CD40615E26541189D69B089F7544866F194B4B1DE662C2E533` | `RT_RUNTIME_CLOUD_PROXY_PLAN_001` | 9 | runtime-cloud control and I/O proxy-policy modules |
| `CODEMAP_GROUP_B2216EF33372140326A7522969CFF6D2412317BF5622D9C4CE14D8DEAC4C62D7` | `RT_RUNTIME_CLOUD_SCHEMA_KEYSPACE_001` | 50 | runtime-cloud contracts and keyspace modules |
| `CODEMAP_GROUP_A2132FECB21EBF6E370FBFE6D7E2FCC9F89CD925319EA413AE5DCEEC11E03B3F` | `RT_RUNTIME_CLOUD_TOPOLOGY_LINK_POLICY_001` | 49 | runtime-cloud projection/link policy plus web link-route and state modules |

All ten invariants remain owned by `trust-runtime`. The accepted
`cross_area_decision_ref` records only that the invariant legitimately spans
the affected scanner areas. It does not change the invariant, catalog,
execution, or adequacy status.

The one real relation defect was
`CODEMAP_GROUP_D45A605472D646D7E23258D713173F126C1FFF64EDC7865BED5EAA670119A48B`.
The reviewed transaction removed
`TEST_RUNTIME_HARNESS_ERROR_CLASSIFICATION_001` only from:

- `CODE_A01D302C9CA3D309014090A9A57CF6502E9234E54798223CD3A80A50E2DA1D73`
  (`parse_restart_mode`);
- `CODE_C86DB27D2CD2A809409D453CC186C81612243B06F7922AA3E9B2DD56B78E4C66`
  (`handle_restart`).

The test remains cataloged and mapped to
`RT_HARNESS_TRANSPORT_PROTOCOL_001` and `RT_HARNESS_BOUNDARY_IO_001`, whose
claims and source references cover the harness error boundary.

The complete decision set and all report, proposal, selection, application,
and metadata digests are frozen in
`verification/p18/review-locks/code-spec-semantic-review-lock.json`.

## Contract correction and test-first proof

The validator correction and semantic acceptance tool were developed through
focused red-green slices:

1. `test_multi_invariant_mapping_requires_bipartite_not_cartesian_links`
   failed on cross-pair mismatches, then passed after the validator enforced
   the bipartite proof-set contract.
2. The semantic contract tests initially failed because the projection and
   selection contracts did not exist, then passed after those boundaries were
   implemented.
3. The functional acceptance tests exposed the unsupported
   `cross_area_decision_ref` projection and passed after the field was added
   to canonical rendering.
4. The metadata-gate registration test failed while the semantic suite was
   absent from the real gate, then passed after registration.

The accepted focused suite:

```text
python3 -m unittest \
  scripts.verification.code_spec_refresh_tests \
  scripts.verification.code_spec_refresh_selection_tests \
  scripts.verification.code_spec_risk_tests \
  scripts.verification.code_spec_semantic_contract_tests \
  scripts.verification.code_spec_semantic_selection_tests \
  scripts.verification.metadata_validator.code_spec_tests \
  scripts.verification.spec_gap_closure_tests
```

passed 70/70 on the clean detached builder checkout.

The semantic tool requires an empty identity queue, zero unclassified
findings, and one complete decision for every semantic review group. It
reconstructs the live proposal twice, projects a recoverable replacement,
and applies it with compare-and-swap protection. Cross-area acceptance adds
only the review reference. A test-relation decision can remove only an
unrelated mapped test; it cannot invent catalog links, invariant links, or
proof.

## Bound transactions

The bipartite-contract transition is bound by:

- source report:
  `sha256:049261a8af4aea624a9e22058f0c6e3c507a6321a2668d3fce562f15622f533d`;
- source proposal:
  `sha256:736426f0f92d5af10784e7e6819ba512e760497345c449251458bf207b3b9317`;
- operation set:
  `sha256:512c6e748c3a0a62432dfe7e54d61be98ddbc121f036f6b01a30996b098674ab`;
- identity selection:
  `sha256:41701c5572fc82d2c4a81a883172f4e3f6b055cd61b56e1b3ceffe344f7dd5cf`;
- identity application:
  `sha256:6f3cf9e36a06124c5670ca24e8bfe73173bb209f3da80e7f9876b85b793eddaf`.

Adding the semantic acceptance tool created 14 new singular verification
facts and one exact rebind. Their identity transition is bound by:

- source report:
  `sha256:4eadb43a9b45cfba21efd25fd50310ebbfe9b2134773888094f1306d8dc9e3f5`;
- source proposal:
  `sha256:5742dbbd5f794e4841bf32ee6d29e50bd46e4a824e38b2237aa9fb35e659ef8b`;
- operation set:
  `sha256:633462b56bbdd713967598e334c42363570fe8879e8a49eb31fb6873336afce7`;
- identity selection:
  `sha256:24683ff44079bac749551a27049558dabda6f1858d3967345a05781137bbe649`;
- identity application:
  `sha256:3f7d157aaf8fdf81c13778fb8a1b5d04895d37839b97e04686d362f9851a0bb1`.

The semantic review itself is bound by:

- source report:
  `sha256:a74dcf4627aea66e85efe6c0bd9d7b7f060193e0fd89db0736d69a0f9c14efee`;
- source-input digest:
  `sha256:53067d7208751d26f004fc45e85834b74226efd192605e91bf57ec83f963b3c7`;
- source proposal:
  `sha256:1faa3995e685fe923e9119c22fb81ca8110c0ea7a09ca7557597d32bbb57eb14`;
- operation set:
  `sha256:28522086214620ad04d6587f749f5e916a2a9c0ba574c856c9e7e168b295b365`;
- semantic selection:
  `sha256:ba17643ddcd9189ab2a6f85ca4055c62750e3dd4951ab307a1dc1aa3ae9a28f4`;
- semantic application:
  `sha256:28717cab52e48903d51b3a2d901038bcd2a7a86a1c9bfd4a3a679b0c51b49f71`.

The canonical code map moved from
`sha256:475bcbe52ba7c4a4811b16e1cd40e8d8ae20af0145f5b79934f2bcab893619c5`
to
`sha256:f41a1d56a209152907911af3e6119f84bbe7ae7c37475d0e0c6321be836dca55`.
The transaction affected 255 code mappings and removed one unrelated mapped
test relation.

## Accepted remote proof

Canonical checkout:
`/home/johannes/projects/trust-platform`

Clean validation checkout:
`trust-builder:/home/johannes/projects/trust-platform-p18-marker-0d31ff12`

Bootstrap:

- remote host: `scena-rust-builder`;
- destination HEAD:
  `587bd203aa81b0fd0a5ec5d79a625b97a902542a`;
- root `AGENTS.md`: exact match to the remote primary checkout;
- `.codex/skills/**`: 25/25 files, exact recursive match.

The complete focused suite passed 70/70. The accepted report is
`sha256:aea3576fccf5d04e93b255a52f89e67d9abb6c7ed6f775372937a1868052ddd6`
with input digest
`sha256:51be0f70e0f0a9b058b58d3e7d9c9fc7758bad55bb0c72179be424fa685c4bab`.
Its dispositions are 15,506 `spec_gap`, 1,651 `specified`, 30
`inherits_spec`, two `generated`, two `non_behavioral`, and one
`vendored_external`.

The empty post-review proposal is
`sha256:8b2d2928c0d19d6b07fb9cd2ad980140209b80e466f271de3eb49f43503daa6d`
with operation-set digest
`sha256:9c029f348397cd6f8be5c8b9c925de27b5584f9039f2d3e6a67327505d84fdd4`.
All identity queues, semantic groups, and unclassified findings are zero.

The metadata validator retained the exact accepted downstream-debt baseline:
1,610 findings,
`sha256:cdb6cbc9d85f22f8cf593d99ea8ed47cd7e69147f684064ba4953068bb6c1cbb`.
Every complete changed-path planner run exited 6 on the same known global
Phase 18 metadata debt of 1,877 findings, with output digest
`sha256:e355e9ff2e0344f2f960c772b486ba32670a135ae063ea49029605d6cdac7f55`.

No `just fmt`, `just clippy`, `just test`, `just test-all`, coverage campaign,
mutation campaign, or final broad gate ran.
