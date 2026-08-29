# truST Specifications

This directory contains the IEC 61131-3 Structured Text language specs
(`01-09`), the split runtime/tooling specs (`10-14`), the Ladder/profile/editor
specs (`15-17`), the project/runtime and operational contract specs (`18-32`), and the current
non-numbered SFC profile note (`sfc-profile`). Product and tooling contracts in
this directory are explicitly identified and do not become IEC requirements.

## Document Index

| File | Owns | Relevant Crate |
|------|------|----------------|
| [01-lexical-elements.md](01-lexical-elements.md) | Character set, identifiers, keywords, comments, pragmas, literals | trust-syntax (lexer) |
| [02-data-types.md](02-data-types.md) | Elementary types, generic types, user-defined types, references, pointer extension | trust-hir (types) |
| [03-variables.md](03-variables.md) | Variable declarations, qualifiers, access specifiers, direct addressing | trust-hir (symbols) |
| [04-pou-declarations.md](04-pou-declarations.md) | FUNCTION, FUNCTION_BLOCK, PROGRAM, CLASS, INTERFACE, METHOD, NAMESPACE | trust-hir |
| [05-expressions.md](05-expressions.md) | Operators, precedence, evaluation rules | trust-syntax (parser), trust-hir (type check) |
| [06-statements.md](06-statements.md) | Assignment, calls, control flow, iteration, jumps | trust-syntax (parser) |
| [07-standard-functions.md](07-standard-functions.md) | Type conversion, numerical, string, date/time, assertion extensions | trust-hir |
| [08-standard-function-blocks.md](08-standard-function-blocks.md) | Bistable, edge detection, counter, timer FBs | trust-hir |
| [09-semantic-rules.md](09-semantic-rules.md) | Cross-cutting semantic validity rules shared by the language specs | trust-hir |
| [10-runtime-semantics.md](10-runtime-semantics.md) | Runtime value, memory, execution, stdlib, I/O, errors, testing API | trust-runtime |
| [11-runtime-engine.md](11-runtime-engine.md) | Runtime architecture, clocks, scheduler, process-image and driver integration, retain, launcher, and host composition | trust-runtime |
| [12-bytecode.md](12-bytecode.md) | STBC container, sections, instruction set, versioning | trust-runtime |
| [13-debug-adapter.md](13-debug-adapter.md) | Debug adapter semantics, breakpoints, variables, reload behavior | trust-debug, trust-runtime |
| [14-lsp.md](14-lsp.md) | LSP architecture, IDE behavior, protocol, diagnostics, performance | trust-lsp, trust-ide, trust-hir |
| [15-ladder-diagram.md](15-ladder-diagram.md) | Normative IEC-aligned LD language semantics and conformance rules | trust-runtime, trust-lsp, editors/vscode |
| [16-ladder-profile-trust.md](16-ladder-profile-trust.md) | truST LD schema/runtime/editor profile and interoperability constraints | trust-runtime, trust-lsp, editors/vscode |
| [17-visual-editors-runtime-unification.md](17-visual-editors-runtime-unification.md) | Shared ST-backed runtime/debug command path for Ladder/Statechart/Blockly | editors/vscode, trust-debug, trust-runtime |
| [18-configurations-resources-tasks.md](18-configurations-resources-tasks.md) | CONFIGURATION/RESOURCE/TASK declarations and program-to-task binding rules | trust-hir, trust-lsp, trust-runtime |
| [19-project-model.md](19-project-model.md) | Project tree, config-file roles, build/run lifecycle ownership | trust-runtime, trust-lsp |
| [20-agent-api-v1.md](20-agent-api-v1.md) | JSON-RPC contract for `trust-dev agent serve` | trust-dev |
| [21-harness-protocol.md](21-harness-protocol.md) | Deterministic harness wire protocol | trust-harness, trust-runtime |
| [22-developer-workflows.md](22-developer-workflows.md) | Developer source-discovery and project-scoped commit behavior | trust-dev |
| [23-connector-status.md](23-connector-status.md) | Canonical connector state, health, confidence, and point-quality vocabulary | trust-runtime, editors/vscode |
| [24-release-evidence.md](24-release-evidence.md) | Platform, source-build, dependency, artifact, version, hardware, and conformance evidence | release tooling, CI, docs |
| [25-vscode-product-contract.md](25-vscode-product-contract.md) | VS Code shell, Live Values, Devices & Connections, examples, libraries, HMI preview, and visual-editor product behavior | editors/vscode |
| [26-hir-semantic-kernel.md](26-hir-semantic-kernel.md) | Incremental HIR source identity, invalidation, translation, concurrency, and semantic-query contracts | trust-hir |
| [27-openot-authoring.md](27-openot-authoring.md) | OpenOT pragma vocabulary, runtime translation, producer publication, event materialization, and fenced reconciliation | trust-hir, trust-runtime |
| [28-hir-warning-policy.md](28-hir-warning-policy.md) | Product warning policy for unused declarations, conversions, complexity, reachability, determinism, and suspicious operations | trust-hir |
| [29-hir-sizeof-and-allocation.md](29-hir-sizeof-and-allocation.md) | Product contracts for `SIZEOF`, `NEW`, and `__DELETE` semantic validation | trust-hir |
| [30-verification-inventory.md](30-verification-inventory.md) | Governance contract for complete IEC table inventory coverage and fail-closed review | verification tooling |
| [31-oscat-library-profile.md](31-oscat-library-profile.md) | Normative truST product profile for the classic OSCAT package and its OOP facade | trust-runtime |
| [32-mqtt-io.md](32-mqtt-io.md) | MQTT communication, configuration, mapping, payload, security, worker, and broker-interoperability contracts | trust-runtime |
| [sfc-profile.md](sfc-profile.md) | Reserved SFC keywords, visual-editor scope, textual SFC boundary | editors/vscode, trust-syntax |

