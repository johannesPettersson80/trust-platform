# LSP

### Document Information

| Property | Value |
|----------|-------|
| Version | 0.1.0 |
| Status | Draft |
| Last Updated | 2026-01-30 |
| Author | truST Contributors |

### Table of Contents

1. [Overview](#1-overview)
2. [Architecture](#2-architecture)
3. [Lexer Specification](#3-lexer-specification)
4. [Parser Specification](#4-parser-specification)
5. [Semantic Analysis](#5-semantic-analysis)
6. [IDE Features](#6-ide-features)
7. [LSP Protocol](#7-lsp-protocol)
8. [Runtime & Debugger](#8-runtime-debugger)
9. [Error Handling](#9-error-handling)
10. [Performance Requirements](#10-performance-requirements)
11. [Testing Strategy](#11-testing-strategy)
12. [Current Implementation Status](#12-current-implementation-status)

---

### 1. Overview

#### 1.1 Purpose

truST LSP is a Language Server Protocol implementation for IEC 61131-3 Structured Text (ST). It provides IDE features including diagnostics, completion, navigation, and refactoring for ST source code. The workspace also contains the
ST runtime, bytecode format, and debug adapter used for execution and testing.

#### 1.2 Scope

This specification covers:
- Lexical analysis of ST source code
- Syntactic analysis and CST construction
- Semantic analysis including type checking
- IDE feature implementations
- LSP protocol integration
- Runtime execution and bytecode decoding
- Debug adapter behavior and control protocols

#### 1.3 Target Standard

Primary: IEC 61131-3 Edition 3.0 (2013)

With extensions for:
- CODESYS v3.5
- Beckhoff TwinCAT 3
- Siemens TIA Portal (partial)

#### 1.4 Design Goals

1. **Correctness** - Accurately parse and analyze valid ST code
2. **Error Tolerance** - Provide useful feedback even for invalid code
3. **Performance** - Sub-100ms response times for interactive features
4. **Incrementality** - Re-analyze only what changed
5. **Extensibility** - Support vendor-specific extensions

---

### 2. Architecture

#### 2.1 Crate Structure

```
trust-platform (workspace)
├── trust-syntax      # Lexing and parsing
├── trust-hir         # High-level IR and semantic analysis
├── trust-ide         # IDE feature implementations
├── trust-lsp         # LSP protocol layer
├── trust-runtime     # Runtime execution engine + bytecode
└── trust-debug       # Debug adapter (DAP)
```

#### 2.2 Data Flow

```
Source Text
    │
    ▼
┌─────────┐
│  Lexer  │  → Token Stream
└─────────┘
    │
    ▼
┌─────────┐
│ Parser  │  → Concrete Syntax Tree (CST)
└─────────┘
    │
    ▼
┌─────────┐
│   HIR   │  → High-level IR + Symbol Table
└─────────┘
    │
    ▼
┌─────────┐
│   IDE   │  → Completions, Diagnostics, etc.
└─────────┘
    │
    ▼
┌─────────┐
│   LSP   │  → JSON-RPC Responses
└─────────┘
```

Runtime and debugger behavior are specified in:
- `docs/specs/10-runtime-semantics.md`

#### 2.3 Key Dependencies

| Crate | Purpose | Version |
|-------|---------|---------|
| logos | Lexer generation | 0.14 |
| rowan | Lossless syntax trees | 0.15 |
| salsa | Incremental query engine | 0.26 |
| tower-lsp | LSP framework | 0.20 |

#### 2.4 Concurrency Model

- Single-threaded analysis (Salsa-backed source/parse/file symbols/`analyze`/diagnostics/`type_of`)
- Async I/O for LSP communication (tokio)
- Document store protected by RwLock

---

### 3. Lexer Specification

The LSP layer consumes the same lexer/token model defined in
`01-lexical-elements.md`. Keyword reservation, literals, identifiers, direct
addresses, and lexer diagnostics are owned by that language spec; this LSP spec
only defines how lexer-backed editor features surface them in diagnostics,
semantic tokens, completion, and navigation.

### 4. Parser Specification

The LSP layer consumes the concrete syntax and statement/declaration grammar
owned by `04-pou-declarations.md`, `05-expressions.md`, and
`06-statements.md`. This spec does not restate the grammar; it defines how the
editor-facing pipeline uses parser output for recovery, syntax diagnostics,
outline/navigation, and incremental document updates.

### 5. Semantic Analysis

Type rules, name resolution, assignability, and most semantic diagnostics are
owned by `02-data-types.md`, `03-variables.md`, `04-pou-declarations.md`, and
`09-semantic-rules.md`. This LSP spec owns how that analysis is queried and
presented through IDE features such as diagnostics, hover, rename, and
workspace indexing.

### 6. IDE Features

#### 6.1 Completion

##### 6.1.1 Trigger Points

- After `.` - member completion
- After `:` - type completion
- After `(` / inside call arguments - parameter-name completions for formal calls (`name :=` / `name =>`) with direction-aware binding (IEC 61131-3 Ed.3, 6.6.1.4.2; Table 50/71)
- After typed literal prefixes (`T#`, `DATE#`, `TOD#`, `DT#`, etc.) - range-aware typed literal snippets with format hints (IEC 61131-3 Ed.3, 6.1.5; Tables 5-9)
- Start of line - statement/keyword completion
- After `VAR` etc. - variable name suggestions

##### 6.1.2 Completion Kinds

| Context | Suggestions |
|---------|-------------|
| After `.` on FB | Properties, methods |
| After `.` on STRUCT | Fields |
| After `:` | Types in scope |
| Call arguments | Formal parameter names + in-scope expressions (IEC 61131-3 Ed.3, 6.6.1.4.2; Table 50/71) |
| Statement start | Keywords, variables, standard functions (IEC 61131-3 Ed.3, Tables 22-36) |
| Expression | Variables, literals, standard functions/FBs with IEC docs (IEC 61131-3 Ed.3, Tables 22-36, 43-46) |

#### 6.2 Diagnostics

Diagnostics are delivered via both push (`textDocument/publishDiagnostics`) and pull (`textDocument/diagnostic`, `workspace/diagnostic`) APIs. Pull diagnostics return stable `resultId` values derived from content + diagnostic hashes, allowing unchanged responses when the client supplies the previous ID. On configuration/profile changes or workspace file updates, the server requests a refresh (`workspace/diagnostic/refresh`) when supported.
Warning diagnostics can be filtered via `[diagnostics]` configuration; rule packs can preconfigure defaults and severity overrides can promote warning codes to errors. Vendor profiles may adjust defaults to mirror tooling expectations (e.g., CASE/implicit conversion warnings per IEC 61131-3 Ed.3 §7.3.3.3.3 and §6.4.2).
Project configuration diagnostics are reported for `trust-lsp.toml` to flag library dependency issues (missing libraries or version mismatches).
External diagnostics can be merged from `[diagnostics].external_paths` JSON files, and optional fix payloads are exposed as quick-fix code actions.

##### 6.2.1 Syntax Errors

- Missing tokens
- Unexpected tokens
- Unclosed blocks

##### 6.2.2 Semantic Errors

- Undefined variable
- Type mismatch
- Duplicate declaration
- Invalid assignment target
- Missing return statement
- Task configuration errors (missing/invalid PRIORITY) and unknown task references in PROGRAM configs (IEC 61131-3 Ed.3 §6.2; §6.8.2; Table 62)

##### 6.2.3 Warnings

- Unused variable
- Unused parameter
- Missing ELSE in CASE (IEC 61131-3 Ed.3, 7.3.3.3.3)
- Implicit type conversion (IEC 61131-3 Ed.3, 6.4.2)
- Non-determinism checks for time/date usage and direct I/O bindings (tooling lint; IEC 61131-3 Ed.3 §6.4.2 Table 10; §6.5.5 Table 16)
- Shared global access across tasks with writes (tooling lint; IEC 61131-3 Ed.3 §6.5.2.2 Tables 13–16; §6.2/§6.8.2 Table 62)

##### 6.2.4 Diagnostic Explainability

When a diagnostic is mapped to an IEC reference, the LSP payload includes:
- `codeDescription.href` → file URL to the relevant `docs/specs/*.md` (when present in the workspace)
- `data.explain` → `{ iec: "...", spec: "docs/specs/..." }`

Initial explainer coverage:

| Codes | IEC reference | Spec doc |
|------|---------------|----------|
| E001–E003 | IEC 61131-3 Ed.3 §7.3 | `docs/specs/06-statements.md` |
| E101/E104/E105/W001/W002/W006 | IEC 61131-3 Ed.3 §6.5.2.2 | `docs/specs/09-semantic-rules.md` |
| E102 | IEC 61131-3 Ed.3 §6.2 | `docs/specs/02-data-types.md` |
| E103/E204/E205/E206/E207 | IEC 61131-3 Ed.3 §6.6.1 | `docs/specs/04-pou-declarations.md` |
| E106 | IEC 61131-3 Ed.3 §6.1.2 | `docs/specs/01-lexical-elements.md` |
| E201/E202/E203 | IEC 61131-3 Ed.3 §7.3.2 | `docs/specs/05-expressions.md` |
| E301/E302 | IEC 61131-3 Ed.3 §7.3.1 | `docs/specs/09-semantic-rules.md` |
| E303/E304 | IEC 61131-3 Ed.3 §6.2.6 | `docs/specs/02-data-types.md` |
| W004 | IEC 61131-3 Ed.3 §7.3.3.3.3 | `docs/specs/06-statements.md` |
| W005 | IEC 61131-3 Ed.3 §6.4.2 | `docs/specs/02-data-types.md` |
| W008/W009 | Tooling quality lint (non-IEC) | `docs/specs/09-semantic-rules.md` |
| W010 | Tooling lint; TIME/DATE types per IEC 61131-3 Ed.3 §6.4.2 (Table 10) | `docs/specs/09-semantic-rules.md` |
| W011 | Tooling lint; Direct variables per IEC 61131-3 Ed.3 §6.5.5 (Table 16) | `docs/specs/09-semantic-rules.md` |
| W012 | Tooling lint; shared global access across tasks (IEC 61131-3 Ed.3 §6.5.2.2 Tables 13–16; §6.2/§6.8.2 Table 62) | `docs/specs/09-semantic-rules.md` |
| W013/W014 | Tooling numeric-hazard lints (non-IEC) | `docs/specs/09-semantic-rules.md` |
| L001–L003 | Tooling config lint (non-IEC) | `docs/specs/10-runtime-semantics.md` |

For access-specifier violations reported under E202 (e.g., PRIVATE/PROTECTED/INTERNAL access),
the explainer is mapped to IEC 61131-3 Ed.3 §6.6.5 (Table 50) in `docs/specs/09-semantic-rules.md`.

Diagnostics without a mapping return only `code` + `message` until their IEC references are added.

##### 6.2.5 Severity Levels and Warning Policy

| Severity | Description | Examples |
|----------|-------------|----------|
| Error | Must be fixed; compilation or validation cannot proceed | Type mismatch, undefined reference, invalid task binding |
| Warning | Likely bug or portability issue; build may still proceed | Unused variable, implicit conversion, unreachable code |
| Info | Supplemental context | vendor-profile hints, migration notes |
| Hint | Non-blocking editor guidance | optional quick-fix suggestions |

Recommended warning groups:

- `W003` unreachable code after unconditional terminators or constant-false branches
- `W004` missing `ELSE` in `CASE`
- `W005` implicit conversion
- `W008` cyclomatic complexity quality lint
- `W009` unused POU quality lint
- `W010`/`W011` non-deterministic time/date and direct-I/O usage
- `W012` shared global access across scheduled tasks
- `W013`/`W014` numeric hazard lints

Workspace warning policy is configured through `trust-lsp.toml` `[diagnostics]`.
Profiles may override severities to mirror vendor expectations, but the
canonical code list and default severity guidance live in this spec.

#### 6.3 Navigation

##### 6.3.1 Go to Definition

- Variables → declaration
- `VAR_EXTERNAL` resolves to the matching `VAR_GLOBAL` declared in the associated program/configuration/resource scope (IEC 61131-3 Ed.3, §6.5.2.2; Tables 13–16, Table 47 feature 8a)
- truST vendor-parity global access also resolves bare global names directly, and qualified names such as `GVL.shared` resolve against namespaced GVL entries recorded in runtime storage.
- Types → type definition
- Methods → method definition
- Properties → property definition

##### 6.3.2 Find References

- All usages of a symbol
- Include/exclude declaration
- Filter by read/write

##### 6.3.3 Document Symbols

- Flat list of declarations

#### 6.4 Refactoring

##### 6.4.1 Rename

- All references updated (workspace-wide)
- Preview changes
- Namespace path moves via dotted rename or refactor action (updates namespace declarations, `USING`, qualified names, and namespace-qualified field access; relocation across files moves the namespace block to a derived target file and removes the source file when empty; default target path maps `Namespace.Path` → `<workspace>/Namespace/Path.st` unless an explicit URI is provided) (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)
- VS Code surfaces namespace relocation via `Structured Text: Move Namespace`, prompting for the new path and optional target file (invokes `trust-lsp.moveNamespace`) (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)

##### 6.4.2 Code Actions / Quick Fixes

- Create missing VAR declarations for undefined identifiers (IEC 61131-3 Ed.3, 6.5.3; Tables 13-14)
- Create missing TYPE definitions for undefined types (IEC 61131-3 Ed.3, 6.5.2; Table 11)
- Insert missing END_* blocks (IEC 61131-3 Ed.3, 7.3; Table 72)
- Insert missing RETURN in FUNCTION (IEC 61131-3 Ed.3, 7.3.3.3.2; Table 72)
- Convert formal ↔ positional call style (IEC 61131-3 Ed.3, 6.6.1.4.2; Table 50)
- Reorder mixed calls to positional-first argument order (IEC 61131-3 Ed.3, 6.6.1.4.2; Table 50)
- Move namespace path (refactor action invoking rename UI) (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)
- Move namespace path (execute command; relocates declarations across files) (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)
- Move namespace quick fix (VS Code lightbulb on `NAMESPACE`/`USING` lines; invokes `trust-lsp.moveNamespace` via UI command) (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)
- Qualify ambiguous namespace references when multiple USING directives apply (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66)
- Fix VAR_OUTPUT binding operators / add missing OUT bindings (IEC 61131-3 Ed.3, 6.6.1.2.2; Table 71)
- Wrap implicit conversions using standard conversion functions (IEC 61131-3 Ed.3, Tables 22–27)
- Generate stub implementations for missing interface methods/properties from IMPLEMENTS clauses (IEC 61131-3 Ed.3, 6.6.5–6.6.6; Tables 50–51)
- Inline variable/constant with safety checks (const-expression analysis, no writes, cross-file constants when safe) (IEC 61131-3 Ed.3, 6.5.1–6.5.2; Tables 13–14)
- Extract method/property/function from a selection (method/property in CLASS/FB, function in POU body) with inferred VAR_INPUT/VAR_IN_OUT parameters; expression selections extract a FUNCTION returning the inferred expression type (IEC 61131-3 Ed.3, 6.6.5; Table 50 for methods/properties; 6.6.2.2; Table 19 for functions)
- Convert FUNCTION ↔ FUNCTION_BLOCK with safe call-site updates (supports qualified names and assignment/return expression sites; no recursive calls; FUNCTION→FB requires no existing VAR_OUTPUT when a return type is present; FB→FUNCTION requires a single VAR_OUTPUT and no type references/instances) (IEC 61131-3 Ed.3, 6.6.2.2; Table 19 and 6.6.3.2; Table 40)
- Remove unused variables/parameters

##### 6.4.3 Future

- Change signature

#### 6.5 Hover Information

Hover content includes:
- Symbol signature + visibility/modifiers (IEC 61131-3 Ed.3, 6.6.5; Table 50)
- Standard function/FB documentation (IEC 61131-3 Ed.3, Tables 22–36, 43–46)
- Namespace/USING resolution details (IEC 61131-3 Ed.3, 6.6.4; Tables 64–66)
- Typed literal guidance for TIME/DATE/TOD/DT prefixes (IEC 61131-3 Ed.3, 6.1.5; Tables 5–9)
- Configuration/Resource/Task declarations show task scheduling inputs and program bindings (IEC 61131-3 Ed.3 §6.2; §6.8.2; Table 62)

```
motorSpeed : REAL
───────────────────
Variable (VAR_INPUT)
Declared in: FB_Motor

The target speed for the motor in RPM.
Range: 0.0 to 3000.0
```

---

### 7. LSP Protocol

#### 7.1 Supported Capabilities

| Capability | Method | Status | Notes |
|------------|--------|--------|-------|
| Text Sync | `textDocument/didOpen`, etc. | ✅ | Incremental sync with full-change fallback |
| Diagnostics | `textDocument/publishDiagnostics` | ✅ | Parse + semantic diagnostics (undefined names, type mismatch, invalid assignments) |
| Pull Diagnostics | `textDocument/diagnostic` | ✅ | Per-file result IDs; unchanged when `previousResultId` matches |
| Workspace Diagnostics | `workspace/diagnostic` | ✅ | Full/unchanged reports per document across indexed workspace |
| Diagnostics Refresh | `workspace/diagnostic/refresh` | ✅ | Server requests refresh on config/profile or workspace changes (client-supported) |
| Completion | `textDocument/completion` | ✅ | Scope-aware + member access + parameter-name completion + standard docs |
| Hover | `textDocument/hover` | ✅ | Shows type + qualifiers |
| Signature Help | `textDocument/signatureHelp` | ✅ | Call signatures with active parameter |
| Definition | `textDocument/definition` | ✅ | Project-wide (workspace indexed; file watching updates) |
| Declaration | `textDocument/declaration` | ✅ | Same target as definition |
| Type Definition | `textDocument/typeDefinition` | ✅ | Type/alias definition lookup |
| Implementation | `textDocument/implementation` | ✅ | Interface implementers (project-wide) |
| References | `textDocument/references` | ✅ | Symbol-aware (workspace indexed; no text fallback); work-done progress + partial results when client provides tokens |
| Document Highlight | `textDocument/documentHighlight` | ✅ | Highlight reads/writes in current document |
| Symbols | `textDocument/documentSymbol` | ✅ | Flat list |
| Workspace Symbols | `workspace/symbol` | ✅ | Multi-root symbol federation with per-root priority/visibility; work-done progress + partial results when client provides tokens |
| File Rename | `workspace/willRenameFiles` | ✅ | Renames single top-level POU/namespace when file stem changes; updates references and USING directives for that namespace (IEC 61131-3 Ed.3, 6.1.2; 6.6.4; Tables 64-66) |
| Rename | `textDocument/rename` | ✅ | Symbol-aware; workspace edits; renames the declaring file when renaming the single primary POU whose identifier matches the file stem (IEC 61131-3 Ed.3, 6.1.2) |
| Semantic Tokens | `textDocument/semanticTokens` | ✅ | Full + range + delta; classified by symbol kind/modifiers |
| Semantic Tokens Refresh | `workspace/semanticTokens/refresh` | ✅ | Server requests refresh on config/profile changes (client-supported) |
| Folding Range | `textDocument/foldingRange` | ✅ | CST-based region folding |
| Selection Range | `textDocument/selectionRange` | ✅ | CST-based hierarchical selection ranges |
| Linked Editing | `textDocument/linkedEditingRange` | ✅ | Identifier-linked ranges in document (IEC 61131-3 Ed.3, 6.1 identifiers) |
| Document Link | `textDocument/documentLink` | ✅ | Links for `USING` directives and `trust-lsp.toml` path entries (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66) |
| Inlay Hints | `textDocument/inlayHint` | ✅ | Parameter-name hints for positional calls (IEC 61131-3 Ed.3, 6.6.1.2.2; Table 71) |
| Inline Values | `textDocument/inlineValue` | ✅ | Constant/enum references show initializer text; runtime values surfaced via debug control for locals/globals/retain when configured (IEC 61131-3 Ed.3, 6.5.1–6.5.2; Tables 13–14) |
| Code Lens | `textDocument/codeLens` | ✅ | Reference count lenses for POU declarations |
| Call Hierarchy | `textDocument/prepareCallHierarchy` | ✅ | Incoming/outgoing call graph for POU declarations |
| Type Hierarchy | `textDocument/prepareTypeHierarchy` | ✅ | Class/FB/interface supertypes + subtypes (IEC 61131-3 Ed.3, 6.6.5) |
| Formatting | `textDocument/formatting` | ✅ | Indentation + spacing + alignment + wrapping (configurable) |
| Range/On-Type Formatting | `textDocument/rangeFormatting`, `textDocument/onTypeFormatting` | ✅ | Line-based formatting using document formatter |
| Configuration | `workspace/didChangeConfiguration` | ✅ | Settings stored (formatting/indexing); project config file is separate |
| Code Actions | `textDocument/codeAction` | ✅ | Quick fixes for unused symbols, missing END_* / RETURN, call style conversion, namespace disambiguation, implicit conversion, etc. |
| Execute Command | `workspace/executeCommand` | ✅ | `trust-lsp.moveNamespace` for namespace relocation across files (IEC 61131-3 Ed.3, 6.6.4; Tables 64-66); `trust-lsp.projectInfo` surfaces build flags, targets, and library dependency graph |

#### 7.2 Document Synchronization

- Incremental sync using `TextDocumentContentChangeEvent` ranges.
- Full-document replacement is supported when the change range is omitted.

#### 7.3 Semantic Token Types

| Token Type | Usage |
|------------|-------|
| keyword | All ST keywords |
| type | Type names |
| variable | Variable names |
| property | Property names |
| method | Method names |
| function | Function names |
| parameter | Parameter names |
| number | Numeric literals |
| string | String literals |
| comment | Comments |
| operator | Operators |

#### 7.4 Semantic Token Modifiers

| Modifier | Usage |
|----------|-------|
| declaration | At declaration site |
| definition | At definition site |
| readonly | CONSTANT variables |
| static | VAR_STAT variables |
| modification | Write to variable |

#### 7.5 Formatting

- Indentation and token-based spacing normalization (operators/separators).
- VAR block `:` alignment across declarations.
- Assignment alignment for `:=` and `=>` within aligned blocks (range formatting expands to align pasted statement lists).
- Keyword casing (upper/lower/preserve), spacing style (spaced/compact), end keyword indentation (aligned/indented), and max line length are configurable; `vendor_profile` presets default indent width, spacing, and end keyword style for common IDEs.
- Block comment lines are left unchanged; line comments and pragma lines preserve inline spacing.
- String literal and pragma lines are excluded from assignment alignment and wrapping to preserve lexical content (IEC 61131-3 Ed.3, 6.1; Tables 4–7).
- Line endings are preserved (LF vs CRLF).
- Line-wrapping at commas honors `maxLineLength` and avoids comment/pragma/string lines (IEC 61131-3 Ed.3, 6.1; Tables 4–7).
- Range formatting expands to the nearest syntactic block (e.g., VAR blocks, IF/CASE loops, POU/method/property bodies) to avoid partial-block drift.
- VAR alignment respects manual grouping: blank lines or comment/pragma lines split alignment groups to preserve intentional spacing and comment anchors.
- Formatting config keys: `indentWidth`, `insertSpaces`, `keywordCase`, `spacingStyle`, `endKeywordStyle`, `alignVarDecls`, `alignAssignments`, `maxLineLength`.
- Vendor preset defaults (overrideable via config): `codesys`/`beckhoff`/`twincat`/`mitsubishi`/`gxworks3` use 4-space indents with spaced operators; `siemens` uses 2-space indents with compact operator spacing; all align `END_*` keywords by default.

#### 7.6 Project Configuration & Workspace Indexing

- Per-root project config file: `trust-lsp.toml`, `.trust-lsp.toml`, or `trustlsp.toml`.
- `[project]` supports `include_paths`, `library_paths`, `vendor_profile` (dialect + formatting presets), and `stdlib` selection.
- `stdlib` profiles: `full` (default), `iec` (IEC standard functions/FBs only; Tables 22–36, 43–46), `none` (no standard library completions/hover), or an explicit allow-list array.
- When `vendor_profile` is set and no explicit stdlib allow-list/profile is provided, the server defaults to the IEC profile for completions/hover.
- `[[libraries]]` entries include `name`, `path`, and optional `version` for external library indexing.
- `[dependencies]` supports local and git package references:
  - local: `Name = "path"` or `Name = { path = "...", version? = "..." }`
  - git: `Name = { git = "<url-or-local-repo>", rev? = "...", tag? = "...", branch? = "...", version? = "..." }`
- Intended usage split:
  - `[dependencies]` is for reusable truST ST packages that participate in source resolution, `trust-runtime build`, and `trust-dev test --project`.
  - `[[libraries]]` is for external/indexed library trees, stub packs, and attached vendor docs used for compatibility/indexing.
- Dependency pinning/lock behavior:
  - `rev`/`tag`/`branch` pin git dependencies explicitly.
  - `build.dependencies_locked = true` requires explicit pinning or a matching lock entry.
  - Resolver snapshots pinned sources to `build.dependency_lockfile` (default `trust-lsp.lock`) for reproducible resolution.
  - `build.dependencies_offline = true` disables clone/fetch and resolves from local cache + lock only.
- Basic supply-chain trust policy is configurable via `[dependency_policy]`:
  - `allowed_git_hosts = ["example.com"]` allow-list (empty = any host).
  - `allow_http` (default false), `allow_ssh` (default false).
- `[[libraries]]` can declare `dependencies` (array of `{ name, version? }`) to model library graphs; missing dependencies or version mismatches are reported as config diagnostics.
- Library/dependency graphs report missing references (L001), version mismatches (L002), conflicting declarations (L003), and dependency cycles (L004).
- `[[libraries]]` can declare `docs` (array of markdown files) to attach vendor library documentation to hover/completion. Each file uses `# SymbolName` headings followed by doc text.
- `[workspace]` controls multi-root federation: `priority` orders root results for workspace symbol search, and `visibility` (`public`, `private`, `hidden`) filters which roots participate when querying (private roots only appear for non-empty queries) (tooling behavior, non-IEC).
- `[build]` exposes project compile flags (`flags`), `defines`, and optional `target`/`profile` defaults.
- `[[targets]]` describes target profiles (`name`, `profile`, `flags`, `defines`) surfaced to LSP clients for toolchain selection.
- `[indexing]` budgets (`max_files`, `max_ms`) bound large workspace indexing.
- `[indexing]` cache options: `cache` (default true) enables persistent index caching across sessions; `cache_dir` overrides the cache location. Cache reuse checks file metadata and stored content hashes.
- `[indexing]` memory budget controls: `memory_budget_mb` caps closed-document index memory (MB) and `evict_to_percent` defines the LRU eviction target; evicted documents are reloaded on demand when accessed.
- `[indexing]` adaptive throttling: `throttle_idle_ms`, `throttle_active_ms`, `throttle_max_ms`, and `throttle_active_window_ms` pace background indexing based on recent editor activity and observed per-file work.
- `[runtime]` supports `control_endpoint` and optional `control_auth_token` for debug-assisted inline values.
- `[diagnostics]` toggles warning categories (`warn_unused`, `warn_unreachable`, `warn_missing_else`, `warn_implicit_conversion`, `warn_shadowed`, `warn_deprecated`, `warn_complexity`, `warn_nondeterminism`, `warn_numeric_hazards`) for vendor-dialect alignment (IEC 61131-3 Ed.3 §6.4.2; §7.3.3.3.3). Cyclomatic complexity warnings (W008) use a default threshold of 15; unused warnings (W001/W002/W009) cover variables, parameters, and top-level POUs; numeric hazard warnings (W013/W014) cover floating-point equality and literal zero divisors.
- `[diagnostics].rule_pack` presets safety-focused defaults (e.g., `iec-safety`, `siemens-safety`, `codesys-safety`, `beckhoff-safety`, `twincat-safety`, `mitsubishi-safety`, `gxworks3-safety`); explicit `warn_*` keys override pack defaults. `[diagnostics].severity_overrides` can promote specific warning codes to error severity (W004 missing ELSE per IEC 61131-3 Ed.3 §7.3.3.3.3; W005 implicit conversion per §6.4.2; W010 TIME/DATE nondeterminism per §6.4.2; W011 direct variables per §6.5.5; W014 literal division/modulo by zero guard).
- `[diagnostics].external_paths` lists JSON diagnostics payloads from external linters (optional per-diagnostic fix data yields quick-fix actions).
- Vendor diagnostic defaults: `siemens` disables Missing ELSE (W004) and implicit conversion (W005); `codesys`, `beckhoff`, `twincat`, `mitsubishi`, and `gxworks3` keep all warning categories enabled unless overridden in `[diagnostics]`.
- `[telemetry]` (opt-in) records aggregated feature usage + latency to JSONL (`enabled`, `path`, `flush_every`); payloads include event names and durations only (tooling behavior, non-IEC).
- Indexing progress is reported via `window/workDoneProgress` when supported by the client.
- Workspace indexing runs in the background; adaptive throttling yields between files to keep interactive edits responsive (tooling behavior, non-IEC).
- Stdlib selection currently filters standard function/FB docs and completions (IEC 61131-3 Ed.3, Tables 22–36, 43–46).

---

### 8. Runtime & Debugger

The workspace includes a runtime and debug adapter used for executing and testing ST programs.
The authoritative specifications for these components are:

- `docs/specs/10-runtime-semantics.md`

#### 8.1 Runtime

- Runtime execution is defined by the ST runtime specification, including task scheduling,
  process image semantics, retain behavior, and fault handling.
- Production runtimes are started via the CLI using the project folder (runtime bundle format)
  format (`trust-runtime` or `trust-runtime run --project`). Project folders can be generated by
  `trust-runtime build` (preferred) or CI tooling that emits STBC.

#### 8.2 Debugger

- Debug adapter behavior follows DAP and the ST debugger specification.
- Breakpoints and stepping are statement-based and use source locations from the compiler.

---

### 9. Error Handling

#### 9.1 Error Categories

```rust
enum DiagnosticSeverity {
    Error,      // Prevents compilation
    Warning,    // Potential issue
    Info,       // Informational
    Hint,       // Style suggestion
}

struct Diagnostic {
    range: TextRange,
    severity: DiagnosticSeverity,
    code: DiagnosticCode,
    message: String,
    related: Vec<RelatedInfo>,
}
```

#### 9.2 Error Codes

| Code | Category | Description |
|------|----------|-------------|
| E001 | Syntax | Unexpected token |
| E002 | Syntax | Missing token |
| E003 | Syntax | Unclosed block |
| E101 | Name | Undefined variable |
| E102 | Name | Duplicate declaration |
| E103 | Name | Cannot resolve type |
| E201 | Type | Type mismatch |
| E202 | Type | Invalid operation |
| E203 | Type | Incompatible assignment |
| W001 | Warning | Unused variable |
| W002 | Warning | Unreachable code |
| W003 | Warning | Implicit conversion |

---

### 10. Performance Requirements

#### 10.1 Latency Targets

| Operation | Target | Maximum |
|-----------|--------|---------|
| Keystroke response | < 16ms | 50ms |
| Completion list | < 50ms | 200ms |
| Go to definition | < 20ms | 100ms |
| Find references | < 100ms | 500ms |
| Full file diagnostics | < 200ms | 1000ms |

#### 10.2 Memory Targets

| Metric | Target |
|--------|--------|
| Per-file overhead | < 10x source size |
| Idle memory | < 100MB |
| Large project (100 files) | < 500MB |

#### 10.3 Optimization Strategies

1. **Incremental parsing** - Salsa invalidation on changed files (file-level granularity)
2. **Cross-file dependency tracking** - Salsa queries recompute only affected dependents
3. **Indexing budgets** - `max_files` / `max_ms` limits for large workspaces
4. **Expression-level type cache** - Cache `type_of` results by expression hash + scope, invalidated when symbol tables change
5. **Adaptive background indexing** - Per-file throttling based on recent editor activity and observed indexing cost
6. **Memory budgets** - Closed-document eviction with on-demand reload when over memory budget
7. **Progress reporting** - Work-done progress notifications during indexing
8. **Request prioritization** - Background workspace scans (`workspace/symbol`, `workspace/diagnostic`, cross-file references) are concurrency-limited to keep interactive requests responsive

---

### 11. Testing Strategy

#### 11.1 Test Categories

##### 11.1.1 Unit Tests

- Lexer token output
- Parser tree structure
- Type checker rules
- Symbol resolution

##### 11.1.2 Integration Tests

- Full file parsing
- Cross-file references
- LSP protocol compliance + golden handler responses
- Performance harness (ignored by default): hover/completion/rename budgets + large workspace indexing (Section 10 targets)
- VS Code extension integration tests for completion, formatting, and code actions (IEC 61131-3 Ed.3 §6.1-6.3; Tables 4-9; §6.5.2.2)
- Stdlib coverage check: ensures all IEC standard function/FB names appear in `docs/specs/coverage/standard-functions-coverage.md` (IEC 61131-3 Ed.3, Tables 22–36, 43–46)

##### 11.1.3 Snapshot Tests

- Parser output (insta)
- Diagnostic output
- Completion / signature / formatting results

#### 11.2 Test Corpus

```
tests/corpus/
├── declarations/
│   ├── variables.st
│   ├── types.st
│   └── functions.st
├── expressions/
│   ├── arithmetic.st
│   ├── logical.st
│   └── comparison.st
├── statements/
│   ├── if.st
│   ├── case.st
│   ├── for.st
│   └── while.st
├── function_blocks/
│   ├── basic.st
│   ├── inheritance.st
│   └── interfaces.st
└── errors/
    ├── syntax_errors.st
    └── type_errors.st
```

#### 11.3 Fuzzing

- AFL/libFuzzer for parser robustness
- Grammar-aware fuzzing for valid-ish input

#### 11.4 Benchmarks

- Large file parsing (10K+ lines)
- Completion response time
- Memory usage under load
- Benchmark evidence must record whether the runtime binary was built as a portable/generic release or with host-native CPU tuning.
- Official/shared release artifacts must remain portable; host-native builds (for example `-C target-cpu=native`) are opt-in benchmark/tuning artifacts only and must not be treated as the default cross-host baseline.

---

### 12. Current Implementation Status

#### 12.1 What's Implemented

- **Lexer**: Complete token set for IEC 61131-3 ST
- **Parser**: ST constructs parsed per specs (including ACTION blocks and AT addresses)
- **Symbol Table**: Scope-aware with namespaces and cross-file resolution
- **Type Registry**: Elementary, generic, and user-defined types (STRUCT/UNION/ENUM/ARRAY/STRING[n])
- **Hover**: Full implementation with type and qualifier display
- **Go to Definition**: Project-wide navigation (workspace indexed)
- **Document Symbols**: Flat list of declarations
- **Debugger (DAP)**: Core DAP adapter with breakpoints, stepping, scopes, variables, evaluate, logpoints

#### 12.2 Known Limitations

1. **Workspace**
   - Workspace indexing runs on initialize; on-disk changes are tracked via file watching when supported by the client (otherwise require reload)

2. **LSP**
   - Formatting does not wrap/reflow lines beyond operator spacing + VAR alignment
   - Folding ranges are coarse (node-based regions)
3. **Debugger**
   - VS Code extension wiring exists, but the manual test plan is still pending
   - Stepping is statement-level only; expressions are not single-stepped
   - Debug evaluation is restricted to side-effect-free expressions and a small pure stdlib whitelist
   - Hot reload is implemented via a custom request and supports per-resource
     reloads with retained globals preserved across warm restart (see DEV-024)
   - I/O forcing supports both input and output areas through the DAP/control
     bridge write path (see DEV-025)


---

### Appendix A: IEC 61131-3 Operator Precedence

| Precedence | Operators | Associativity |
|------------|-----------|---------------|
| 1 (lowest) | OR | Left |
| 2 | XOR | Left |
| 3 | AND, & | Left |
| 4 | =, <> | Left |
| 5 | <, >, <=, >= | Left |
| 6 | +, - | Left |
| 7 | *, /, MOD | Left |
| 8 | ** | Right |
| 9 (highest) | NOT, -(unary), +(unary) | Right |

---

### Appendix B: Type Hierarchy

```
ANY
├── ANY_DERIVED
│   ├── ANY_ELEMENTARY
│   │   ├── ANY_MAGNITUDE
│   │   │   ├── ANY_NUM
│   │   │   │   ├── ANY_REAL
│   │   │   │   │   ├── REAL
│   │   │   │   │   └── LREAL
│   │   │   │   └── ANY_INT
│   │   │   │       ├── ANY_SIGNED
│   │   │   │       │   ├── SINT
│   │   │   │       │   ├── INT
│   │   │   │       │   ├── DINT
│   │   │   │       │   └── LINT
│   │   │   │       └── ANY_UNSIGNED
│   │   │   │           ├── USINT
│   │   │   │           ├── UINT
│   │   │   │           ├── UDINT
│   │   │   │           └── ULINT
│   │   │   └── ANY_DURATION
│   │   │       ├── TIME
│   │   │       └── LTIME
│   │   ├── ANY_BIT
│   │   │   ├── BOOL
│   │   │   ├── BYTE
│   │   │   ├── WORD
│   │   │   ├── DWORD
│   │   │   └── LWORD
│   │   ├── ANY_STRING
│   │   │   ├── STRING
│   │   │   └── WSTRING
│   │   ├── ANY_DATE
│   │   │   ├── DATE
│   │   │   └── LDATE
│   │   └── ANY_DATE_AND_TIME
│   │       ├── DT
│   │       └── LDT
│   └── USER_DEFINED
│       ├── STRUCT
│       ├── ENUM
│       ├── ARRAY
│       └── FUNCTION_BLOCK
└── ANY_POINTER
    ├── POINTER TO ...
    └── REF_TO ...
```

---

### Appendix C: PLCopen XML Interchange (ST-Complete)

Runtime exposes an ST-complete PLCopen XML profile through `trust-runtime plcopen`:

- `trust-runtime plcopen profile` prints the supported profile contract.
- `trust-runtime plcopen export` exports ST project content to PLCopen XML.
- `trust-runtime plcopen import` imports supported PLCopen ST project content into `sources/`:
  - ST POUs (`PROGRAM`, `FUNCTION`, `FUNCTION_BLOCK`)
  - supported `types/dataTypes` subset (`elementary`, `derived`, `array`, `struct`, `enum`, `subrange`) materialized as generated `TYPE` declarations
  - project model declarations in `instances/configurations/resources/tasks/program instances`
- `trust-runtime plcopen import` emits a migration report at
  `interop/plcopen-migration-report.json` with:
  - discovered/imported/skipped POU counts
  - imported type/project-model counts (`imported_data_types`, `discovered_configurations`, `imported_configurations`, `imported_resources`, `imported_tasks`, `imported_program_instances`)
  - source coverage (% imported/discovered)
  - semantic-loss score (weighted from skipped POUs + unsupported nodes/warnings)
  - compatibility coverage summary (`supported_items`, `partial_items`, `unsupported_items`, `support_percent`, `verdict`)
  - structured unsupported diagnostics (`code`, `severity`, `node`, `message`, optional `pou`, `action`)
  - applied vendor-library shim summary (`vendor`, `source_symbol`, `replacement_symbol`, `occurrences`, `notes`)
  - per-POU entry status (`imported` or `skipped`) and skip reasons

Current ST-complete contract:

- Namespace: `http://www.plcopen.org/xml/tc6_0200`
- Profile: `trust-st-complete-v1`
- Supported POU body: `ST` text bodies for `PROGRAM`, `FUNCTION`, `FUNCTION_BLOCK`
- Supported `dataTypes` baseType subset: `elementary`, `derived`, `array`, `struct`, `enum`, `subrange`
- Supported project model: `instances/configurations/resources/tasks/program instances`
- Source mapping: embedded `addData` payload + sidecar `*.source-map.json`
- Unsupported nodes: reported as diagnostics and preserved via vendor extension hooks where applicable
- Vendor-variant import aliases:
  - `PROGRAM`/`PRG` -> `program`
  - `FUNCTION`/`FC`/`FUN` -> `function`
  - `FUNCTION_BLOCK`/`FB` -> `functionBlock`
- Vendor ecosystem detection heuristics for migration reports:
  - `codesys`, `beckhoff-twincat`, `siemens-tia`, `rockwell-studio5000`,
    `schneider-ecostruxure`, `mitsubishi-gxworks3`, fallback `generic-plcopen`
- Vendor-library baseline shim catalog includes selected alias normalization
  (e.g., Siemens `SFB3/4/5` -> `TP/TON/TOF`) with per-import diagnostics.

Deliverable 5 parity fixture gate:

- CODESYS ST fixture pack (`small`/`medium`/`large`) with deterministic expected
  migration artifacts under:
  - `crates/trust-runtime/tests/fixtures/plcopen/codesys_st_complete/`
- Schema-drift parity regression test:
  - `crates/trust-runtime/tests/plcopen_st_complete_parity.rs`

Round-trip limits and known gaps are documented in
`docs/guides/PLCOPEN_INTEROP_COMPATIBILITY.md`.

### Appendix D: References

1. IEC 61131-3:2013 - Programmable controllers - Part 3: Programming languages
2. PLCopen - Technical Committee 6 (XML)
3. CODESYS Online Help - https://help.codesys.com
4. Beckhoff InfoSys - https://infosys.beckhoff.com
5. rust-analyzer Architecture - https://github.com/rust-lang/rust-analyzer/blob/master/docs/dev/architecture.md
