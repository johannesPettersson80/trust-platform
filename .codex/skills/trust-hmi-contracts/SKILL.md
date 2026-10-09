---
name: trust-hmi-contracts
description: Guides work on truST's HMI - the HMI Builder editor, page files, the runtime display at /hmi, bindings, commands and writes, authorization, alarms and trends, the plant library and the HMI tests. Use for any HMI feature, fix or test, the HMI runtime API or write path, HMI visual design, plant-library function blocks, or HMI deployment.
---

# HMI

**Hard rule (AGENTS.md): all logic is in Rust.** Logic is anything whose result is stored, sent to
a runtime or PLC, or decides an engineering or operator outcome. TypeScript only presents (drawing,
layout, mouse interaction, view state) and forwards requests to Rust. Never add logic in
TypeScript. Existing TypeScript logic is debt to move into Rust, not to extend.

HMI logic is Rust: bindings, commands and writes, authorization, alarms, trends, equipment edits and
page validation live in the runtime or in `trust-lsp`. The HMI Builder webview and the `/hmi` page
only render and forward. Known TypeScript debt, listed in AGENTS.md: the HMI agent's authoring
engine in `agent/` (composition, edits, drafts, reviews, requests), which moves in checklist H4. Drawing,
drag, snapping and pipe routing are presentation and stay in TypeScript.

**Specification 39** (`docs/specs/39-hmi-builder.md`) defines the HMI:
- the HMI Builder editor in VS Code;
- the TOML page files;
- the runtime display at `/hmi`;
- bindings, commands and authorization;
- alarms and events;
- deployment, and the verification it requires.

**The plant library** is specified in `docs/specs/trust-plant-library.md` and documented in
`docs/guides/HMI_LIBRARY_GUIDE.md`.

**Older checkouts**: a checkout without specification 39 still carries the old HMI, which
specification 39 removed (`hmi/*.toml` widgets, `hmi.schema.get`, `hmi.values.get`, `hmi.write`).
There, `docs/guides/HMI_OPERATOR_FIRST_SPECIFICATION.md` applies.

## Product direction from the user

- **Simple**: every feature works with no configuration, using smart defaults. There is no settings
  jungle. Reuse faceplates and existing concepts before adding new ones. It is not a Siemens-style
  HMI builder.
- **Modern look**: dark first, with near-black surfaces, a vivid accent for live data and glowing
  status. No grey 1980s ISA-101 look. The light theme is supported too.
- **Alarms** follow ISA-18.2 and EEMUA 191:
  - an alarm is an instance of the object-oriented `Alarm` function block; extend it for your own
    detection;
  - priorities are High, Medium and Low;
  - informational messages and trips are events, not alarms.
- **Plant library**: native object-oriented Structured Text. It does not wrap older equipment code,
  and it has no "take control" ownership model.
- **Replacements first**: never remove a UI capability until its replacement exists.

## Architecture (specification 39, §17)

- **Separate modules**:
  - the page model and TOML serializer (TypeScript);
  - the drawing module (TypeScript), shared by the editor and the runtime display so that both draw
    the same page;
  - the HMI Builder editor: React panels and inspector, plus the drawing module;
  - the runtime loader and validator (Rust);
  - the runtime's command handling.
- **The runtime** provides:
  - pages and a live-value stream, with quality and timestamps;
  - writes checked for type, capacity, range and non-finite values;
  - a write policy with permission checks that denies by default;
  - a command tracker with outcomes and deduplication;
  - sessions;
  - alarm and trend state;
  - an audit log;
  - a bundle store with atomic activation and rollback.
- **The language server**:
  - checks page files and bindings;
  - serves the component catalogue and the variable tree;
  - handles rename and references across HMI files.
- **File size**: no source file grows beyond about 1,000 lines.

## Tests

Behaviour changes follow `trust-test-authoring`: specification 39 first, then the native test.

- **Runtime**, on the builder:
  - `cargo test -p trust-runtime --lib hmi::`
  - `cargo test -p trust-runtime --test hmi_check_command --test hmi_examples --test plant_examples --test web_hmi_display --test web_hmi_faceplates --test web_hmi_commands --test web_hmi_method_commands --test web_hmi_settings --test web_hmi_alarms --test web_hmi_history`
- **VS Code**: `TRUST_VSCODE_TEST_GREP='HMI'` selects the HMI Builder, HMI drawing and HMI display
  suites.
- **Browser**: run `scripts/hmi-screen-tests` with `npx playwright test`, after
  `cargo build -p trust-runtime --bin trust-runtime`. It covers:
  - the operator journey;
  - the simulation badge;
  - the reference-page screenshots in both themes;
  - the performance case.

  `npm run update-screens` approves a new look. Run it only after you have reviewed the images.
- **Live check**: open `/hmi` on a running example. Check the pages with live values, the equipment
  panels, the alarm list and the trends, and review the screenshots yourself.
- **What specification 39 requires**:
  - **§19** lists the required tests and these acceptance failure cases: a disconnect during a
    command, a denied write, an expired login, stale data, an invalid deployment, a rollback and
    rename recovery.
  - **§19.4**: the editor and the runtime draw the reference pages identically, with at most 32/255
    difference per colour channel.
  - **§19.6** sets the performance budget. It is measured on the AM6442 target and on a Raspberry Pi 5
    panel PC; builder measurements do not certify it.
