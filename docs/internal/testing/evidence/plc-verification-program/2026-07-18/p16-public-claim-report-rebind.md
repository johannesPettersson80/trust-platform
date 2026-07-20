# Phase 16 Public-Claim Report Rebind

- Date: 2026-07-18
- Clean source commit: `fac00d48c2030553e6d39fc8af231fe24ae8790a`
- Timestamp: `2026-07-18T12:15:00+02:00`
- Platform: `trust-builder-linux-x86_64`

The public-claim closeout changes specification, invariant, matrix, board, and
verification-tool inputs consumed by the report families. Fifteen report pairs
were regenerated from three isolated clean worktrees at the same source commit.
Each generator was followed by its matching at-rest validator, and every
worktree was clean after its assigned reports completed.

| Report | JSON SHA-256 | Markdown SHA-256 |
| --- | --- | --- |
| `test-class-completeness` | `d927632d98fa0b17e70517e9d27cec2e07c2b7eca13801e0fcbc26a6811a0002` | `58839ae23d63cffa3216afb05dae12b4ab78b0bd77d63d98cf6c47364a801f56` |
| `coverage-matrix-gaps` | `b534c10829f5314df70cbcc07a24d0c65ce0c6d203866ff98494e05c9173b60d` | `64d5c5d0b4e0127ba0e4b2b5d193deeb2566830d26d93fdec50562fced08e26b` |
| `malformed-input-coverage` | `74daa7e1897f839e8f32283e2366827bc8d4dc39ddae584f77eaaa11bac48ccf` | `0dbc16c83aa11c4b5f34d9f99d32dea63cfb5e4dd3f76a05931af665d845b39f` |
| `unmapped-test-debt` | `3dbd561d023fa8862d3f32962c24df378fc87986e8e9d0a01e1e6d3fd0d849dc` | `57156200c08673a13fe93757e67e06faf4e2fc1290689c5f84d144349d7670e2` |
| `test-refactor-assessment` | `9564d141a361ae8eb84daa8f7a24915ce7b326e0089e67a98c8a4e461e5b2a53` | `f346b60d9c150ef36167704ed6c0564b30abcd838dae37bbd1eb4eb0c6a8455e` |
| `ignored-test-inventory` | `441d2d8cbd061e572f81da01ea105f02686b8864c2fb072bea127f7ee1129437` | `34dcc37467b75534d1c305f7e2f329b1266d396568fb302adf07ba0d68d16f84` |
| `invariant-seed-audit` | `a997657d7751cc54bbbe6295e7e38bdabfd0d37f67346c383ddc55cf2745519a` | `b0b176d0c33a44a5235144195177b7eaa0180e37943cd7931b6c051f15021bf9` |
| `spec-completeness` | `b68aab13a3a54bc55155e389079a68be804de93b632b3a60448b052a724327a8` | `d42e24fc4badfd022016f1daae80e1cafc4ec6fd512c7d6b950140d8c2d8d9ef` |
| `phase5-suite-audit` | `1f212ed6672876811c4b90ba5c8b082e8278f3ea2045c282077eded9937549f5` | `4b0d196f3464140d0b642db919de97962fe993c915ba2a29d0fb549f8c832016` |
| `requirement-oracle-audit` | `7cc3611f4b4137888fe5ef0101438b08e58d95d119adb9fc0c38a2edba5fdd91` | `bf829e4c3f9f6f16d98c72aff70ea46341fa69f6fac632100717033e98643857` |
| `conformance-alignment` | `345e5fa44e3b975464ff4e19661ddbb9eaba07a47e0634179da28c3f0261b2d1` | `412f76e1e8d27079034bda093d736bbbcd95101e2fd9196a936148c1a02d731b` |
| `runtime-anomaly-audit` | `109ea22d2bdbac10dcf6cb7e8c63f832390143fe4a3b56dbf0197829a4ec1f30` | `d975af3801462ba6fe1f3a1b11c83ae6c35f190ab8ddfb9eccac673e8d0218c3` |
| `fuzz-program-audit` | `6f491dfcd4e82466fb6e4cb884884aab2ddd020da98842110de885d23a01c627` | `394b2a370f8096ca82747795e7152eb4db59db67bf758999d6f13892f51cf1ed` |
| `mutation-program` | `b7f0296c6cb95e7c4d9a585fadbc6530f5b1c1f84b2a911a3a32178cf8027724` | `2927cd1e1abe9f6fdde01ed7a470773762280cc7bb06bb4efe130022df428bef` |
| `spec-source-audit` | `6aad92467bd6cd9208f4dabe76c5fde0a4c91920aee220f2b57e10048998e553` | `3aa10cad9a0beba7a963517461e3118f5bcd336ce48ffed43c5be24fafb1996a` |

The spec-source audit initially exposed a stale reviewed-topic mapping that
still required four now-closed gaps to be open. The existing scope table and
its regression fixture were corrected in `fac00d48`; the final report records
those two release topics as source-present with no open-gap IDs.

This rebind creates no product proof, changes no runtime behavior, and does not
change CI, suite definitions, or approved proof producers.
