# Specification Architecture Audit

Status: In progress. Complete inventory recorded on 2026-08-29; release-relevant
ownership corrections and enforcement remain open.

Purpose: keep `docs/specs` organized by one authoritative product or language
responsibility per document. File size is a review signal, not an automatic
reason to split. A document must split when unrelated owners, lifecycles, or
test surfaces share authority; a cohesive language or wire-format specification
may remain large when its internal navigation is clear.

This audit is source-derived from every Markdown document directly under
`docs/specs`, its headings, cross-references, implementation owners, and native
test boundaries. The communication-specific evidence is recorded in
`communication-solid-spec-ownership-audit.md`.

## Rules

- [x] `SPECARCH-RULE-001` One behavior has one authoritative specification.
  Summaries link to that authority and do not restate competing normative rules.
- [x] `SPECARCH-RULE-002` IEC language semantics, truST product semantics,
  runtime integration, protocol behavior, UI behavior, and developer/release
  workflow behavior are distinct responsibilities.
- [x] `SPECARCH-RULE-003` Backlog, implementation status, and future-work lists
  belong in tracked checklists or plans, not in normative product contracts.
- [x] `SPECARCH-RULE-004` A split preserves stable links through explicit
  replacement references and preserves every behavioral requirement. It does
  not silently delete prose to make a file smaller.
- [x] `SPECARCH-RULE-005` Native tests bind to the closest owning specification.
  A change to MQTT cannot satisfy its spec companion with an unrelated runtime,
  UI, or workflow document.
- [x] `SPECARCH-RULE-006` Specification architecture enforcement is itself a
  native executable repository test and runs in pull-request CI.

## Inventory and disposition

Line counts below are audit signals from the candidate before reorganization.

| Document | Lines | Responsibility assessment | Disposition |
|---|---:|---|---|
| `01-lexical-elements.md` | 564 | Cohesive lexer/token authority with IEC anchors and implementation boundary notes. | Keep. Normalize implementation-note wording only when touched. |
| `02-data-types.md` | 835 | Cohesive type-system authority; extensions are clearly labeled. | Keep. Its size follows the IEC type domain rather than mixed product ownership. |
| `03-variables.md` | 725 | Cohesive declaration/storage/qualifier authority. | Keep. Cross-reference runtime storage rather than importing runtime behavior. |
| `04-pou-declarations.md` | 889 | Cohesive POU declaration authority. Duplicate `Implementation Notes` headings need cleanup. | Reorganize headings, no semantic split. |
| `05-expressions.md` | 479 | Cohesive expression semantics. | Keep. |
| `06-statements.md` | 787 | Cohesive statement semantics. | Keep. |
| `07-standard-functions.md` | 858 | Cohesive standard-function authority, including clearly labeled truST extensions. | Keep. Continue using the separate coverage table for inventory. |
| `08-standard-function-blocks.md` | 727 | Cohesive standard-FB authority. | Keep. |
| `09-semantic-rules.md` | 896 | Cohesive cross-cutting semantic-diagnostic authority. One duplicated `3.5` numbering needs cleanup. | Reorganize numbering, no semantic split. |
| `10-runtime-semantics.md` | 3,375 | Mixes value representation, memory, execution, standard library, I/O, errors, testing API, implementation phases, and verification. | Split required. Preserve `10` as the runtime-semantics index/core execution contract; move I/O integration and testing API to dedicated owners, and move phases/status to checklists. |
| `11-runtime-engine.md` | 5,281 | Mixes scheduler/clock/process image with every I/O protocol, discovery, ADS product surfaces, Web IDE, realtime, mesh, cloud, launcher, CLI, HMI, and deployment. | Immediate split required. Keep only runtime-engine composition and integration ports. Protocol, control/product, UI, and deployment contracts receive dedicated authorities. |
| `12-bytecode.md` | 1,348 | Large but cohesive STBC container and instruction-set authority. | Keep. Improve H2/H3 hierarchy; no responsibility split. |
| `13-debug-adapter.md` | 717 | Cohesive DAP/debug contract, but ends with normative-looking improvement backlog. | Keep behavior contract; move `Required Improvements` to a checklist. |
| `14-lsp.md` | 1,368 | Mixes LSP/IDE behavior with duplicate lexer/parser/semantic summaries, runtime/debugger integration, testing/status material, and PLCopen interchange. | Split/reorganize required. Keep LSP transport and IDE feature behavior; replace language summaries with links, move PLCopen to its product owner, and move status/backlog out. |
| `15-ladder-diagram.md` | 212 | Cohesive IEC-aligned LD semantics. | Keep. |
| `16-ladder-profile-trust.md` | 164 | Cohesive truST LD schema/runtime/interoperability profile distinct from IEC semantics. | Keep. |
| `17-visual-editors-runtime-unification.md` | 99 | Cohesive cross-editor ST-lowering and runtime-entry architecture contract. | Keep as shared integration authority; UI details remain in editor product specs. |
| `18-configurations-resources-tasks.md` | 172 | Cohesive IEC configuration/resource/task declaration plus runtime binding contract. | Keep. |
| `19-project-model.md` | 70 | Cohesive canonical-file and lifecycle ownership index. | Keep as the authority linking configuration files to their domain specs. |
| `20-agent-api-v1.md` | 69 | Cohesive agent JSON-RPC API boundary. | Keep. |
| `21-harness-protocol.md` | 13 | Intentionally thin authority pointing to the canonical schema. | Keep; do not duplicate the schema into Markdown. |
| `22-developer-workflows.md` | 912 | Mixes project init, tutorials, native tests, CI reliability, release cleanup, registry, PLCopen, API docs, prompts, HMI scaffolding, compatibility aliases, and project commit behavior. | Immediate split required. Keep developer command/workspace workflows; move CI/release mechanics to `24`, PLCopen to a dedicated spec, registry to a registry spec, and native-test execution to a test-runner spec. |
| `23-connector-status.md` | 80 | Cohesive shared connector status vocabulary and projection contract. | Keep. It must not absorb protocol behavior. |
| `24-release-evidence.md` | 158 | Cohesive release/provenance/evidence authority. | Keep and receive release-gate/capture/cleanup requirements currently mixed into `22`. |
| `25-vscode-product-contract.md` | 872 | One product, but multiple independently changing surfaces: shell/projects, Live Values, Devices & Connections, HMI/visual editors, and existing authoring workflows. | Split required after communication release. Keep `25` as product-shell/index contract and create surface-specific documents with registered extension tests. |
| `26-hir-semantic-kernel.md` | 236 | Cohesive HIR query/source-identity/invalidation kernel. | Keep. |
| `27-openot-authoring.md` | 497 | One vertical OpenOT product contract spanning authoring to producer/publication/reconciliation. Boundaries are explicit and share one event-model lifecycle. | Keep. If transport variants grow, extract publication transport without splitting the event semantics. |
| `28-hir-warning-policy.md` | 130 | Cohesive diagnostic warning policy. | Keep. |
| `29-hir-sizeof-and-allocation.md` | 54 | Cohesive language-extension contract. | Keep. |
| `30-verification-inventory.md` | 240 | Cohesive verification-tooling contract and evidence limits. | Keep. It cannot become product behavior authority. |
| `31-oscat-library-profile.md` | 189 | Cohesive OSCAT compatibility/product profile. | Keep. |
| `sfc-profile.md` | 52 | Cohesive current SFC profile boundary. | Keep. Consider numeric naming only when SFC becomes a full language spec. |
| `README.md` | 203 | Index and usage guide. Current table order is not numeric and coverage omits later documents. | Reorganize immediately and make the ownership column explicit enough to route changes. |

