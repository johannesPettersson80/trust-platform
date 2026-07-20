# PLC Integration Report Rebind

- Source commit: `ea78013921d1731b5d808c76adc4621edf396eff`
- Timestamp: `2026-07-20T08:40:00+02:00`
- Platform: `trust-builder-linux-x86_64`

All 18 report pairs were generated and validated from pristine detached
worktrees. Four workers reduced wall time without sharing a dirty checkout.
Only each command's declared JSON and Markdown outputs were collected.

| Report | JSON SHA-256 | Markdown SHA-256 |
| --- | --- | --- |
| Test-class completeness | `1451d56f0e12892890bfd3e17080069ab332cfdf461d922e12a3e60349aec676` | `9643281f5c2485bb76a24634387449ec7f650ea87b37dabdfe5438d57729a269` |
| Coverage-matrix gaps | `9a535ae010f4d15d571b8df1f9816ad56c1a64ce5b422c638d2a9149a9197003` | `fc3e2c8efef54225457baea851e6094ed7ead200dee8e8bd09e8ae80aaea4b21` |
| Malformed-input coverage | `d6fba97454d64a7677d47ee1a59fc770710140adf947515f72bee5420c135135` | `fd804f43eae5bc8c629d5f80fd17f160b965bbabccddf01a54e870f103b58988` |
| Unmapped-test debt | `0b6ecc28b4bcfcbec41f3fe7124f09259385a536b97e95bc2ef624293fa83e01` | `de9f688668cdcdc9c8e73437074c0567b7c4129725c14bed8bd64a0a67314877` |
| Test-refactor assessment | `1f8eb123bc62d39afeac6057d6da1575337e4df8dbdda597c1cfdd485d563664` | `f431d4e19a0c7b0ee89cf1b3800dfc872b981b2c715f443bc2a0364f744d173e` |
| Ignored-test inventory | `1c2ad8652afbe56103742faf20929b662cea992ee5027345109f34f4b45120a5` | `c3d931eeee8184203d913a0f89b3d93586c6ddbb1b17ab1646ff9a2e025a83c6` |
| Phase 5 suite audit | `b64a10c7673c83ad6f603b5168c3039b29802ad8b392ceda3c9d36a298f9cea3` | `75e5c28320f4aa4ca39fc0cb2db5ec8818c31b90e6a03469ddd094acaa055e49` |
| Invariant-seed audit | `4c254ed0a0c18c6960ba8b462f71dc8eb216988409689188596d8a6f6d197196` | `c6ce472edfea2cf899e3892dff3b24732c30fe3e50487b6b08dd731a71c0df43` |
| Specification completeness | `57a60ff5c55c7bb75d0987e192b1219ecf7651d0355fa1d97ed65d0fbe931fcf` | `e90e2cc4240b947da481cce243c66ca244dd1d1f6fdf3bfff495cee3355976a1` |
| Requirement/oracle audit | `129eeea840f43a56b94838b615c9911b0b366f8da70ac71e95d00f2d49bbde08` | `903eca859ef1d3eb677e3104df81b6cd58c2e86a943a3f53cba94b2386fcd8ab` |
| Conformance alignment | `6a9a28b36b7b9f2b2c7050f686062ee5fb7abe90a82d5f758374bba020e46eeb` | `dbee5dda19e034526f2befca03abb009db6b7b5ef9c988adade91d7e5fcb688a` |
| Runtime-anomaly audit | `e482cf78c299cd81f65f897b22b3267dcb60e75771ee8636b6935a48a33b3aee` | `fee2d31d73b4f70c821c96c841bb4dd0bd320793b1726ef860f029a088f213ee` |
| Fuzz-program audit | `872cc465e5a856b0a5732aa53368c41f1b486b5f75e0a4af8408d340ccc9c6c8` | `fbf6f461738e74679011e62bc9c1e24db33bb0239332df9563781bae4f7d639c` |
| Mutation program | `0603119aaaa380188a32ba7721fe69ac8e39542832a06e060f14dc7e6fb0baef` | `8ce9b48fa778ca743e17280eeba039e1ed401e13fdaf118f2ca5471e2246afb8` |
| Specification-source audit | `d9c53456ccb24bae98514fb5e1d4e8f90872b9361c83b11a605439039e43e2f9` | `4832b5262c4f1c6bba7dc3556ac7505a19042667edaf030c1cc1460026447b0e` |
| Phase 12 workflow/UI audit | `5164e373ecd7000f982dcf2b7809cfcae2933b80aafe5081616b34a84a5598b3` | `573cf146d7e8ca0a3b82ed914b8da5f0b99e07a99d017489fe98adc31d96ec5e` |
| Phase 13 release-evidence audit | `3a1f50a408c7c69bf9f392f30e4f5af309e88c5cf357ba82eafa579eac0d6d47` | `13da6cc980dc7da534a8dcd7ad320f9fc8c0fb74a151f29ea9e28c1789d2e432` |
| Phase 11 hardware-lab program | `7bdb9cd2e91740552fb3157fc9fa9ae2edd542f4adbe83cbc9c1f4217198a070` | `35f426e7bf5252815c039a1466c47c28bdb36fc7ecc33d27af7c192606932997` |

The rebind changes no product behavior, suite authorization, CI wiring, proof
level, or board state. It records the refreshed integration census and report
provenance only.
