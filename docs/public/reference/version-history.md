# Version History

## Current Baseline

- current repository baseline: `v0.24.13`
- public docs in this tree describe the `v0.24.13` workspace version unless a
  page explicitly marks a feature as experimental, roadmap, or target-specific

## What Changed Recently

### `v0.24.x`

- Runtime release archives include `trust-dev` as the developer/workbench CLI.
  `trust-dev agent serve` now owns the external agent JSON-RPC server and
  `trust-dev commit`, `trust-dev docs`, and `trust-dev test` own the project
  commit, ST API documentation, and ST test workflows, while the matching
  `trust-runtime` commands remain deprecated forwarding aliases during the
  product/workbench CLI split.
- MQTT I/O now has explicit TLS/mTLS configuration: `tls = true` requires a CA
  trust file, optional client certificate/key files enable mTLS, `mqtts://` and
  `ssl://` broker schemes imply TLS, and remote plaintext MQTT remains gated by
  `allow_insecure_remote = true`.
- Dependency hygiene now has project `cargo deny` policy, explicit
  advisory/allowlist metadata, workspace-scoped `cargo machete` cleanup,
  a Rust `1.95` MSRV baseline, patched `time` 0.3.47, `ratatui` 0.30, tracked
  `Cargo.lock`, Rust `1.95` `is_multiple_of` cleanup for runtime modulo checks,
  and full-map doctor reporting for the remaining policy-managed transitive
  advisories.
- OSCAT OOP reset behavior, converter parity, API naming, dependency
  alias, and comparison examples were hardened after external review.
- OSCAT OOP example validation keeps all catalog/layout/pattern checks in the
  default Rust suite, while the full 98-project runtime CLI execution sweep is
  an explicit ignored gate for release or targeted OSCAT validation.
- HIR default-initializer analysis now rejects non-repeat call expressions used
  as array defaults and validates repeated array defaults against the element
  type.
- Parser recovery for malformed aggregate/positional initializers now uses
  bounded declaration-aware helpers and mutation-checked recovery tests so bad
  initializers do not consume following declarations.
- The OSCAT OOP test fixture now covers reset, multi-scan parity,
  invalid-limit rejection, FIFO ordering, and version lookup through classic
  OSCAT.
- OSCAT OOP now includes the v1.0 component surface for additional
  controllers, filters, generators, memory, logic, measuring, calendar/RTC,
  selected device-driver, and building-control objects, plus 49 classic/OOP
  comparison pairs: 27 hand-written process-first industrial pattern scenarios,
  20 compact component-composition showcases, and 2 compact pattern showcases
  covering state machines, alarm handling, historian/logging records,
  communication boundaries, and named OOP patterns.

### `v0.23.x`

- PLCopen Motion now has an object-oriented companion package with `itfAxis`,
  command objects, Structured Text unit tests, and five runnable OOP motion
  examples.
- OSCAT now has an object-oriented Components companion package with
  Structured Text parity tests, public docs, and 20 classic/components
  comparison scenarios.
- The public docs now define the default truST Structured Text naming standard
  for new APIs, variables, constants, examples, and inherited-symbol
  exceptions.

### `v0.22.x`

- `One Project, Every Surface` and `truST Mesh` became named public concepts
- the public docs were collapsed to six user-facing doors: `What Is truST?`,
  `Install`, `Program`, `Run`, `Hardware`, and `Reference`
- Migrate now has canonical ecosystem pages under `migrate/*`
- Editor AI tools and the Agent API have separate public scope boundaries
- public docs checks now guard nav coverage, snippet H1 collisions, list
  spacing, assets, links, and search expectations

### `v0.21.x` and earlier

- public docs moved to a question-driven site structure under `docs/public/`
- agent and harness surfaces became public documented contracts
- diagnostics gained a dedicated public code reference
- Browser IDE, HMI, examples, and operate runbooks were aligned into the docs
  site

## Reading The Version History

- if you are returning after a few weeks or months, start here
- then open the full [Changelog](../changelog.md)
- if you are upgrading an installed runtime, also read
  [Upgrade](../operate/upgrade.md)

## Related

- [Changelog](../changelog.md)
- [Upgrade](../operate/upgrade.md)
- [API Lifecycle And Deprecation](api-lifecycle-and-deprecation.md)
