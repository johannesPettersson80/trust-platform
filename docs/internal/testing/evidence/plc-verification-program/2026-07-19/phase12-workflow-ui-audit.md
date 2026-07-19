# Phase 12 Workflow and UI Journey Audit

Generator: `phase12-workflow-ui-audit v1`
Source revision: `93e644975a9e29063da4461b95871f41774fde59`
Generated: `2026-07-19T22:50:00+02:00`
Platform: `linux-aarch64`
Generated JSON SHA-256: `949b48b7586454ca7f4826fb69fb9bdef75f5a7a98a5729faacc89ae58351a66`
Input SHA-256: `sha256:3db37ab82a5eb0af48cd5e6a875a3067e2feac8e62dcbcf0b420231f19203a95`

This report inventories workflow specifications and UI journey evidence without
converting backend tests or provisional screenshots into UI acceptance.

## Summary

- Public workflow candidates: 47
- Workflow specifications: 33
- Reviewed nonworkflows: 14
- Workflow specs missing invariant links: 29
- Workflow specs missing acceptance evidence: 33
- UI journeys: 30
- Journeys with fresh visual evidence: 1
- Backend-supported journeys without fresh visual evidence: 0

## Workflow Review

| Candidate | Disposition | Spec source | Journeys | Invariants | Acceptance |
| --- | --- | --- | --- | --- | --- |
| `WORKFLOW_CANDIDATE_DE089F3C6FC9C0CC2D379F95` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_75156CC03F315B9579C4C79C` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_3D214144F481BFE56F8DDDC1` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_CEC76566DE1B285E5CA38320` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_7649437BD4EB84A337E84535` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_6A9FCE162E075BA0EA8A0671` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_D334B2CB65DE6B33E415D255` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_ACCF4969E1327DE6B799568F` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_ACCF4969E132_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_3C0DDF559EC291A8293B985C` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_3C0DDF559EC2_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_456FA568AFC2994E73F07160` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_456FA568AFC2_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_D7844FFDB0CC66E4A63190BE` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_D7844FFDB0CC_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_F16D362FF6234B98DD156CA4` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_F16D362FF623_001` | `J-17` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_94D293455A698D5BF1BA8AC3` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_94D293455A69_001` | `J-18` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_FD234848695904A9A6A35342` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_D24FCC998C80FB91D155C38B` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_D24FCC998C80_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_7E9C84093FC5EFDE994DB33F` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_7E9C84093FC5_001` | `J-32` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_0F343DE358D002D8F09D4694` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_0F343DE358D0_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_15A2F0DD661339E7B7FDD01C` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_15A2F0DD6613_001` | `J-26` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_4C182E3FAB92EC274C3A75B9` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_BB46EA910511E52D723B21C1` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_BB46EA910511_001` | `J-26` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_812A3212DF07E6617191E075` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_812A3212DF07_001` | `J-26` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_CDC07CFF8A917C8DBAF6EB02` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_CDC07CFF8A91_001` | `J-26` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_7EEF7481947C105881BAC728` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_7EEF7481947C_001` | `J-24` | `UI_STATUS_001` | `missing` |
| `WORKFLOW_CANDIDATE_7B2D72A09D5208C36EE460B9` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_686AA9E04F9471A5529224E3` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_1AB8C986099E14573E3DA30B` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_A22E00BD1F131DA07539CBDC` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_A22E00BD1F13_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_D3D2C29082449A6AA6E9ABA4` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_D3D2C2908244_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_87AF7E445561A27FC77D1FAC` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_87AF7E445561_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_962469ED8F0BF062E6B2C972` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_81374E35F805D94E472F9B39` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_81374E35F805_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_819D4CA4AFE80939AC52D192` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_819D4CA4AFE8_001` | `J-24` | `UI_STATUS_001` | `missing` |
| `WORKFLOW_CANDIDATE_11CE18940EA5654E4C536EDC` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_11CE18940EA5_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_0DFBFE040B69305328ECDBC4` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_0DFBFE040B69_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_E001D7C69B4CFD17AC6B45B5` | `reviewed_nonworkflow` | `none` | `none` | `none` | `not_applicable` |
| `WORKFLOW_CANDIDATE_92F337788E11BA12E26A1F1F` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_92F337788E11_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_B856ED5CDDB494B14EF9A53D` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_B856ED5CDDB4_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_E15DCFA0D2646305EBCEF808` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_E15DCFA0D264_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_556BCA7D07FB1286B97928B1` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_556BCA7D07FB_001` | `J-01, J-04` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_A9AF3F21E41ED6A0597FD1F1` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_A9AF3F21E41E_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_F08D90359C6BA2DF43D50A47` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_F08D90359C6B_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_F495BAA5B93DC09081231F64` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_F495BAA5B93D_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_CC557DA8405666CCB0D1A71F` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_CC557DA84056_001` | `J-24` | `UI_STATUS_001` | `missing` |
| `WORKFLOW_CANDIDATE_06F0B50B276C143A8E3394BD` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_06F0B50B276C_001` | `none` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_C8E790E4831041416EBC315D` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_C8E790E48310_001` | `J-03, J-04, J-05, J-07` | `DEBUG_PAUSE_001, EDIT_RENAME_001, EDIT_RENAME_002` | `missing` |
| `WORKFLOW_CANDIDATE_027D30E7601F15D962AC2EBC` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_027D30E7601F_001` | `J-07` | `none` | `missing` |
| `WORKFLOW_CANDIDATE_28057E7E1763569B539C8DB3` | `workflow_spec` | `SPEC_PUBLIC_WORKFLOW_28057E7E1763_001` | `none` | `none` | `missing` |

