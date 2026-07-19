# PLC Integration Report Rebind

- Source commit: `f3bbc8d0e264c9d27bdf6355a444f4403494cb18`
- Timestamp: `2026-07-19T19:20:00+02:00`
- Platform: `trust-builder-linux-x86_64`

All 18 report pairs were generated and validated from pristine detached
worktrees. Four workers reduced wall time without sharing a dirty checkout.
Only each command's declared JSON and Markdown outputs were collected.

| Report | JSON SHA-256 | Markdown SHA-256 |
| --- | --- | --- |
| Test-class completeness | `6543fbf0f088bd8c2ddff603c2abc395505ddfe7caee3301b367d3ed58376930` | `393f45f73148f165c2656cabaa533238a7dd28ef2c3207ed2209af9aedcae7a6` |
| Coverage-matrix gaps | `5e23a91917b4156fb9836aa5885e4811eac94bda9cb2dfccff6c681b7624245b` | `cb90b7bb949e3f2f3b7f137034136ec5988733f655b19d860f9d34477e6d330d` |
| Malformed-input coverage | `72de19dd72c9f5e44a545fe171fcc7816d7838a80193885d18bb520bd86f5bff` | `d9782a40913a5484a8d4c59b34dbac018ab0069c7410827bb9343cdabe8f3e87` |
| Unmapped-test debt | `d32dee441532bd48f76a3be98cd24d4cca13299b13e08e2c2b835d3e3b64bf4f` | `60fff29b1805bbef78c610e5844db6b6013c3bd38a588f9d838eac6ab7a8fb7e` |
| Test-refactor assessment | `63429b2fe214461842f3ab2ef7ac8b0a242df993210856a8622ac34ddd637111` | `f10f99ce80e684293d65cb9d5ad4740fa2b050624388de3f0b279820b32893ed` |
| Ignored-test inventory | `a68cc10353396e419f93a1b97fc4edabacb2106cadc9ad7565c2a49461fe0409` | `2e0d912b1c20f20b499a5ec854ef15b3044bd7a68f5f7ee8edac909200a893ea` |
| Phase 5 suite audit | `e31b0bfc0b3388617055d6b306be7ed002776e9cb2613e7b2337f05fc895dc05` | `add5430c196359649ac87e9ce6daefdb47c40abfe99092f6c18ee0e8ecb99641` |
| Invariant-seed audit | `fdb5d40d4111703696a9d8370f1335c9bb0afc8c0ce9af30baad0356e756f68e` | `ec4f08e6d6a7d14595e01a73bbaff4bd171a07074ebc893e25d2f4f45af6a6c9` |
| Specification completeness | `c2fd919579ea97edf714b3e9edc60a68d1ee42cb8a634da0f7875f4988209161` | `37b09839b15988d6c81fb04c5f499341afd991dd9527924d6a8b164b4ff0f85b` |
| Requirement/oracle audit | `a7c6147c62e2087b1ddadd88422990094c8245745236b9ee823fa84744ddeb28` | `b314bdda152372e3752f1649f1bf845b918c62e2e3992b272b6eb3bf55440700` |
| Conformance alignment | `ac4e6c314910b2be6c43fd9a5fcd6fe5bd5fcbbdeae5550647a24067d3207d60` | `4b0ae9cca04f9ed03fe1519bc10a39c8a27f824cbecf4c767ac8d7fe749770c3` |
| Runtime-anomaly audit | `913ce4a34184ffcc5a73ceee0d82e00a9f800e6ad6ee1ef90c4c7c064b006c60` | `5c51740cbf119f7d2412a7b99d7ba4a2f1aa17cbcd73c40f9247fba6e13e08ae` |
| Fuzz-program audit | `1bbd75340ebfb2fbb1d16a0a26c8257b241a8b93abb2dd90bf630897e81e6f1a` | `33bf27ad001649aa784354bf961dd2450bd4e079eecea28701edd2a5c1d5bb0d` |
| Mutation program | `e4250d0d9f0aae04a780938e29643ea44e79f59199638145e5ce184b622ec0bc` | `22e9d8b42ff4d6bd568f6bb85d084cebb5f4ae5bed63e4d2ee17af03321a71b5` |
| Specification-source audit | `03f18b7739e8f25d580517c73a7bd1845288f4ff705c98724870c18383ea5356` | `b57c0ab8d067f27e3f54df2ec52bf3665ab2e6f164abe42ae44f1e8e44372e4a` |
| Phase 12 workflow/UI audit | `c00490232e64744b75214d3d986da34fa40f8966d927eaf957b9ee96cfcec3fd` | `e7695a15f47d1b412acd550753250c74d6726c6a537b51b27ce60364775eb521` |
| Phase 13 release-evidence audit | `6842c767c1b2d46de1a419537e97c903a5db8cfec723ee9fcdd9fbbb502e572f` | `182ac6ff568bc20921e6e15486e5573668dbb24079be60629e0f54558f4941a0` |
| Phase 11 hardware-lab program | `0d5fba824336b79047f02c75d18f6f9893cea2c1961f084217864f22873622ac` | `b7be82e483feea8e59b0c45473149a3f25e2b6944db1c7648921bfe54b5a716d` |

The rebind changes no product behavior, suite authorization, CI wiring, proof
level, or board state. It records the refreshed integration census and report
provenance only.
