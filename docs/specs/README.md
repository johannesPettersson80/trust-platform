# IEC 61131-3 Specifications

This directory contains IEC 61131-3 Structured Text (ST) specification extracts (01-09),
the consolidated platform/runtime/tooling spec (10-runtime), and Ladder Diagram (LD)
specification documents split into:

- normative LD language semantics (`11-ladder-diagram`)
- truST LD implementation profile (`12-ladder-profile-trust`)
- visual-editor runtime unification contract (`13-visual-editors-runtime-unification`)

## Document Index

| File | Description | Relevant Crate |
|------|-------------|----------------|
| [01-lexical-elements.md](01-lexical-elements.md) | Character set, identifiers, keywords, comments, pragmas, literals | trust-syntax (lexer) |
| [02-data-types.md](02-data-types.md) | Elementary types, generic types, user-defined types, type conversion | trust-hir (types) |
| [03-variables.md](03-variables.md) | Variable declarations, qualifiers, access specifiers, direct addressing | trust-hir (symbols) |
| [04-pou-declarations.md](04-pou-declarations.md) | FUNCTION, FUNCTION_BLOCK, PROGRAM, CLASS, INTERFACE, METHOD, NAMESPACE | trust-hir |
| [05-expressions.md](05-expressions.md) | Operators, precedence, evaluation rules | trust-syntax (parser), trust-hir (type check) |
| [06-statements.md](06-statements.md) | Assignment, control flow, iteration statements | trust-syntax (parser) |
| [07-standard-functions.md](07-standard-functions.md) | Type conversion, numerical, string, date/time functions | trust-hir |
| [08-standard-function-blocks.md](08-standard-function-blocks.md) | Bistable, edge detection, counter, timer FBs | trust-hir |
| [09-semantic-rules.md](09-semantic-rules.md) | Scope rules, error conditions, OOP rules | trust-hir |
| [10-runtime.md](10-runtime.md) | Runtime bytecode VM + helper-evaluation + debugger + LSP/IDE tooling spec | trust-runtime, trust-debug, trust-lsp |
| [11-ladder-diagram.md](11-ladder-diagram.md) | Normative IEC-aligned LD language semantics and conformance rules | trust-runtime, trust-lsp, editors/vscode |
| [12-ladder-profile-trust.md](12-ladder-profile-trust.md) | truST LD schema/runtime/editor profile and interoperability constraints | trust-runtime, trust-lsp, editors/vscode |
| [13-visual-editors-runtime-unification.md](13-visual-editors-runtime-unification.md) | Shared ST-backed runtime/debug command path for Ladder/Statechart/Blockly | editors/vscode, trust-debug, trust-runtime |

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
- Runtime, debugger, and tooling integration (see `10-runtime.md`)
- Ladder Diagram (LD) normative semantics (see `11-ladder-diagram.md`)
- Ladder Diagram (LD) implementation profile and interop constraints (see `12-ladder-profile-trust.md`)
- Visual editor runtime/debug ST-path unification contract (see `13-visual-editors-runtime-unification.md`)

### Not Covered (Out of Scope)

- Instruction List (IL) - Deprecated in Edition 3
- Function Block Diagram (FBD) - Graphical language
- Sequential Function Chart (SFC) - Partially relevant, not ST-specific
- Configuration and resource management details
- Communication function blocks

## Usage Guide

For project configuration, runtime integration, debugger behavior, and LSP/IDE tooling
notes, start with `docs/specs/10-runtime.md`.

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
| Table 19 | FUNCTION declaration | 04-pou-declarations.md |
| Table 22-27 | Type conversion functions | 07-standard-functions.md |
| Table 28-36 | Standard functions | 07-standard-functions.md |
| Table 40 | FUNCTION_BLOCK declaration | 04-pou-declarations.md |
| Table 43 | Bistable FBs | 08-standard-function-blocks.md |
| Table 44 | Edge detection FBs | 08-standard-function-blocks.md |
| Table 45 | Counter FBs | 08-standard-function-blocks.md |
| Table 46 | Timer FBs | 08-standard-function-blocks.md |
| Section 8.2 | Ladder Diagram (LD) semantics | 11-ladder-diagram.md |
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
