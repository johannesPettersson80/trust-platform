# Configurations, Resources, and Tasks

IEC 61131-3 Edition 3.0 (2013) - Sections 6.2 and 6.8.2

This specification owns the declarative project-model elements that bind
`PROGRAM` instances onto scheduled runtime tasks.

## 1. Scope

This document covers:

- `CONFIGURATION` declarations
- `RESOURCE` declarations
- `TASK` declarations and scheduling parameters
- `PROGRAM ... WITH <Task>` bindings
- validation rules shared by `trust-hir`, `trust-lsp`, and the runtime bundle

POU declaration syntax remains in `04-pou-declarations.md`. Runtime scheduler
behavior is specified in `11-runtime-engine.md`. STBC encoding is specified in
`12-bytecode.md`.

## 2. Declaration Model

IEC 61131-3 Ed.3 models project scheduling through nested configuration
elements:

```text
CONFIGURATION
  RESOURCE
    TASK
    PROGRAM ... WITH <Task>
```

### 2.1 CONFIGURATION

`CONFIGURATION` is the outer project-level declaration that owns resources and
program instances. A project may declare zero or more configurations depending
on profile/runtime needs.

### 2.2 RESOURCE

`RESOURCE` groups task declarations, process-image bindings, and program
instances that execute against the same runtime resource.

### 2.3 TASK

`TASK` defines scheduler inputs such as priority, interval, and single-shot
behavior.

### 2.4 PROGRAM Binding

`PROGRAM name WITH TaskName : ProgramType;` binds a program instance to a task
declared in the same resource or configuration scope.

## 3. Validation Rules

| Rule | Requirement |
|------|-------------|
| Task priority | `TASK` init must include `PRIORITY := <Unsigned_Int>` |
| `SINGLE` | When present, must be a `BOOL` literal |
| `INTERVAL` | When present, must be a `TIME` literal |
| Program binding | `PROGRAM ... WITH <Task>` must reference a declared task in the same `RESOURCE`/`CONFIGURATION` |
| Program type | Bound program type must resolve to a declared `PROGRAM` POU |
| Resource scope | Task/program names resolve within the containing resource/configuration hierarchy |

These rules are surfaced as diagnostics by `trust-lsp` and revalidated during
runtime bundle/build steps.

## 4. Runtime Contract

Validated configuration/resource/task metadata is lowered into runtime bundle
metadata and then encoded into the `RESOURCE_META` section of STBC. The runtime
engine uses that metadata to construct scheduler state, process-image sizes, and
program-to-task mappings.

See:

- `11-runtime-engine.md` for scheduler/process-image behavior
- `12-bytecode.md` §6.7 for `RESOURCE_META`

## 5. Diagnostics Ownership

Semantic declaration rules live here. The diagnostic code registry and severity
policy for the resulting editor diagnostics live in `14-lsp.md`.