## Immediate release-relevant corrections

- [x] `SPECARCH-NOW-001` Add a dedicated MQTT protocol specification and make
  it the sole authority for broker configuration, mapping directions, topics,
  payloads, security, sessions, and interoperability.
- [x] `SPECARCH-NOW-002` Reduce MQTT material in `11-runtime-engine.md` to its
  runtime integration port: atomic tag lowering, process-image bindings,
  bounded worker handoff, health/fault projection, and scan non-blocking rules.
- [x] `SPECARCH-NOW-003` Add an owning-spec routing rule so an MQTT production
  change paired only with `11-runtime-engine.md` fails the direct-change gate.
- [x] `SPECARCH-NOW-004` Add native tests for owning-spec routing, record the
  expected assertion red before implementation, and wire the test into CI.
- [x] `SPECARCH-NOW-005` Correct README ordering/coverage and the duplicate
  headings/numbering found in `04` and `09` without changing behavior.

## Staged structural program

- [ ] `SPECARCH-STAGE-001` Split `10-runtime-semantics.md` with behavior-locking
  link and specification-source tests before moving authority.
- [ ] `SPECARCH-STAGE-002` Split non-MQTT protocol and product-surface ownership
  out of `11-runtime-engine.md`, following
  `communication-solid-spec-ownership-audit.md` for communication domains.
- [ ] `SPECARCH-STAGE-003` Reorganize `14-lsp.md` around LSP/IDE ownership and
  remove duplicate language and PLCopen authorities.
- [ ] `SPECARCH-STAGE-004` Split `22-developer-workflows.md` into developer
  command/workspace, native test runner, package registry, PLCopen interchange,
  and release/CI authorities.
- [ ] `SPECARCH-STAGE-005` Split `25-vscode-product-contract.md` by independently
  tested product surface while preserving one product-shell index.
- [ ] `SPECARCH-STAGE-006` Move normative-looking backlog/status sections out of
  `13-debug-adapter.md` and `14-lsp.md` into tracked checklists.
- [ ] `SPECARCH-STAGE-007` Add link, duplicate-authority, and owner-route checks
  so future growth cannot recreate these mixed documents silently.

## SOLID/KISS/DRY acceptance

- [ ] `SPECARCH-SOLID-001` Every reorganized document has one named product or
  language owner and one native test family.
- [ ] `SPECARCH-SOLID-002` Shared contracts contain only shared abstractions;
  protocol/UI/tool-specific behavior depends on those abstractions rather than
  being copied into them.
- [ ] `SPECARCH-SOLID-003` Cross-document summaries are links, not duplicated
  normative prose.
- [ ] `SPECARCH-SOLID-004` Stable paths and anchors either remain valid or have
  explicit replacement links covered by a repository test.
- [ ] `SPECARCH-SOLID-005` No new specification file becomes a miscellaneous
  overflow document.