## Standard Reference

These specifications are based on:

> **IEC 61131-3:2013**
> *Programmable controllers - Part 3: Programming languages*
> Edition 3.0, 2013-02

## Coverage

### Fully Documented

- Structured Text (ST) language elements
- Elementary and user-defined data types
- Variable declarations and qualifiers
- Program organization units (POUs)
- Standard functions and function blocks
- Semantic and error rules
- Runtime semantics (see `10-runtime-semantics.md`)
- Runtime engine/platform architecture (see `11-runtime-engine.md`)
- Bytecode format (see `12-bytecode.md`)
- Debug adapter behavior (see `13-debug-adapter.md`)
- LSP/IDE behavior (see `14-lsp.md`)
- Ladder Diagram (LD) normative semantics (see `15-ladder-diagram.md`)
- Ladder Diagram (LD) implementation profile and interop constraints (see `16-ladder-profile-trust.md`)
- Visual editor runtime/debug ST-path unification contract (see `17-visual-editors-runtime-unification.md`)
- Current SFC keyword/profile scope (see `sfc-profile.md`)
- Configuration/resource/task declarations (see `18-configurations-resources-tasks.md`)
- Project model and build/run ownership (see `19-project-model.md`)
- Agent API contract (see `20-agent-api-v1.md`)
- Harness protocol (see `21-harness-protocol.md`)
- Developer source-discovery and commit workflows (see `22-developer-workflows.md`)
- Connector status vocabulary (see `23-connector-status.md`)
- Release evidence contracts (see `24-release-evidence.md`)
- VS Code product behavior (see `25-vscode-product-contract.md`)
- HIR semantic-kernel behavior (see `26-hir-semantic-kernel.md`)
- OpenOT authoring semantics (see `27-openot-authoring.md`)
- HIR warning policy (see `28-hir-warning-policy.md`)
- `SIZEOF` and allocation semantic validation (see `29-hir-sizeof-and-allocation.md`)
- IEC table inventory governance (see `30-verification-inventory.md`)
- OSCAT classic-package and OOP-facade profile (see `31-oscat-library-profile.md`)
- MQTT communication and PLC I/O mapping (see `32-mqtt-io.md`)

### Not Covered (Out of Scope)

- Instruction List (IL) - Deprecated in Edition 3
- Function Block Diagram (FBD) - Graphical language
- Sequential Function Chart (SFC) textual body syntax in the ST parser
- Communication function blocks

## Usage Guide

For runtime behavior, start with `docs/specs/10-runtime-semantics.md`.
For runtime platform/ops architecture, use `docs/specs/11-runtime-engine.md`.
For STBC container details, use `docs/specs/12-bytecode.md`.
For debugging behavior, use `docs/specs/13-debug-adapter.md`.
For LSP/IDE behavior, use `docs/specs/14-lsp.md`.
For configuration/resource/task declarations, use `docs/specs/18-configurations-resources-tasks.md`.
For machine-facing orchestration contracts, use `docs/specs/20-agent-api-v1.md`
and `docs/specs/21-harness-protocol.md`. For developer workflow, connector
status, and release evidence contracts, use `docs/specs/22-developer-workflows.md`,
`docs/specs/23-connector-status.md`, `docs/specs/24-release-evidence.md`, and
`docs/specs/25-vscode-product-contract.md`.
For HIR query/invalidation behavior, OpenOT authoring, warning policy, and
allocation-related semantic checks, use `docs/specs/26-hir-semantic-kernel.md`
through `docs/specs/29-hir-sizeof-and-allocation.md`. For verification
inventory governance, use `docs/specs/30-verification-inventory.md`.
For MQTT broker communication, configuration, symbolic tag mapping, payloads,
and interoperability, use `docs/specs/32-mqtt-io.md`.

