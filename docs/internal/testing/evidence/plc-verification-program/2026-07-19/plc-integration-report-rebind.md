# PLC Integration Report Rebind

- Source commit: `148d8d2787153a948700ab4249cdf383f092c8cc`
- Timestamp: `2026-07-20T02:55:00+02:00`
- Platform: `trust-builder-linux-x86_64`

All 18 report pairs were generated and validated from pristine detached
worktrees. Four workers reduced wall time without sharing a dirty checkout.
Only each command's declared JSON and Markdown outputs were collected.

| Report | JSON SHA-256 | Markdown SHA-256 |
| --- | --- | --- |
| Test-class completeness | `4323dd4dec2e4094be951e5819f7a5573eb7c27851638ace803888a07ee8657f` | `01303c61fd6a4036e6ca36a46640c09ce0d7fcc1226f743a14368bd26087e9b6` |
| Coverage-matrix gaps | `76e8ac03807d63aef82f0e2809ff7ea3b1f94c181729288d61153a61d8801df1` | `ed39df609e628bb292bfd39eed86c174b0d43a52a9354a721a10566f58d15320` |
| Malformed-input coverage | `3564485397e769aa5dc7739f815ff95f7150fc61754170263a05a0ae8dcf4f12` | `d833ffdb91117b9f1a37def2612710ef14381b55cfcb882c5e8d59f4c9ff0962` |
| Unmapped-test debt | `693fc8cbe69ae502908111f686be23aca96424a4e0ea17bc09e50df31eb98c34` | `6a373469ca755df3d8dc1f4551e42d48fcc0bdcc190f2bfb98d795bc3b448d42` |
| Test-refactor assessment | `ed2250f16f6d2517fd0a3b342ac4e23c24a1e0d97c9f81b1285fcd40f731f0ba` | `0cce71ee80e74b11d50343ef7c759ca6008de8fead649dce733d7f25cb16a1e2` |
| Ignored-test inventory | `c3f1fb279db81ea1d03d1260466def3e9609581340ac2d6c8d7abce4c9a0d774` | `3a08c7a8ef5033684362b0be75e0536c68d5be5f8897629c5404fe93931318a9` |
| Phase 5 suite audit | `732b9ac954ca2073fceca0c11d8fd8b9f8dbad4109154a91e7d75a5bcb8c3b07` | `7bca0aebf99223b721f7955b63b6f06174b4295c246155c8f865728ea375acf7` |
| Invariant-seed audit | `d5201a8b962f5caeb620759a3660f4277b8eb6d50d115415932cb5571a21f0d8` | `313ff47a8a6dc433091beed944b69ffe8356acd6fb4abee6f176be4cea2e9fea` |
| Specification completeness | `f464c960e04e8d933e8167b43995a6c1c8fb03c0f4851090ef60ed97299b8a9b` | `9d191a6c5193b35d4bc252b5dc31cd17c536ddc07e4b7270d0f9c9669ab3a140` |
| Requirement/oracle audit | `8df2eb73d1b21ff321f9e68cd34fc4283be15f70fe8dad046d0ecb82bfef97f0` | `b3955af5e6904d37c5314711300f6a2fb3e1f648a29e150818cfe8c1e342d9db` |
| Conformance alignment | `d05c242a3281bfa0a5df041e7b7de0028ee2bd13878893ab2d6b56c2f49a4d75` | `8442f848bb3605c36cf43bd51ab4a463d51e0c734636d622a1fff6e43b482e24` |
| Runtime-anomaly audit | `5e2b7f6000aa75d4106d503cb357aeee4d9936722ce233b950aeb8eacdc40560` | `cf891e56dd51ecfd32104bceff0b1f8826a2091f45916419606ad87114848370` |
| Fuzz-program audit | `8c11e70568ff86a76d4087c6cbe9d89fd9a15eb89cb183af11d51bc453582f1e` | `756d3763abab4f44726d6396c685f9849b77fbc41cb8bf1f14bd2b41ab1a3657` |
| Mutation program | `2fbb0885d3b7edb1aed63b89087fa4afbc444470e0005320e478770776933c85` | `e9dced155bef7796269286e2afd3281470415702297bc25f384ac72681fe93f1` |
| Specification-source audit | `a5f0d98b7b30c444f6b75f9f3ff5bfc4de653c820dcffeee6b994c970c8d60cc` | `bfe11cbffe339d3c76a56eb78db0b641d3741e2d090bacdc1aa9c3845a1d6b56` |
| Phase 12 workflow/UI audit | `43f24e8cfa03b6ccf04711ce1cc99cbcc63adc6d57b78b73c29b68cd39eb634b` | `ee48cc6ce774325c855c61017963f230bfc4f5c80edb73dc7f620851a9618611` |
| Phase 13 release-evidence audit | `748dca6d170bb159093ce0a4b9f5b49574284c180b72d520459c5955bbf84062` | `a821cd559024869f58cd13aa95ed8c6a5b12e6f9c14f080f752ecdabec1f44ad` |
| Phase 11 hardware-lab program | `c417c1c6cab052c1114c8211e806510b0b5345997c49f2653dd2d67c28164916` | `6417e16a6c6b5365e4adefbb21513a6fb283227fcc90e04682bd625529ad2c24` |

The rebind changes no product behavior, suite authorization, CI wiring, proof
level, or board state. It records the refreshed integration census and report
provenance only.
