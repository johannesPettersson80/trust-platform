# Phase 17 historical debt routing review

Status: implementation-side routing and gap-classification review. This is not
the independent `VERIF-P17-012` semantic acceptance verdict.

Review date: 2026-07-21

The conversion preserves every frozen identity, signature digest, body digest,
visibility, and parser-derived risk signal. It does not infer expected behavior
from names, source text, tests, or the current implementation. Each converted
fact remains `spec_gap` against one exact, same-area open gap; the gap lists
every affected `CODE_*` identity explicitly. The debt baseline is reduced only
by deleting the corresponding complete entries.

## Reviewed ambiguous routing

- Runtime VM and register-interpreter paths route to `bytecode_vm`; scheduler
  paths route to `runtime_safety`.
- Runtime debug/control handlers route to `control_security`; the standalone
  debug adapter and VS Code debug client route to `editor_safety`.
- Runtime HMI and the reviewed VS Code HMI, I/O, network-canvas, ladder, SFC,
  Blockly, statechart, and visual webview paths route to `hmi_ui`.
- Runtime I/O driver and addressing paths route to `protocols`.
- Developer build-command logic routes to `plcopen_devtools`; Cargo/product
  build and bundle assembly paths route to `supply_chain_platform`, except VM
  encoder and harness builders, which route to `bytecode_vm`.
- Release evidence scripts route to `release`; other gate, benchmark, hardware,
  and unsafe-analysis scripts route to `verification`.
- Vendored `third_party/tiverse-mmap/**` facts route to
  `supply_chain_platform`.

The conversion set contains 15,133 facts and resolves 2,175 previously
ambiguous area bindings. Counts by selected area are:

| Area | Facts | Risk facts | Resolved bindings |
|---|---:|---:|---:|
| `bytecode_vm` | 1,227 | 461 | 302 |
| `compiler_iec` | 1,504 | 362 | 0 |
| `control_security` | 2,405 | 1,136 | 193 |
| `editor_safety` | 3,304 | 1,069 | 449 |
| `hmi_ui` | 764 | 235 | 764 |
| `plcopen_devtools` | 352 | 146 | 2 |
| `protocols` | 1,546 | 418 | 88 |
| `release` | 21 | 13 | 21 |
| `runtime_safety` | 1,667 | 586 | 88 |
| `supply_chain_platform` | 237 | 126 | 237 |
| `verification` | 2,106 | 1,177 | 31 |

## bytecode-vm-historical-risk-gap

Risk acknowledgements retain every parser-derived signal for the exact
`bytecode_vm` body digest while behavior remains in the open area gap.

## compiler-iec-historical-risk-gap

Risk acknowledgements retain every parser-derived signal for the exact
`compiler_iec` body digest while behavior remains in the open area gap.

## control-security-historical-risk-gap

Risk acknowledgements retain every parser-derived signal for the exact
`control_security` body digest while behavior remains in the open area gap.

## editor-safety-historical-risk-gap

Risk acknowledgements retain every parser-derived signal for the exact
`editor_safety` body digest while behavior remains in the open area gap.

## hmi-ui-historical-risk-gap

Risk acknowledgements retain every parser-derived signal for the exact
`hmi_ui` body digest while behavior remains in the open area gap.

## plcopen-devtools-historical-risk-gap

Risk acknowledgements retain every parser-derived signal for the exact
`plcopen_devtools` body digest while behavior remains in the open area gap.

## protocols-historical-risk-gap

Risk acknowledgements retain every parser-derived signal for the exact
`protocols` body digest while behavior remains in the open area gap.

## release-historical-risk-gap

Risk acknowledgements retain every parser-derived signal for the exact
`release` body digest while behavior remains in the open area gap.

## runtime-safety-historical-risk-gap

Risk acknowledgements retain every parser-derived signal for the exact
`runtime_safety` body digest while behavior remains in the open area gap.

## supply-chain-platform-historical-risk-gap

Risk acknowledgements retain every parser-derived signal for the exact
`supply_chain_platform` body digest while behavior remains in the open area
gap.

## verification-historical-risk-gap

Risk acknowledgements retain every parser-derived signal for the exact
`verification` body digest while behavior remains in the open area gap.
