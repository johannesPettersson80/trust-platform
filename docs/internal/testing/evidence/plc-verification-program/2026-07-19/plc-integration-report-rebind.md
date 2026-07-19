# PLC Integration Report Rebind

- Source commit: `93e644975a9e29063da4461b95871f41774fde59`
- Timestamp: `2026-07-19T22:50:00+02:00`
- Platform: `trust-builder-linux-x86_64`

All 18 report pairs were generated and validated from pristine detached
worktrees. Four workers reduced wall time without sharing a dirty checkout.
Only each command's declared JSON and Markdown outputs were collected.

| Report | JSON SHA-256 | Markdown SHA-256 |
| --- | --- | --- |
| Test-class completeness | `c12a8600acab67f0668c1a41127d7cd6c6285b8d605faf538404b8ee52c17047` | `8c88f848cf64b173f268767f05465743ac498c5e47e6f6266563f473cb1c2d81` |
| Coverage-matrix gaps | `24aaa1660ddc1e528b0e575d405d3ed3ed504735faa00dc05fc2be37b724fa7b` | `ef1eaf9855f953bc9cc4c8756ef8d4613b04d4f73aa54a3ba55dd0f3c6989ed9` |
| Malformed-input coverage | `0deb3698b82d720056ea206b2bafb9212c31480d4b750002c33d006725baa868` | `28a5a0dfb4b1b3e967cfa6de44c74975427f7b56c16c4f2bacc547fa396302d8` |
| Unmapped-test debt | `a3cd90f00db7ba81e28507013c87d39fc28b60208e88661c77563ab6044744c8` | `9697dfb46bd7b886c99d122c73cafc3ce6ca29c960bb3ccf112ccf87ecc06a75` |
| Test-refactor assessment | `fbd67cdb624cdc9f3ef97ceef64896255914156245a9a15ad0c936f72e9bcd95` | `439705101722a6d26ac3d700c6cb5acf4e0520e8ba3bc880a479cb9dd3699861` |
| Ignored-test inventory | `18d18019598b0686eb77c1dc9f928727dc2786226373d07feba7b1f78dc30916` | `35a271d57c40ceb607c398b5a15d9955428e04688034bdb82fbf4bf45cde0f29` |
| Phase 5 suite audit | `3bea4fb9232eb940542bf053851bf371bae974da4e7aac11af778ec16c5fb62f` | `76fb46d332f4630b74fb9710c45a3966665549a6e105e3728ed828f3965cda9c` |
| Invariant-seed audit | `c47dddc8471f4710dfe54d9bcc981f43525c7c3d91696345fb1b7655fff2938d` | `037a02b2215f3633a3a716c1e739d452a7bdcdcb363a390e70b1678cc8585c04` |
| Specification completeness | `cabca4cc4d27c75e445dc0e3c71f27a27ccdf2c9c9332ef82ffbe6fcbaab5f58` | `dd6db7325c76d2c15fabb5a14d92eebf3c70d78d8e4361832fec8db6cea3fbf3` |
| Requirement/oracle audit | `b576d0d6486ef20861460408147ef729a760583cd29b4d3b7e3314b14eda2e23` | `a61fdf4983cd2f128fa7c8f5ab9f8474dd01ea7444150a27594169703dcc6ef3` |
| Conformance alignment | `b885567e03c82471cfb1062cba207eda70911640c61bde2938e90a1cfe5e4196` | `9a439133f7303a43b4cde2af4cb2f9c6533419e208a4656b085bad3bb398d964` |
| Runtime-anomaly audit | `cc2f6d28c6edd6b91d20aaaffdb86942894dd02437dcc1cc1ab89345bc2493d5` | `1a379aa9e32fde79c2b81ef35825ca834c42847c9859503840031d861641c6b6` |
| Fuzz-program audit | `0f0a92c540b8f4d3d6432e272af85207020026e2721f8892a6a72990882ba0f3` | `c2a89b6673b55c5960db621398c2aa2b98250a70c30882e8d8105d068469dafe` |
| Mutation program | `82976ea78e335925ad75ee35a0c9c5c9ff590ed57a280e36b21a120fa5da65be` | `ff2b0d46407a615a5520096e6ecf5612ef76f353d13f7058c06c0cc1a7747da5` |
| Specification-source audit | `c7f8ab2b1abb15709d873f11ff125d00bbb94cda22466787265ca428ac2e2e92` | `eb5be551e7824aed3a6ae7d724aac4bad199063bb38eaa91bd03fd9a9278ff2f` |
| Phase 12 workflow/UI audit | `949b48b7586454ca7f4826fb69fb9bdef75f5a7a98a5729faacc89ae58351a66` | `7cdbb3ffd6279dd8ce67b2edab11f578ecd8edd161023027cf1e0c61724ab6a8` |
| Phase 13 release-evidence audit | `e7b44bb714312f5e28e6e7d4c347b50946cb96ad97d5af04a7776e6b5ba8ed5e` | `efd6673115304c02143314714c741699ee6ea8b84eabdccab9657540b66f6b61` |
| Phase 11 hardware-lab program | `f36534ac24059c506edd61c4c9d70f46801a3d08aa02932ca098a7cb3b66d2c8` | `eae7883596ce8baee2f1242f9643da6732aa2ee8f1d2e6547834e4ddc34f7563` |

The rebind changes no product behavior, suite authorization, CI wiring, proof
level, or board state. It records the refreshed integration census and report
provenance only.
