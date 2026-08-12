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

The source-to-runtime `CompileSession::build_runtime` profile produces one
`Runtime` and therefore accepts at most one explicit `RESOURCE`. With no
explicit resource, configuration-level tasks and program bindings describe the
one implicit resource and the runtime identity remains the synthetic
`RESOURCE`. With one explicit resource, its declared instance name becomes the
runtime identity. Two explicit resources, or configuration-level
task/program declarations mixed with an explicit resource, are rejected before
any declarations are flattened or registered.

The identifier after `ON` is the IEC implementer-specific resource type. The
current source runtime accepts that identifier as declaration metadata but
does not use it to select the host backend; host selection belongs to the
validated runtime/bundle configuration. It must never replace the resource
instance name used as runtime identity.

The scheduler can run several independently constructed `Runtime` resources in
separate `ResourceRunner` threads. That orchestration capability does not make
one `build_runtime` call a multi-resource configuration compiler.

### 2.3 TASK

`TASK` defines scheduler inputs such as priority, interval, and single-shot
behavior.

### 2.4 PROGRAM Binding

`PROGRAM name WITH TaskName : ProgramType;` binds a program instance to a task
declared in the same resource or configuration scope.

### 2.5 Configuration parser composition

A complete `CONFIGURATION ... END_CONFIGURATION` syntax tree may contain
configuration-level `VAR_GLOBAL`, nested `RESOURCE` declarations, `TASK`
declarations and `PROGRAM ... WITH` bindings, followed by `VAR_ACCESS` and
`VAR_CONFIG` blocks. The parser preserves qualified access/configuration paths,
including array indexes and bit selectors, their declared types, and
`READ_ONLY`/`READ_WRITE` access modes. The variable-block and access-path rules
remain owned by `03-variables.md`; this section specifies their composition
inside one configuration declaration.

### 2.6 Project-source assembly profile

Configuration binding is performed against the complete project declaration
graph, not against declarations that happen to precede the configuration
source. A configuration source may therefore precede the source containing its
qualified `PROGRAM` type. Only one configuration is assembled for one runtime
project even when the declaration search spans multiple files. Whole-project
duplicate, unknown-type, namespace, source-order, and diagnostic-label rules
are owned by `10-runtime-semantics.md` section 7.10.

IEC 61131-3 Ed.3 section 6.8 and Table 62 own the configuration/resource
hierarchy. The mapping of several repository source units into one declaration
graph is a truST build profile and does not create an IEC decision or
deviation.

### 2.7 Configuration and resource variable scope

IEC 61131-3 Ed.3 section 6.8.1 and Table 62 permit `VAR_GLOBAL` in a
configuration or resource and `VAR_EXTERNAL` in a resource. A configuration
global is visible to its resource. A resource global is local to that resource
in the IEC hierarchy.

Because the source runtime profile accepts only one resource, its runtime
storage uses one unqualified effective global namespace for root/vendor GVL,
program-global, configuration-global, and resource-global declarations.
Names retain their declaration spelling but are compared
ASCII-case-insensitively for collisions. A duplicate across any of those host
scopes is rejected under the project decision in `docs/IEC_DECISIONS.md`;
later allocation or initialization must not replace the first declaration.

`VAR_EXTERNAL` in the accepted resource links to the matching effective global
and allocates no second value. Its declared type and `CONSTANT` state must
match. Configuration/resource constant declarations are evaluated through the
case-insensitive dependency graph specified in `03-variables.md` and
`10-runtime-semantics.md`, not accepted declaration order. A configuration
constant is visible to its contained resource. A resource constant remains
local to that resource. Forward dependencies within a visible scope are
accepted; undefined, out-of-scope, mutable, or cyclic dependencies are
rejected before the runtime is returned.

Only the IEC configuration/resource variable sections are accepted:

- `VAR_GLOBAL`, including reviewed `CONSTANT`, address, and retain qualifiers;
- resource `VAR_EXTERNAL`, including matching `CONSTANT`; and
- the separately specified `VAR_ACCESS` and `VAR_CONFIG` sections.

Ordinary `VAR`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`, `VAR_TEMP`, and
`VAR_STAT` sections are rejected in configuration/resource scope rather than
being silently projected as globals.

## 3. Validation Rules

| Rule | Requirement |
|------|-------------|
| Task identity | Task names are unique under ASCII-case-insensitive comparison within their configuration/resource scope |
| Task fields | Only `PRIORITY`, `SINGLE`, and `INTERVAL` are accepted, and each may occur at most once |
| Task priority | `TASK` init must include `PRIORITY := <Unsigned_Int>` in the runtime `u32` range; zero is highest priority |
| `SINGLE` | When present, names a visible `BOOL` global whose rising edge triggers the task; a Boolean literal is not a storage source |
| `INTERVAL` | When present, must be a non-negative `TIME` literal; zero disables periodic triggering |
| Program binding | `PROGRAM ... WITH <Task>` must reference a declared task in the same `RESOURCE`/`CONFIGURATION` |
| Program type | Bound program type must resolve to a declared `PROGRAM` POU |
| Resource scope | Task/program names resolve within the containing resource/configuration hierarchy |

These rules are surfaced as diagnostics by `trust-lsp` and revalidated during
runtime bundle/build steps.

Accepted tasks retain declaration order, original spelling, interval,
single-trigger identity, priority, program bindings, and function-block
bindings. Registration samples the current Boolean `SINGLE` value so an input
that is already high does not create a false initial rising edge. Holding the
input high does not retrigger; a later low-to-high transition does.

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