## UI Journeys

| Journey | Status | Workflows | Invariants | Supporting tests | Fresh visual |
| --- | --- | --- | --- | --- | --- |
| `J-01` | `evidence_missing` | `WORKFLOW_CANDIDATE_556BCA7D07FB1286B97928B1` | `none` | `none` | `false` |
| `J-02` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-03` | `evidence_missing` | `WORKFLOW_CANDIDATE_C8E790E4831041416EBC315D` | `EDIT_RENAME_001, EDIT_RENAME_002` | `none` | `false` |
| `J-04` | `evidence_missing` | `WORKFLOW_CANDIDATE_556BCA7D07FB1286B97928B1, WORKFLOW_CANDIDATE_C8E790E4831041416EBC315D` | `none` | `none` | `false` |
| `J-05` | `evidence_missing` | `WORKFLOW_CANDIDATE_C8E790E4831041416EBC315D` | `DEBUG_PAUSE_001` | `none` | `false` |
| `J-06` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-07` | `evidence_missing` | `WORKFLOW_CANDIDATE_C8E790E4831041416EBC315D, WORKFLOW_CANDIDATE_027D30E7601F15D962AC2EBC` | `none` | `none` | `false` |
| `J-08` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-09` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-11` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-12` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-13` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-14` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-15` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-16` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-17` | `evidence_missing` | `WORKFLOW_CANDIDATE_F16D362FF6234B98DD156CA4` | `none` | `none` | `false` |
| `J-18` | `evidence_missing` | `WORKFLOW_CANDIDATE_94D293455A698D5BF1BA8AC3` | `none` | `none` | `false` |
| `J-19` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-20` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-23` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-24` | `evidence_missing` | `WORKFLOW_CANDIDATE_7EEF7481947C105881BAC728, WORKFLOW_CANDIDATE_819D4CA4AFE80939AC52D192, WORKFLOW_CANDIDATE_CC557DA8405666CCB0D1A71F` | `UI_STATUS_001` | `none` | `false` |
| `J-25` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-26` | `evidence_missing` | `WORKFLOW_CANDIDATE_15A2F0DD661339E7B7FDD01C, WORKFLOW_CANDIDATE_BB46EA910511E52D723B21C1, WORKFLOW_CANDIDATE_812A3212DF07E6617191E075, WORKFLOW_CANDIDATE_CDC07CFF8A917C8DBAF6EB02` | `none` | `none` | `false` |
| `J-27C` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-28` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-29` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-31` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-32` | `evidence_missing` | `WORKFLOW_CANDIDATE_7E9C84093FC5EFDE994DB33F` | `none` | `none` | `false` |
| `J-Deploy` | `evidence_missing` | `none` | `none` | `none` | `false` |
| `J-PEER-TOPOLOGY-FAILURE` | `provisional` | `none` | `UI_STATUS_001` | `TEST_REVIEW_PEER_TOPOLOGY_VISIBLE_FAILURE_001, TEST_REVIEW_PEER_TOPOLOGY_ERROR_RENDER_001` | `true` |

## Boundaries

- `report_emits_proof`: `false`
- `report_promotes_ui_invariants`: `false`
- `backend_proof_replaces_visual_evidence`: `false`
- `source_transform_requires_silent_corruption_risk`: `true`
- `validated_ui_requires_accepted_journey`: `true`

## Limitations

- The report inventories reviewed workflow and UI-journey associations; it emits no product proof and promotes no invariant.
- A backend or extension test is supporting evidence only and cannot replace fresh visual journey evidence.
- Evidence-missing, stale, and provisional journeys remain visible debt; only ux_accepted is acceptance.
- Implementation-change attribution is limited to the implementation paths explicitly owned by each journey row.
