# HIR Semantic Kernel Contract

This specification defines the observable product contract of the
`trust-hir` semantic database, its stable semantic model, and its declaration
catalog. It is a truST tooling contract. IEC 61131-3 defines the language
meaning consumed by this kernel, but it does not prescribe Salsa queries,
cache identity, file identifiers, cancellation, or the public Rust semantic
model.

## 1. Source identity and revision lifecycle

- A source is identified by one `FileId` and its registered source key.
- Replacing a source with byte-for-byte identical text leaves the source
  revision unchanged.
- Removing an absent source leaves the source revision unchanged.
- Removing an existing source clears its source text and its file-local query
  products. Re-adding it restores analysis from the new live text.
- Updating an existing source synchronizes the project input and semantic
  database before a public query returns.
- An explicit `FileId` collision for a different source key is rejected.
  Re-registering the same key returns its existing identifier.
- Canonical and noncanonical fallback paths are normalized deterministically.
  A fallback path that cannot be proven identical to a canonical path must not
  alias it silently.

These are fail-closed identity rules: stale text, symbols, or diagnostics must
not be returned under a different source identity.

## 2. Query reuse and invalidation

The public queries `source_text`, `file_symbols`, `analyze`, `diagnostics`,
`type_of`, `expr_id_at_offset`, and the project type catalog obey the following
contract:

| Change | Required result |
|---|---|
| No source changed | A repeat query returns the same semantic result and may reuse the same cached allocation. |
| An unrelated source changed | A file-local query whose dependency set is unchanged retains its semantic result and may retain cache identity. |
| The queried source changed | Queries that read that source recompute before returning. |
| A dependency changed or was removed | Every dependent result is invalidated; stale cross-file resolution is forbidden. |
| A dependency is re-added | Resolution is recomputed from the new live dependency graph. |

Cache identity is testable only where the public Rust API returns shared
objects. Identity reuse is an optimization contract for unchanged inputs; it
never authorizes stale semantic content.

## 3. Cross-file semantic translation

Project analysis imports declarations and types from other files without
changing their language meaning:

- root and namespaced globals retain their resolved declared type;
- constants remain available to string lengths, array bounds, and field
  defaults;
- scalar, array, structure, union, enumeration, subrange, pointer, reference,
  string, wide-string, function-block, class, interface, alias-chain, callable
  return, parameter, property, and initializer identities are translated into
  valid target-table identities;
- imported namespace scopes merge with an existing namespace of the same
  qualified name and are created when the target file has no local namespace
  declaration;
- a callable body remains bound to its real POU scope;
- a local value name does not collide with an imported type of the same
  spelling;
- duplicate project type declarations are diagnosed instead of being selected
  by file order.

An imported identity that cannot be translated is unresolved. It must not be
treated as assignment-compatible or replaced with an unrelated local type.

## 4. Stable semantic outcomes and diagnostic context

Resolution APIs distinguish at least:

- resolved;
- missing name;
- missing owner symbol;
- missing owner scope;
- ambiguous;
- wrong symbol kind;
- cyclic alias or dependency.

Mapping a semantic outcome may transform only a resolved payload. It preserves
every non-resolved classification. Missing POU, action, expression, method, or
inheritance ownership does not fall back to an unrelated global symbol or
scope.

Function and method diagnostic contexts retain their resolved symbol, scope,
and declared return type. Diagnostic construction retains its code, severity,
primary range, message, and related entries.

## 5. Declaration catalog

Every completed analysis exposes a declaration catalog whose entries retain:

- declaration kind and semantic role;
- case-preserving qualified name;
- source `FileId` and source range;
- owner symbol and owner scope when present;
- translated type identity;
- whether the declaration is local or imported.

Reference entries retain the written qualified name, their owner, and a
classified semantic outcome. Imported declarations report their original
source file; local declarations never claim to be imported.

## 6. Concurrency and cancellation

Cancellation and concurrent edit/query boundaries may abandon work, but they
must not panic, deadlock, cross-wire file identities, or publish a partially
updated semantic result. A query that completes returns a result consistent
with one synchronized project revision. Event counters and instrumentation may
report query categories, but they do not change semantic output.

## 7. Primitive model fail-closed rules

- Top-level duplicate insertion follows the documented first-writer lookup
  policy while retaining distinct symbol identities for diagnostics.
- Alias cycles remain explicitly classified as cycles.
- Missing array-element or pointer-target type identities are not assignment
  compatible.
- Built-in names are case-insensitive; a user type retains its canonical
  case-preserving display name.

These rules constrain the public semantic model. They are not claims about IEC
runtime execution or generated bytecode.
