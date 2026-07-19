# PLC Integration Report Rebind

- Source commit: `3803c39a829cf4d771c8b621f48edff5d2500600`
- Timestamp: `2026-07-19T20:10:00+02:00`
- Platform: `trust-builder-linux-x86_64`

All 18 report pairs were generated and validated from pristine detached
worktrees. Four workers reduced wall time without sharing a dirty checkout.
Only each command's declared JSON and Markdown outputs were collected.

| Report | JSON SHA-256 | Markdown SHA-256 |
| --- | --- | --- |
| Test-class completeness | `8d34794c976c24315f614cff6f2c0f4b0f728bfedc9a1b5252d6c40440a61844` | `022c0bf60b5ff91db5856c39911d85de4acf5774ce05a4c6cdd27a09db1032bc` |
| Coverage-matrix gaps | `7a36a15b26a3dd3528cc3fd36bbf69c8713eb65c8537ba94e1d051518585cc9d` | `fb567ea19a3bd56e58336de103c563604b46ada5052e72596666e0b71d771718` |
| Malformed-input coverage | `b89c41a5aae29477186c10969b80cdc38bc5c4357158d16397781bee71045da5` | `cd8685ac61f0f0874655dc09cf27e2c79b1364ca73d2699b78d643fac3fbaf2d` |
| Unmapped-test debt | `6a55cc4d5072036b03c30013879cd4445c42010b0ba48793389164805bf92baa` | `ccb82128412c1cdabeba0d85790cd46d21b4f971ad746085f4c60550c60152c2` |
| Test-refactor assessment | `4cbf53ca1078505399a1ccf7442e8ff8873996194cf3baebe19498aea1724dc8` | `80c6b3fa16a078523d366575a95c90fdabaafefa94a0a3329412fefc9951de8d` |
| Ignored-test inventory | `8efa841760fd18f5403ac852ae608bac9f13ea0c61bdc2a4ed0d0f845f2ef27c` | `137ad1e415dfccfbb29af9daf667ca42b7382e3a25f43df5d15cb012eb93611f` |
| Phase 5 suite audit | `567637805cbd705b6d1d11e6f8d95f3b970a34904e60e3bcef118158afbc91f3` | `8790e0e69f6bc588d3041bb9c8ec66bfe306b0b017098ba393cf1760cae945d4` |
| Invariant-seed audit | `39394d1b741f51c3864e55a675fb1aee200b930bc53f6c097864997e11acddbe` | `8c356cac5b9371eabc7b6933d080f3881028333a1c734411a32ff5138160b0f7` |
| Specification completeness | `d1f3d6afda6bed32728fbac04631e952456c2e982ecae2700b241d1437dd9009` | `efa45d2ddf6ac6b8c8d375b0308836a27424ec4ca477c589ecff9c78f229a3e3` |
| Requirement/oracle audit | `6fd486dc37dae2cd635b3d502f37e4c81d5a942f91290efb8806772ade0b5809` | `ba6323e00594ea8a7437af186a02235203255b86aa84f19781363b69384a4148` |
| Conformance alignment | `668e023e9788781f4607ec1fb5617d70586b86075c6ce3d0cec4e4b8609a9fb1` | `60e2c8dfaf832c2516945cf3782326fb2e53a1dc8d3d85b165f2325ff55903d7` |
| Runtime-anomaly audit | `4e05549615452a9a4a9b69268f63b0a08356c887586abbb1acac5c6dbad77b3d` | `91e046a5db31f261e2a733320925d453cb3c9b69dd69273683dda6388b50e319` |
| Fuzz-program audit | `2fab1755bae3ef4b6d1cffc4b53c1d650892692683b55c9b35858a446c61ea86` | `3054484f326dbc3c783fdedf99a073093f44a0bf8818bfb179f579d76b4a65e2` |
| Mutation program | `ecd1f72a073bb05b6c186d3186a3907098c13f24b66c1fb22c67755184f224d2` | `06c7693154026927fb25a9479148c89873c5a9867a57b8cd1d741a827f065605` |
| Specification-source audit | `f41aeaf02991103a6243bb1aaa80a0956edf8d70564faed44803d9659d654119` | `9596427096feaa519bd57b329153e6090b2d10184d6875800a4236de451a83d7` |
| Phase 12 workflow/UI audit | `f41ffcc20c44cadc3392a6f24161004e6fd317590d3560017d492c036782a5df` | `9160dd044dd032674fffb3ee31a02c44987eb70418b370918251e50c34a24ab5` |
| Phase 13 release-evidence audit | `96d288152a7bcf3dc05d197a0d061c1135fa94f5edbfd4afa5ce53b225d0a5b7` | `6a132b37629112ece358a484f7b416167fbcca838e2cde6fe94b370603a79122` |
| Phase 11 hardware-lab program | `03cdf9357e4a49c2afea163142eac57b1dd11b2124c71a2e86e265c8d1ebd256` | `e05865c42d5ab23eff1c70fd6da212313f9d9788a26ec55f110f2eb6feed7c5c` |

The rebind changes no product behavior, suite authorization, CI wiring, proof
level, or board state. It records the refreshed integration census and report
provenance only.
