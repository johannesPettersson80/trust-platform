# Phase 8 Runtime-Anomaly Report Rebind

Date: 2026-07-18

Source commit: `5395f5d969d7f5828dc2e7e3701f6d7bc7f69a56`

The exhaustive runtime-anomaly denominator adds a verification module and
changes shared board and metadata inputs. The 15 report pairs were regenerated
in the isolated `trust-builder` checkout
`~/.cache/trust-platform-p8-denominator-5395f5d9` with timestamp
`2026-07-18T09:53:26+02:00`.

The checkout was reset to the clean source commit before every generator. Each
matching at-rest validator passed before its JSON and Markdown outputs were
copied to an external staging directory. The checkout was reset after the last
pair and finished clean at the source commit.

| Evidence row | JSON SHA-256 | Markdown SHA-256 |
| --- | --- | --- |
| `EVID_P2_TEST_CLASS_COMPLETENESS_20260710` | `6869c28f4a76f5869db073a9b80856d4fe105541406c76959461afe7af054b4f` | `7fdcea839ab2cceb1de8fc90153dfe58ada59421ace7d8146d85c76010748268` |
| `EVID_P2_COVERAGE_MATRIX_GAPS_20260710` | `ed34e01e5e074ef1cd12b22cd56b17e73d257d728f54e84db82cda23573394b5` | `a4a89da057c46655a2a0ca99c5f316ecb572d356ff6754ec36fd13cd5942095e` |
| `EVID_P2_MALFORMED_INPUT_COVERAGE_20260710` | `99b9a940dd59d78d37e4c06b77f0a320806ef80768980915457740e2ea557596` | `c242e93c7d26a41ebe4214e23845886403b91025b8e0e023f390547a0e5e0f67` |
| `EVID_P2_UNMAPPED_TEST_DEBT_20260710` | `f73c80128d788be495ea6ee9afd6cef633b6bae55907fcfbcb5e4c67a82a9558` | `d3b1438f27890bdf6979b1c1c235ac0849880d460b1de71f38ba4c4601d185d7` |
| `EVID_P2A_TEST_REFACTOR_ASSESSMENT_20260710` | `e9e4b249ff86d534163413e4c5f498f775eafcce19ce9ecaa2b876505c007fbd` | `15c8228f11f9edaf5a44d571cfaffe47c44887f89b3e193924148a5d21002db1` |
| `EVID_P3_IGNORED_TEST_INVENTORY_20260710` | `a82e757c5fc51bfea49dc4d3c3d7ffbc14e7b888c2dcb9dee4240bbddec55ca9` | `d2e3187a50d0ed40a17681013a99d41fad8bb8b77070323c4f0f49cf7f54291a` |
| `EVID_P4_INVARIANT_SEED_AUDIT_20260710` | `94da647478628e5397ffaae3e3ebff697a18413ff894f81328d5c83812090895` | `913e55c16f9d041ad144d693dc957225ea84847038e75580359e8bb7471f75b3` |
| `EVID_P4A_SPECIFICATION_COMPLETENESS_20260710` | `93dea7356309fe09a50544ba6173751e42f5f42529646138fe71f312d9dca82b` | `c88ff1a12b577fe257a052836b97e88e134b2ba897c144f13ceb5fb9daf1a4a6` |
| `EVID_P5_SUITE_GATE_ROUTING_AUDIT_20260710` | `bea7b4b8a9c4e310b0394b961e55a3c79d211a4b7c59ee2473ee564258ca1c6b` | `1ee206ea6363751323dcfa4a9452904a8a153ad0be43ef4a36ea11138bce3261` |
| `EVID_P6_REQUIREMENT_ORACLE_AUDIT_20260711` | `e9edecd290367bc703a25043e95d25e932fe9d262905159622b5f421dc7bcda2` | `6242922039881a30dc8ebc6e14f47570bd373063d5b7c0a2581a3103f6db3df6` |
| `EVID_P7_CONFORMANCE_ALIGNMENT_20260711` | `bb2c7bd2e57b73c00637a2412895759a54da74ce70a9c40d385467fd9677a585` | `ddbfc4f1fb033b5475f35d2db2a5c2b9a8253ed6089202024ea4004a6bd9e02b` |
| `EVID_P8_RUNTIME_ANOMALY_AUDIT_20260711` | `2d3daaeb511608c8ea56c5febf76b40857d4d84f66b536fb4f9acedbd85ef339` | `c806d41d1fd5dd10cd1c21a5aa91d0550c718b424b1b251acc53f754464dbda5` |
| `EVID_P9_FUZZ_PROGRAM_AUDIT_20260711` | `bf85d197848bda7db1fe1c019c91c268fe1ca053f3460e55ecdd898b2737a446` | `55ad21d7b71e34ee28787cc22876b18dc8414d6918f7f50041f3158682c147c5` |
| `EVID_P10_MUTATION_PROGRAM_REPORT_20260712` | `4d2a41b3d4263ee1b430bdbb90fe43468270f9dd52cabf5e5bf7505506b2b5d2` | `ee75730e9f4daf04fff34ef533943f16e16bc797e0cb85e7fab2c85c08e4de45` |
| `EVID_P1A_SPEC_SOURCE_AUDIT_20260713` | `287e66ba51a655d18daf2a8758f7bd69ee7584acda74c0d2a9535b630fab6cf0` | `eb81267efbcdfee960d2b9cb6fbf318bafa349d7324bb1e463387bfde3ca6d14` |

This rebind creates no proof, closes no specification gap, changes no product
behavior, and changes no CI or suite enforcement.

