# Eighteenth-review report rebind

Date: 2026-07-18

All fifteen generated verification reports were regenerated independently on
`trust-builder` from clean source revision
`7f8451df3c6b6ff39df0531c49d778f20ab8d8bb` with timestamp
`2026-07-18T17:30:00+02:00`. The worktree was reset to that clean revision
before each generator because generation deliberately refuses dirty source
trees. Every generator and every paired at-rest validator exited zero.

## Fail-closed discovery

The first regeneration attempt stopped on a stale OPC UA test identity in the
runtime-anomaly denominator. Recomputing the live Rust census then surfaced
nine previously unreviewed facts introduced by the review fixes. The committed
denominator repair:

- replaces the stale OPC UA identity with the current scanner identity;
- maps the partial-safe-state watchdog regression to the deadline anomaly;
- maps the simulation-clock saturation regression to the timer-duration
  overflow anomaly; and
- records an explicit reviewed-nonmapping rationale for every other new fact.

The repaired partition is exhaustive: 3,229 Rust facts equal 135 explicit
associations plus 3,094 reviewed non-mappings, with zero unreviewed facts. The
runtime-anomaly report contains 19 classes, 135 associations, and zero gap
classes.

## Reproduced artifacts

| Report | JSON SHA-256 | Markdown SHA-256 |
| --- | --- | --- |
| Test-class completeness | `8bc61bd081029f3bd5607273c21ad09c17062d8b377f69f0c52ab08951a8a335` | `bc92bf7c40b2fc349af0295d41a8403fc0103470e6e763589ac58b4a02018dfc` |
| Coverage-matrix gaps | `361e7f02f73ac3e3346187e10ddbffeaa53695279b4bde9d18c399e617f3800c` | `b149299a4176a285a1013391e3054fd6380d9774a1546da0d24ee8cd63e31a9f` |
| Malformed-input coverage | `a90521ceb0d4432feeb204eba9f239971a32ef849753a279e92c4832bcea2668` | `69bb4b4b6eb15c11d91ab0b4de6d5ac739d6908f3a8305b892ea3adce7130efb` |
| Unmapped-test debt | `5aee8882e0f7aeef7b7692ca2a7594dba1a230c0d0d9e7873b2d9455edce84f0` | `4b219e544863167f4562958ef8ed0345d7196ce5deaf6aed928dcbe8b9d05288` |
| Test-refactor assessment | `837e2f14c9a43f5d60290f6d4acfdddde7fb532f6135158ccc3fd032a900c36f` | `48a3fb42053d937ded5b9e9480ccf29008f40308689c4a34771a56ade945b909` |
| Ignored-test inventory | `c2d0bb6bc71d342b0723da13527bbaf94087a63054aa0a06cf981f6969e8f086` | `2f68760dd9523eec17231166d0350161c737eecc13a63c07b57a9f67c62d4345` |
| Invariant-seed audit | `d64fed05858adc88dd938e4ba0fc8ff257e7b03c3fc9cb36226080026f64241c` | `dc06cc831734a70e552eee8bf8fe899014714722ba2bbf46968c9b7826a4c56b` |
| Specification completeness | `55c5ca04852c75f3f07daae28a315d69a3f57870f646a3b00934d34aa783bdf6` | `7249f40fb8d060460ce045504108b1050d987601bdb6c62bfe926f8ed4347b49` |
| Phase 5 suite audit | `c6ee79bf63922dc988018d77baaf74c27f83b0b599f3aec51b20ac30088ae30f` | `0e0a66ff5da98b60b621975fe7407835192f95081390f9cefbe103f43f30a5ae` |
| Requirement/oracle audit | `e7f4fb7b24330e51a9d3b0819a60a3ce4bd18c3c2a53d28535892cc7bcd8ea20` | `e392831fe369fba35af3952cfdacefcff28bbd63ac5b5fbe700e6992b16b7d76` |
| Conformance alignment | `347565109c2dc384d7d994757497f88ceb567dd5a77b3ff989f8213acf7d1c5a` | `bd3097dab9abaf2a467c3194203673b2fba8bd8b95ac59560fe0fd759c939712` |
| Runtime-anomaly audit | `495c4b9495ff0cb6dfe40e8510da59833652661a4400659e93f083cee32bffc1` | `02ff30c6f6d9b016151988abd31ee84a8ae5c9674a9deeed56abdb63ab2bb0b1` |
| Fuzz-program audit | `1226f3d2a1af87493ee12d32b13292ba78c0fc14132856ba3e18e814ff51a180` | `ff2621981fb18426d35cee52de7f7943cc07cf335b402d9de008aab668d79ab7` |
| Mutation program | `8980602dddf8112a115bee56e3f81871a81b215998bc1af0f5f4ab07580964f3` | `8d6fbfe10aeb3bdf48d1bb93ad6bf2958c65f81f460b9038c8536a6ad50dd9f9` |
| Specification-source audit | `03d5d3f083d6b4ec6ce9aaa627c85e7521c8658caf1e65452a79c18e825a103a` | `98b5d79653e2fd7967eeacfc966deb7a1e2c23d8a93704808c2fabe97b9ea633` |

## Boundaries

This refresh changes no suite definition, approved proof producer, workflow,
board row, proof level, or enforcement posture. `VERIF-P16-007` and
`VERIF-P16-008` remain open. The report evidence rows remain `proof_kind =
"none"`; the refresh records current metadata state and does not create product
proof.
