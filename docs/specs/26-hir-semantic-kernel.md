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
- A source key has one stable bidirectional `FileId` mapping while registered.
  Automatically assigned identifiers increase monotonically and are not
  recycled merely because one source is removed. An explicit identifier
  advances the automatic allocation floor. Clearing the entire registry
  removes both directions and resets automatic allocation for the new empty
  project.
- Updating an existing source synchronizes the project input and semantic
  database before a public query returns.
- An explicit `FileId` collision for a different source key is rejected.
  Re-registering the same key returns its existing identifier.
- Canonical and noncanonical fallback paths are normalized deterministically.
  A fallback path that cannot be proven identical to a canonical path must not
  alias it silently. Fallback normalization removes `.` components but retains
  unresolved `..` components rather than claiming filesystem equivalence.

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

Diagnostic string codes are stable and unique within the HIR registry:
syntax/name/type/semantic failures use their registered `E` codes, warnings use
their registered `W` codes, and hints use their registered `I` codes.
Constructing from a code uses that code's default severity; the explicit error
and warning constructors force the requested severity. Related entries append
in call order.

The diagnostic builder preserves first-insertion order and removes only exact
duplicate diagnostics. A changed range, message, severity, or related entry is
a distinct diagnostic. `has_errors` observes explicit severity, and formatting
uses `<severity>[<code>]: <message> (at <start>..<end>)`.

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

Qualified names are non-empty, case-preserving part lists rendered with `.`;
parsing a dotted name ignores empty separators. Catalog lookup compares the
complete rendered name ASCII case-insensitively and never falls back to a
suffix.

Catalog construction is deterministic. Declarations sort by source file,
source start, then qualified name. References sort by owner symbol, written
name, then reference kind. Distinct declaration records, including duplicate
source declarations retained for diagnostics, are not silently deduplicated.

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

## 8. Symbol table, scope, and type registry contract

The HIR symbol table is the authoritative declaration index used by semantic
analysis. Its observable behavior is:

- Construction creates one global scope, registers every builtin type
  case-insensitively, and publishes the builtin function-block symbols and
  their parameters. User symbols receive monotonically increasing table-local
  identities.
- Child scopes retain their parent and optional owner. Popping the global
  scope is a no-op. Owner-to-scope association is idempotent, and requesting a
  scope for a missing owner fails without manufacturing a global child scope.
- Unqualified resolution searches the local scope, then unambiguous `USING`
  matches in that scope, then each parent. A local declaration shadows a
  `USING` match. Two distinct imported matches are ambiguous and do not resolve;
  duplicate paths to the same symbol are one match.
- Names are compared ASCII case-insensitively. Qualified resolution traverses
  namespace owners only and rejects a non-namespace intermediate. Global and
  qualified lookup never falls back to an arbitrary table-wide match.
- A scope definition reports the prior declaration when replacing the same
  normalized local name. Top-level lookup remains first-writer even though all
  duplicate symbol identities are retained for diagnostics.
- Source-backed declaration lookup uses the normalized name plus the exact
  source range. Imported symbols are not indexed as local source declarations.
  Import collisions retain declaration order and both source ranges.
- Builtin and user type lookup is case-insensitive and stable. Re-registering a
  concrete type name returns the existing identity without replacing its
  definition; a placeholder alias targeting `UNKNOWN` may be completed in
  place. `type_name` preserves the canonical case supplied by the first user
  declaration.
- Constructed struct, union, enum, array, pointer, reference, and subrange
  types retain their full structural payload. Source initializer records are
  table-local, insertion ordered, copy to a fresh identity when imported, and
  may be attached as the default for a type.
- The primitive type registry assigns user type identities monotonically from
  its user-type floor. Reserving a name publishes an `UNKNOWN` placeholder;
  replacing the placeholder preserves both its identity and its case-insensitive
  name binding. Unknown identities do not acquire a name or value.
- Builtin `TypeId` lookup recognizes the short and long IEC spellings for
  `TOD`/`TIME_OF_DAY`, `LTOD`/`LTIME_OF_DAY`,
  `DT`/`DATE_AND_TIME`, and `LDT`/`LDATE_AND_TIME`; display uses the canonical
  long spelling. Near misses are not normalized into a builtin.
- Type classification has explicit family boundaries. Numeric types include
  integer, real, and subrange types; bit strings include `BOOL`; character and
  string types form `ANY_CHARS`; durations and civil date/time values remain
  distinct subfamilies. Generic/meta types are constraints, not elementary or
  derived runtime types.
- Fixed-width elementary integers, bit strings, reals, durations, characters,
  and integer subranges report their declared bit width. Unsized aggregates,
  strings, civil-time values, generics, and subranges with an unknown base do
  not invent a width. `SIZEOF` for `POINTER TO` and `REF_TO` uses the target
  platform's pointer width.
- Array dimensions retain source order and signed bounds. The sole wildcard
  sentinel is `(0, i64::MAX)`, displayed as `*`; every other dimension is
  displayed as `lower..upper`. Generated array, bounded-string, pointer, and
  reference names are deterministic and preserve their complete payload.
- Primitive assignment compatibility is directional. It permits exact
  identity, documented accuracy-preserving scalar widening, same-character-width string
  families regardless of declared bound, `NULL` to pointer/reference storage,
  generic-family membership, subrange normalization to a known base, and
  structurally compatible arrays. Arrays require equal rank, compatible
  element types, and equal bounds unless either corresponding dimension is the
  wildcard. Missing type or element identities fail closed.
- Unqualified enum value lookup distinguishes not found, one match, and
  ambiguity across enum types. Constant lookup normalizes both optional scope
  and name.
- `EXTENDS` and `IMPLEMENTS` references retain their written names and owner.
  Resolution is relative to the owner's scope or follows an explicit qualified
  namespace path; only function blocks, classes, and interfaces are valid OOP
  targets. Missing, wrong-kind, and missing-owner-scope outcomes remain
  classified.
- Alias resolution follows the complete chain. A self-cycle or multi-type
  cycle is an invariant violation and never loops. Member lookup checks the
  derived owner before bases, follows the recorded inheritance chain
  case-insensitively, and terminates safely on cycles.
- Declaration catalogs omit synthetic empty-range builtins, preserve local
  versus imported source identity, build case-preserving qualified names from
  the parent chain, retain containing/owned scopes, and include classified
  inheritance references. A cyclic parent chain is omitted rather than
  producing a false qualified declaration.
