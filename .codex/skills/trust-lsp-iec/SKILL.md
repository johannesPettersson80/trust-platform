---
name: trust-lsp-iec
description: Guides IEC 61131-3 Structured Text language work in truST - lexer, parser, types, semantics, standard functions and function blocks, OOP, namespaces and editor features - checked against the standard. Use when changing language behaviour or the standard library, editing language chapters in docs/specs, or deciding whether something belongs in docs/IEC_DECISIONS.md, docs/IEC_DEVIATIONS.md or a product specification.
---

# IEC 61131-3 language work

**Hard rule (AGENTS.md): all logic is in Rust.** Logic is anything whose result is stored, sent to
a runtime or PLC, or decides an engineering or operator outcome. TypeScript only presents (drawing,
layout, mouse interaction, view state) and forwards requests to Rust. Never add logic in
TypeScript. Existing TypeScript logic is debt to move into Rust, not to extend.

Language behaviour is computed in the Rust crates named below; the extension only shows the
language server's answers.

## Workflow

1. Identify the area and its specification in `docs/specs/` (`docs/specs/README.md` maps them).
   The areas are lexing, parsing, types, semantics, standard functions and function blocks, OOP,
   namespaces, runtime-assisted language features and editor UX.
2. Find the IEC section or table, and cite it in the specification edit.
3. Write the focused test at the closest layer together with the change (`trust-test-authoring`).
   The layer can be syntax, HIR, IDE, LSP, runtime or extension.
4. Implement in the owning crate:
   - `trust-syntax`: lexer and parser;
   - `trust-hir`: types, semantics and standard semantics;
   - `trust-ide`: diagnostics, completion and hover;
   - `trust-lsp`: wiring;
   - `trust-runtime` or `trust-debug`: when the language server uses runtime data.
5. A new language feature also extends `crates/trust-runtime/tests/fixtures/complete_program/`. Then
   run `cargo test -p trust-runtime --test complete_program`.
6. If you touched standard functions, update `docs/specs/coverage/standard-functions-coverage.md`.
7. For a change visible in the editor, run the VS Code extension tests. Update `editors/vscode/README.md`
   when the user-facing surface changes.

## Where a decision is recorded

- **`docs/IEC_DEVIATIONS.md`** is only for cases where truST intentionally conflicts with, omits or
  relaxes a normative IEC requirement. Before adding an entry:
  1. cite the exact section or table;
  2. state the requirement;
  3. state truST's behaviour;
  4. explain the concrete conflict.

  If you cannot state a conflict, there is no deviation. Existing entries are not precedent.
- **`docs/IEC_DECISIONS.md`** records the reading chosen where the IEC text genuinely allows several,
  with the citation.
- **The relevant product specification** records behaviour that IEC does not govern, even when it is
  intentional and tested:
  - IEC-silent or out-of-scope behaviour;
  - implementation-specific, platform, runtime or CLI behaviour;
  - truST-only APIs.

  Examples are a runtime API default, a file format, a protocol version or a tool limit.
- **PLCopen Motion** choices go in `docs/PLCOPEN_DECISIONS.md` and `docs/PLCOPEN_DEVIATIONS.md`.

## The standard

- **Sources**: the authoritative source is `docs/internal/standards/IEC 61131-3_2013 Ed3.pdf`. For
  searching (`rg -n`), use `docs/internal/standards/iec61131-3.txt`. Neither file is in git.
- **If both are missing**: when `~/Downloads/iec-61131-3.ocr.txt` exists, copy it to
  `docs/internal/standards/iec61131-3.txt`. Otherwise, state that the standard is unavailable before
  claiming conformance.
- **Extract one page**:
  `pdftotext -f <page> -l <page> "docs/internal/standards/IEC 61131-3_2013 Ed3.pdf" - | sed -n '1,200p'`
- **Key tables**:

  | Tables | Content |
  | --- | --- |
  | 1–9 | lexical elements and literals |
  | 10 and figure 5 | elementary types and the generic hierarchy |
  | 13–16 | variables, direct addressing, arrays |
  | 19, 40, 47, 48, 51 | FUNCTION, FUNCTION_BLOCK, PROGRAM, CLASS, INTERFACE |
  | 22–36 | standard functions and conversions |
  | 43–46 | standard function blocks |
  | 71 | operators and their precedence |
  | 72 | statements |
- **More references**: `docs/internal/standards/IEC_ST_FEATURE_MATRIX.md`,
  `docs/specs/10-runtime-semantics.md` and `docs/internal/testing/checklists/lsp.md`.