For IEC coverage tracking and spec-to-test mapping, see:
- `docs/specs/coverage/standard-functions-coverage.md`
- `docs/specs/coverage/iec-table-test-map.toml`
- `docs/specs/coverage/ld-coverage.md`

### For Lexer Development (trust-syntax)

Start with [01-lexical-elements.md](01-lexical-elements.md):
- Token definitions (keywords, literals, operators)
- Comment and pragma syntax
- Identifier rules

### For Parser Development (trust-syntax)

Refer to:
- [05-expressions.md](05-expressions.md) for operator precedence
- [06-statements.md](06-statements.md) for statement syntax
- [04-pou-declarations.md](04-pou-declarations.md) for declaration syntax

### For Type System (trust-hir)

Consult:
- [02-data-types.md](02-data-types.md) for type hierarchy
- [07-standard-functions.md](07-standard-functions.md) for function signatures

### For Semantic Analysis (trust-hir)

Use:
- [03-variables.md](03-variables.md) for scope and access rules
- [09-semantic-rules.md](09-semantic-rules.md) for error conditions
- [26-hir-semantic-kernel.md](26-hir-semantic-kernel.md) for query identity,
  invalidation, translation, and concurrency
- [27-openot-authoring.md](27-openot-authoring.md) for OpenOT product semantics
- [28-hir-warning-policy.md](28-hir-warning-policy.md) for warning diagnostics
- [29-hir-sizeof-and-allocation.md](29-hir-sizeof-and-allocation.md) for
  `SIZEOF`, `NEW`, and `__DELETE`

## Table Reference

Key tables from the IEC 61131-3 standard referenced in these documents:

| Table | Content | Document |
|-------|---------|----------|
| Table 1 | Character set | 01-lexical-elements.md |
| Table 2 | Identifiers | 01-lexical-elements.md |
| Table 3 | Comments | 01-lexical-elements.md |
| Table 4 | Pragmas | 01-lexical-elements.md |
| Table 5 | Numeric literals | 01-lexical-elements.md |
| Table 6-7 | String literals | 01-lexical-elements.md |
| Table 8 | Duration literals | 01-lexical-elements.md |
| Table 9 | Date/time literals | 01-lexical-elements.md |
| Table 10 | Elementary data types | 02-data-types.md |
| Table 11 | User-defined types | 02-data-types.md |
| Table 12 | Reference operations | 02-data-types.md |
| Table 13-14 | Variable declaration | 03-variables.md |
| Table 15-16 | Arrays, direct variables | 03-variables.md |
| Table 17 | Partial access to ANY_BIT variables | 02-data-types.md |
| Table 19 | FUNCTION declaration | 04-pou-declarations.md |
| Table 22-27 | Type conversion functions | 07-standard-functions.md |
| Table 28-36 | Standard functions | 07-standard-functions.md |
| Table 40 | FUNCTION_BLOCK declaration | 04-pou-declarations.md |
| Table 43 | Bistable FBs | 08-standard-function-blocks.md |
| Table 44 | Edge detection FBs | 08-standard-function-blocks.md |
| Table 45 | Counter FBs | 08-standard-function-blocks.md |
| Table 46 | Timer FBs | 08-standard-function-blocks.md |
| Section 8.2 | Ladder Diagram (LD) semantics | 15-ladder-diagram.md |
| Table 47 | PROGRAM declaration | 04-pou-declarations.md |
| Table 48 | CLASS declaration | 04-pou-declarations.md |
| Table 51 | INTERFACE declaration | 04-pou-declarations.md |
| Table 64-66 | NAMESPACE declaration | 04-pou-declarations.md |
| Table 71 | ST operators | 05-expressions.md |
| Table 72 | ST statements | 06-statements.md |
| Figure 5 | Generic type hierarchy | 02-data-types.md |
| Figure 7 | Variable sections | 03-variables.md |
| Figure 11-12 | Type conversions | 02-data-types.md |
| Figure 15 | Timer timing diagrams | 08-standard-function-blocks.md |

## Implementation Status

To track implementation progress against these specifications, compare with:
- `crates/trust-syntax/src/lexer.rs` - Lexer implementation
- `crates/trust-syntax/src/parser.rs` - Parser implementation
- `crates/trust-hir/src/` - HIR and type system
- `crates/trust-ide/src/` - IDE features

## Contributing

When updating these specifications:
1. Reference the specific IEC 61131-3 section/table number
2. Include code examples from the standard where helpful
3. Mark implementer-specific features clearly
4. Keep formatting consistent with existing documents
