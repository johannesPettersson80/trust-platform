# Eighteenth-review final report rebind

Date: 2026-07-18

The final report refresh was generated on `trust-builder` from clean source
revision `f514021eef1395b1d6aed0f8a8f77eb67cd7b40a` with timestamp
`2026-07-18T19:20:00+02:00`. Each pair was generated and validated in a fresh
detached worktree; no dirty-source override or destructive reset was used.

This refresh follows the focused-suite discovery that four explicit reviewed
census tripwires still named the pre-batch counts. The four owning tests were
updated to the already regenerated live values and passed together before this
refresh. Scanner and report logic were not changed.

| Report | JSON SHA-256 | Markdown SHA-256 |
| --- | --- | --- |
| Test-class completeness | `8522bcd3e6d6a5a26348ae9e10ba87b7b790e4b7a0317d6467914c8983e31b8b` | `d3e76503fa3bdb2fd9b58c60b328d0f8a6c87a038588892ea2fe2b0495ec22aa` |
| Coverage-matrix gaps | `6acc0eb96716992a85d04955876049aca992e0e6c7f76de8c1969f53388c9b25` | `ed48315f2c8e9b427f3bf7e169a2b44314586eebeb497cd074e1fa655046882e` |
| Malformed-input coverage | `1ce4da77867a173d8ea2d1d9b1188b937f448f278045de60df8f034694b9fcbe` | `14f138f3ee18263908839c9d7ef95b6a9845bd065976fe3a4f41f6bd4ad0d2be` |
| Unmapped-test debt | `8af1ef51756563649ca2e3cb95c4434d1d72cbfd09cf7feeb390cfcc8db5fb59` | `26eb48a36be279a63f6234bb4c1d7ba0a7a7221bddfa22e953223dc3bf15323a` |
| Test-refactor assessment | `da654e3436fd1b4181360da7336b42f749e2b107868d2114f29a2cc8b90783f4` | `55d4f5869c1e451f7254fb6260c183f98b15e96f5c2a0465a48c757ef86f8db7` |
| Ignored-test inventory | `88b06c3f30a049b832a0e6ef1c2471f4fc4a07755f80014c47c096f91d186231` | `57cee7c28e8f4e6c1e6f2154eaf4a8613814dadbb115b11ca456b35d5a0f1a06` |
| Invariant-seed audit | `9bf6b3fddccd82cedb50cfae98e7e76b6dddd5fc7502814705fd2b0586607874` | `bfd6566593fac53ef97bae457e38fd8b3b74d2ae8bc796f1813370826b199c2e` |
| Specification completeness | `c711e375cf99306ea5102475da91b5da32ae016ac1108e662f2a170283ce6824` | `a8bbfe1faa95abb1f99faa659a492356f5080d3d4c365b5dddd8747cb114e7c9` |
| Phase 5 suite audit | `ef0aa1e2361c2e231c20c2acdf8854b7272fc986cac6706d0b7b74d8281940f6` | `798a0733f62517d13d9b04d3050b5badfc65e4b61a57d723fa19cbfa4b36c465` |
| Requirement/oracle audit | `8150532be1d1575486e23e3f028e27f1a5c497bd34991dea6f518fce5789c487` | `304e51d31471b96e7be15b37912c4031ab07935fb8b021fef62794f75319dcca` |
| Conformance alignment | `5b2cf55c1aef73c64c4019fb62c201b9df8dbb33b2d773cf3bd803bce90665cd` | `772d90140217215db4798e8137a56b9373ae3ab0b1a53143da91c1bf3ce43115` |
| Runtime-anomaly audit | `d628e4e9c086f265d16be285b24e7c8a684a780f08ab79600fc4ac2cb18f8140` | `c8675758145ada84e0f319e043c052a25077923d7a9aaeed7c9e0c67875b7bba` |
| Fuzz-program audit | `98e384cd762a3214146d68da1bdb5e9b06c9011b2aa5bfee7a4ca12dbac34849` | `801ae486c768505b8bbece3a3a368abc0156f84d669eae190e04d2aa4c2baf45` |
| Mutation program | `070aaf0d9321063c9456b5c83cb707e29d12afbcb419a2784b53e02ed36425f0` | `f3a67ca0c5c88a4f3c069f1ffa2b4abb363157396008698aa9faa8c2c322d0d5` |
| Specification-source audit | `c18e9b85de7e88f1f3508b6480362715a219c4fdfbc9667538ab7435a2336f7f` | `28f339c58c354f2587088f340bc25010b4e9c2ecfee96c4cfe55f3657c01957d` |

Substantive boundaries remain unchanged: 54 of 54 invariants have eligible
oracles; 19 runtime-anomaly classes have 135 explicit associations and zero
gaps; 21 conformance cases are explicitly linked; all 3,781 unmapped facts are
reviewed; and no suite, workflow, approved proof producer, board row, proof
level, or enforcement setting changed.
