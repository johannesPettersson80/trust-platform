# PLCopen Interop Compatibility (Deliverable 5)

This document defines the current PLCopen XML interoperability contract for
`trust-runtime plcopen` after Deliverable 7 (ST-complete coverage + v1
multi-vendor export adapters).

## Scope

- Namespace: `http://www.plcopen.org/xml/tc6_0200`
- Profile: `trust-st-complete-v1`
- Command surface:
  - `trust-runtime plcopen profile [--json]`
  - `trust-runtime plcopen export [--project <dir>] [--output <file>] [--target <generic|ab|siemens|schneider>] [--json]`
  - `trust-runtime plcopen import --input <file> [--project <dir>] [--json]`
- Product decision for this phase:
  - ST-only PLCopen project support.
  - FBD/LD/SFC graphical network bodies are out of scope and are rejected
    before any source file is emitted.

## ST-complete Profile Contract

The public profile identity is the TC6 namespace
`http://www.plcopen.org/xml/tc6_0200` with profile name
`trust-st-complete-v1`. Both the library profile and
`trust-runtime plcopen profile --json` expose that identity, at least one
supported compatibility row, and nonempty round-trip-limit and known-gap
lists. The library profile additionally publishes the ST POU path in its
strict subset and a partial compatibility-shim row. These fields describe the
selected product profile; they do not claim complete TC6 interoperability.

## ST-complete Interchange Contract

For the supported subset, import and export preserve the tested POU signature
triples of name, POU kind, and normalized ST body across
`export -> import -> export`. A data-type-only document is also a successful
import: supported elementary, enumeration, subrange, structure, and
single-dimension array declarations are emitted together in
`src/plcopen_data_types.st`.

For the reviewed two-POU round trip, each export writes its source-map
sidecar, import writes its migration report, source coverage is `100%`, and
semantic loss is `0%`.

The current data-type-only test does not establish the migration report's
semantic-loss percentage or compatibility verdict. Those report values remain
outside this contract until their zero-POU scoring policy is decided and
tested.

## ST-only Import Profile

A supported ST POU body is emitted as ST source under `src/`. Benign
`addData` beside or inside that body does not replace it and is not classified
as an unknown executable body. Supported and unsupported sibling POUs are
handled independently.

## Import Failure Contract

Malformed XML returns an import error. The CLI also exits unsuccessfully for a
missing input path and reports that the input does not exist. Exact error
types, stable exit-code numbers, and proof that every failure leaves the
destination untouched remain explicit gaps.

## Non-ST Body Diagnostics

FBD, LD, SFC, and unknown executable bodies are skipped with error diagnostics
`PLCO215`, `PLCO216`, `PLCO217`, and `PLCO218`, respectively. A document
containing only one POU of each kind reports four discovered and four skipped
POUs, imports none, emits no ST source, and returns the reviewed
`no importable PLCopen ST content` failure.

## Compatibility Matrix

| Capability | Status | Notes |
|---|---|---|
| ST POU import/export (`PROGRAM`, `FUNCTION`, `FUNCTION_BLOCK`) | supported | Includes common aliases (`PRG`, `FC`, `FUN`, `FB`). Import also reconstructs ordered TC6 body worksheets and standard interface var sections, including `globalVars` and `accessVars`. |
| ST `types/dataTypes` import (`elementary`, `derived`, `array`, `struct`, `enum`, `subrange`) | supported | Imported into generated ST `TYPE` declarations under `src/`. |
| ST `TYPE` export to `types/dataTypes` | partial | Supported ST declarations are emitted; unsupported forms are skipped with warnings. |
| `instances/configurations/resources/tasks/program instances` import/export | supported | Deterministic ST mapping with name normalization and structured diagnostics. |
| CODESYS `addData/globalVars` import/export | supported | Import prefers `interfaceasplaintext` for pragma fidelity and falls back to `<variable>` synthesis. Default import now materializes native truST vendor-parity globals (file-scope GVLs and namespaced GVLs for `qualified_only` lists) without mandatory `VAR_EXTERNAL` injection. Adapter callers can still request strict IEC reshaping through `PlcopenImportGlobalVarMode::StrictIecAdapter`, which restores the older wrapper + injected-`VAR_EXTERNAL` flow for external consumers that require it. Export emits deterministic CODESYS `globalVars` metadata. |
| CODESYS `addData/method` on `FUNCTION_BLOCK` POUs | supported | Import reconstructs ST `METHOD` members from vendor metadata and export emits deterministic `Method` objects/object IDs on the owning POU and in `projectstructure`. Other vendor OOP objects such as properties remain out of scope. |
| CODESYS `addData/projectstructure` folder mapping import/export | partial | Import/export mirrors deterministic folder placement for POUs/GVLs; library/device-tree semantics remain metadata-only. |
| Source map metadata (`trust.sourceMap`) | supported | Embedded `addData` payload + sidecar `*.source-map.json`. |
| Vendor extension preservation (`addData`) | partial | Preserved/re-injectable, but not semantically interpreted. |
| Vendor ecosystem migration heuristics | partial | Advisory signal only; not semantic equivalence. |
| Vendor library shim normalization | partial | Selected aliases are mapped to IEC FB names during import; each mapping is reported. |
| Multi-vendor export adapters (`--target ab|siemens|schneider`) | partial | Exports PLCopen XML + target diagnostics/manual-step report; Siemens target also emits direct `.scl` source bundle; native vendor project package generation is out of scope in v1. |
| Graphical bodies (FBD/LD/SFC) | unsupported | ST-complete contract remains ST-only by product decision. Import rejects these bodies with named diagnostics instead of scraping XML text into ST. |
| Vendor AOI/library internal semantics | unsupported | Advanced behavior remains manual migration work beyond symbol-level shims. |

The POU-kind parser normalizes ASCII alphanumeric characters and applies this
exact table:

| Accepted input | XML kind | declaration | terminator |
|---|---|---|---|
| `program`, `prg` | `program` | `PROGRAM` | `END_PROGRAM` |
| `function`, `fc`, `fun` | `function` | `FUNCTION` | `END_FUNCTION` |
| `functionBlock`, `function_block`, `fb` | `functionBlock` | `FUNCTION_BLOCK` | `END_FUNCTION_BLOCK` |

Any other normalized input is unsupported. Import options default to
`PlcopenImportGlobalVarMode::NativeVendorParity`; callers must explicitly
select `StrictIecAdapter` to request wrapper and `VAR_EXTERNAL` reshaping.

## Export Adapter Contract (Deliverable 7)

When `plcopen export` runs with `--target ab|siemens|schneider`, the export report
includes target-specific adapter fields:

- `target`
- `adapter_report_path`
- `siemens_scl_bundle_dir` (Siemens target only)
- `siemens_scl_files[]` (Siemens target only)
- `adapter_diagnostics[]`:
  - `code`
  - `severity`
  - `message`
  - `action`
- `adapter_manual_steps[]`
- `adapter_limitations[]`

The command also writes:

- target-specific default XML path:
  - `interop/plcopen.ab.xml`
  - `interop/plcopen.siemens.xml`
  - `interop/plcopen.schneider.xml`
- Siemens-target direct source bundle:
  - `<output-file>.scl/*.scl`
- sidecar adapter report:
  - `<output-file>.adapter-report.json`

The public export-target identity table is stable:

| target | report/metadata id | file suffix | display label | serialized enum |
|---|---|---|---|---|
| Generic | `generic-plcopen` | `generic` | `Generic PLCopen XML` | `generic` |
| Allen-Bradley | `allen-bradley` | `ab` | `Allen-Bradley / Studio 5000` | `allen-bradley` |
| Siemens | `siemens-tia` | `siemens` | `Siemens TIA Portal` | `siemens` |
| Schneider | `schneider-ecostruxure` | `schneider` | `Schneider EcoStruxure` | `schneider` |

Adapter diagnostics are migration guidance only; they are not proof of semantic
equivalence on target runtimes.

## Export Adapter Artifact Contract

An Allen-Bradley-targeted library export records target `allen-bradley`, writes
the requested XML plus `<xml>.adapter-report.json`, embeds adapter metadata,
reports `PLCO7AB1`, and exposes nonempty manual-step and limitation lists.
A Siemens-targeted library export writes `<xml>.scl/`, lists each generated
SCL artifact, renders the tested `Main` program as an
`ORGANIZATION_BLOCK`, and writes the matching adapter report.

When the CLI omits `--output`, target `ab` writes
`interop/plcopen.ab.xml` and a real adapter report whose path is returned in
JSON. These artifacts are migration aids, not native vendor project packages
or proof of target-runtime equivalence.

## Migration Report and Vendor Extension Contract

`plcopen import` writes `interop/plcopen-migration-report.json` with:

- Coverage metrics:
  - `discovered_pous`
  - `imported_pous`
  - `skipped_pous`
  - `imported_data_types`
  - `discovered_configurations`
  - `imported_configurations`
  - `imported_resources`
  - `imported_tasks`
  - `imported_program_instances`
  - `discovered_global_var_lists`
  - `imported_global_var_lists`
  - `imported_project_structure_nodes`
  - `imported_folder_paths`
  - `source_coverage_percent`
  - `semantic_loss_percent`
  - `compatibility_coverage`:
    - `supported_items`
    - `partial_items`
    - `unsupported_items`
    - `support_percent`
    - `verdict` (`full` | `partial` | `low` | `none`)
- Structured diagnostics (`unsupported_diagnostics`) with:
  - `code`
  - `severity`
  - `node`
  - `message`
  - optional `pou`
  - `action`
- Applied shim summary (`applied_library_shims`) with:
  - `vendor`
  - `source_symbol`
  - `replacement_symbol`
  - `occurrences`
  - `notes`
- Per-POU migration entries (`entries`) with `status` and `reason`.

The migration percentages are deterministic product metrics, not PLCopen
standard scores:

- `source_coverage_percent` is imported POUs divided by discovered POUs,
  expressed as a percentage; zero discovered POUs produces `0.0`.
- `semantic_loss_percent` is the sum of 70% skipped-POU weight, 20%
  unsupported-node weight, and 10% capped warning weight. The unsupported
  ratio is `unsupported / (unsupported + discovered)`, the warning ratio is
  `min(warnings / (2 * discovered), 1)`, and zero discovered POUs produces
  `100.0`.
- compatibility counts imported POUs as supported, unsupported nodes and
  applied shim occurrences as partial, and skipped POUs as unsupported.
  `support_percent` is supported divided by the total. The verdict is `none`
  for an empty total, `full` when no partial or unsupported item remains,
  `partial` when at least one supported item remains, and `low` otherwise.
- every percentage is rounded to two decimal places.

The report writer creates the parent directory, emits pretty JSON followed by
one newline, and returns the canonical report path.

Ecosystem detection searches XML element attributes, `data` names, and the
complete XML text case-insensitively. Its precedence is Beckhoff/TwinCAT,
OpenPLC, Schneider, CODESYS, Siemens, Rockwell, Mitsubishi, then
`generic-plcopen`. This is advisory migration metadata only.

Unknown top-level project children produce `PLCO101`; unknown children under
`types` other than `pous` and `dataTypes` produce `PLCO102`; and unknown
children under `instances` other than configurations, configuration, or
resource produce `PLCO103`. Each is retained as an owned warning and is not
treated as executable semantics.

An embedded source map is read only from `data` named `trust.sourceMap`; a
missing or malformed JSON payload is ignored. Other `data` elements, including
unnamed elements, are copied in source order into
`plcopen.vendor-extensions.imported.xml` beneath one `vendorExtensions`
wrapper. The embedded source map is excluded from that opaque extension file.

Non-ST POU body diagnostics are fail-closed:

- `PLCO215`: unsupported FBD graphical body.
- `PLCO216`: unsupported LD graphical body.
- `PLCO217`: unsupported SFC graphical body.
- `PLCO218`: unknown non-ST body element.

When one of these diagnostics is emitted, the POU is skipped and no ST source is
synthesized from the graphical or unknown XML payload.

For a supported ST POU accompanied by unsupported type metadata and opaque
vendor `addData`, import reports the unsupported type, returns partial
compatibility, writes the ST source, and preserves the opaque vendor payload in
`plcopen.vendor-extensions.imported.xml`. That reviewed mixed fixture reports
both positive source coverage and positive semantic loss. Export includes the
reviewed vendor-extension marker and payload from
`plcopen.vendor-extensions.xml`; exact XML placement and byte preservation are
not yet proved. The generic CLI JSON report exposes the tested export
counts/source-map path and import ecosystem, compatibility verdict, and
diagnostic code.

## CODESYS Metadata Contract

Export emits the reviewed CODESYS `globalVars`, `projectstructure`, POU
`addData`, interface-as-plain-text, object-ID, and `METHOD` metadata for the
tested supported declarations. Importing that exported method metadata
reconstructs the owning `FUNCTION_BLOCK`, method signature, inputs, and tested
method body. The contract does not extend to CODESYS properties, device trees,
or executable equivalence inside CODESYS.

## Vendor Library Shim Contract

For recognized ecosystems, import replaces only the reviewed symbol positions
with the catalogued alias and records the ecosystem, source symbol,
replacement symbol, occurrence count, and `PLCO301` migration diagnostic.
The Siemens fixture maps `SFB3 -> TP` and `SFB4 -> TON` while preserving an
unrelated variable named `SFB4`. The OpenPLC CLI fixtures detect `openplc`,
map `R_EDGE -> R_TRIG`, and remain exportable with the source-map sidecar.
These are symbol-level migration transforms, not semantic-equivalence claims.

The exact reviewed alias catalogue is:

| Ecosystem | Source | Replacement |
|---|---|---|
| Siemens TIA | `SFB3` | `TP` |
| Siemens TIA | `SFB4` | `TON` |
| Siemens TIA | `SFB5` | `TOF` |
| Rockwell Studio 5000 | `TONR` | `TON` |
| Schneider, CODESYS, OpenPLC | `R_EDGE` | `R_TRIG` |
| Schneider, CODESYS, OpenPLC | `F_EDGE` | `F_TRIG` |
| Mitsubishi GX Works 3 | `DIFU` | `R_TRIG` |
| Mitsubishi GX Works 3 | `DIFD` | `F_TRIG` |

Matching is case-insensitive and rewrites only a type position following
`:`, `OF`, `EXTENDS`, or `REF_TO`, or a direct call position immediately
followed by `(` after trivia. Member access following `.` and ordinary bare
identifier use are unchanged. Repeated applications of the same
ecosystem/source/replacement/notes tuple are aggregated into one record with
the exact occurrence count. Unknown ecosystems and empty token streams are
returned unchanged without a synthetic application record.

## Semantic XML-to-ST Import Contract

Semantic import treats PLCopen element and attribute names
case-insensitively. POU kinds are normalized after removing ASCII punctuation:
`program`, `prg`, and equivalent case or punctuation spellings map to
`PROGRAM`; `function`, `fun`, and `fc` map to `FUNCTION`; and
`functionBlock` and `fb` map to `FUNCTION_BLOCK`. No other POU kind is inferred.

The supported XML type projection is exact and fail-closed:

- `bool`, `byte`, `word`, `dword`, `lword`, `sint`, `int`, `dint`, `lint`,
  `usint`, `uint`, `udint`, `ulint`, `real`, `lreal`, `time`, `ltime`, `date`,
  `ldate`, `tod`, `ltod`, `dt`, `ldt`, `char`, and `wchar` map to their
  uppercase ST names.
- `string` and `wstring` retain a positive integer `length` or `maxLength`;
  an omitted length is unbounded. Zero, negative, nonnumeric, or expression
  lengths are unsupported.
- A derived type requires a nonblank identity. An array requires at least one
  complete lower/upper dimension and a complete base type; dimensions retain
  source order. A structure requires at least one complete, named, typed
  field; fields and simple initializers retain source order. An enumeration
  requires at least one uniquely named element under ASCII-case-insensitive
  comparison and retains explicit values. A subrange requires both bounds and
  a base type, whether the bounds are direct or nested beneath `range`.
- An incomplete aggregate is rejected as one semantic unit. Import never
  publishes a shortened structure, enumeration, array, subrange, interface,
  or data-type source assembled from only the valid children.

POU interface sections render in this stable order: `VAR_INPUT`,
`VAR_OUTPUT`, `VAR_IN_OUT`, `VAR_EXTERNAL`, ordinary `VAR`, `VAR_TEMP`, then
`VAR_GLOBAL`. Variable order and simple initial values are preserved.
`CONSTANT`, `RETAIN`, and `PERSISTENT` modifiers retain declared order;
contradictory `RETAIN` and `NON_RETAIN` reject the POU. Access variables map
`ReadOnly` to `READ_ONLY` and otherwise use `READ_WRITE`. Missing names,
missing or unsupported types, invalid ST identifiers, or duplicate declaration
names under ASCII-case-insensitive comparison reject the owning POU without
publishing a partial source.

A complete ST body whose outer declaration matches its POU is preserved after
newline normalization. A statement-only body is wrapped with the semantic
interface and `PLCO207`; an interface without a body produces a declaration
shell and `PLCO208`. A function uses its structured return type, or defaults
to `INT` with `PLCO211`. If executable source contains no real assignment to
the function result, import inserts `<function> := <function>;` and reports
`PLCO212`; comments and strings do not count as assignments. A `PROGRAM` used
as a derived instance type is promoted to `FUNCTION_BLOCK` with `PLCO210`.
A supported ST body beside any executable FBD, LD, SFC, or unknown body is
ambiguous and rejects the whole POU rather than selecting one representation.

CODESYS method metadata is semantic only on a `FUNCTION_BLOCK`. The reviewed
plain-text method header, interface, body, visibility, return type, and source
order are preserved before `END_FUNCTION_BLOCK`. Missing header metadata
defaults to `METHOD PUBLIC` with `PLCO214`; method metadata on another POU
kind is skipped with `PLCO213`; and a case-insensitively matching method
already present in the ST body is not duplicated.

## Global-Variable Import and Projection Contract

Each CODESYS `globalVars` list is one publication unit. A valid
`interfaceasplaintext` payload takes precedence over structured `variable`
children. Plain text retains leading attributes and the `VAR_GLOBAL` header
suffix, and comma-separated declarations expand in source order with the
same type and initializer. Structured declarations retain XML order, types,
initial values, and attribute order. A child `name` is accepted when the
attribute is absent; an unnamed list receives a stable fallback identity.

A list with no declarations reports `PLCO601` and is not imported. An
unparseable preferred plain-text payload reports `PLCO602` and does not fall
back to structured children. One invalid identifier, duplicate variable name
under ASCII-case-insensitive comparison, missing type, or malformed
declaration rejects the entire list without writing a shortened source.
List identities are normalized deterministically and reported as warnings;
colliding list identities receive `_2`, `_3`, and later suffixes under
ASCII-case-insensitive comparison.

In the default `NativeVendorParity` mode, an ordinary list renders one
`VAR_GLOBAL` source. A true CODESYS `qualified_only` attribute renders the
list as a namespace, preserving its leading attribute and declarations; a
false attribute does not create a namespace. In `StrictIecAdapter` mode, an
ordinary list is wrapped in one configuration. A qualified list instead
renders one `<list>_TYPE` structure and one configuration containing a single
instance of that type.

Strict mode injects a `VAR_EXTERNAL` declaration only into a POU that contains
a real qualified `<list>.<member>` reference. Matching is identifier-boundary
and ASCII-case-insensitive, ignores comments and string literals, and does not
match longer identifiers. An existing declaration suppresses injection only
when the symbol occurs in an actual `VAR_EXTERNAL` section. New external
sections appear after existing variable sections and before executable
statements, and each insertion reports `PLCO603` for the owning POU.

The import report counts every discovered list, only complete imported lists,
and exactly the source paths actually written. Skipped lists remain visible in
diagnostics and semantic-loss scoring; they are not hidden by successful
siblings.

## Project-Model Import Contract

The supported project model accepts `instances/configurations/configuration`,
a configuration directly beneath `instances`, or resources directly beneath
`instances`. Direct resources are wrapped in the deterministic
`ImportedConfiguration`. Configuration, resource, task, and program element
names and their reviewed alias attributes are case-insensitive. Holder and
child XML order is preserved, and a program nested beneath a task inherits
that task unless it names another valid task explicitly.

Identifiers are normalized to valid ST identifiers with deterministic
warnings. Duplicates receive `_2`, `_3`, and later suffixes under
ASCII-case-insensitive comparison, independently within configuration,
resource, task, and program scopes. Configuration-level content renders before
resources; within each scope, tasks render before program bindings.

Task intervals already written as IEC `T#`, `TIME#`, or `LTIME#` literals are
trimmed and preserved. Nonnegative ISO-second forms `PT<n>S` and `PT<n>MS`
are converted exactly to milliseconds or seconds; negative, nonfinite,
overflowing, empty, or otherwise unsupported explicit intervals reject the
owning configuration. A task without an interval or event uses
`INTERVAL := T#100ms`; an event task remains event-driven and receives no
interval default. Missing priority defaults to `1`; invalid explicit priority
rejects the owning configuration. A task specifying both interval and event
is ambiguous and is rejected.

Program bindings retain optional `RETAIN` or `NON_RETAIN`, instance identity,
type identity, and an optional valid task binding. An explicit unknown task
reference rejects the owning configuration; it is never silently redirected.
When a scope contains programs but no tasks, import creates `AutoTask` with
`T#100ms` and priority `1`: configuration-level synthesis reports `PLCO507`,
and resource-level synthesis reports `PLCO506`.

An incomplete task or program is diagnosed and omitted while complete siblings
remain renderable. Empty `instances` reports `PLCO501` and emits no source. An
explicit empty named configuration is preserved as an empty configuration,
reports `PLCO508`, and contributes semantic loss. Reported counts equal the
complete configurations, resources, tasks, and program instances actually
rendered.

## Semantic ST-to-XML Export Contract

Semantic export recognizes `PROGRAM`, `FUNCTION`, and `FUNCTION_BLOCK` and
normalizes bodies to LF with one trailing newline. `TEST_PROGRAM` and
`TEST_FUNCTION_BLOCK` export as their standard POU kinds with an explicit
warning. Unsupported top-level nodes are omitted with source-path and one-based
physical-line warnings. POU source-map entries retain project-relative paths
and declaration lines.

`TYPE` extraction retains declaration order, source identity, multiline
structures, and complete type expressions. The elementary, string, derived,
array, structure, enumeration, and subrange projections are the inverse of the
supported import forms above. Derived types require valid dot-qualified ST
identifiers; strings require positive integer lengths; and aggregates require
all dimensions, fields, members, bounds, and base types. Empty, duplicate,
unfinished, or partially malformed types are never shortened into publishable
XML.

`VAR_GLOBAL` extraction retains leading attributes, modifiers, declarations,
initializers, and comma-name order. Multiple blocks in one source receive
stable ordinal names derived from the source name. An unterminated block, one
malformed declaration, or a duplicate variable identity rejects the entire
global list.

Configuration extraction preserves configuration-level and resource-level
tasks and program instances. Keyword case is insignificant, task intervals
are normalized through the project-model rules, and `RETAIN` or `NON_RETAIN`
binding prefixes are retained. Ambiguous task modes, invalid task values,
missing resource terminators, or unknown task references reject the complete
configuration rather than publishing a partial model. Methods of a function
block preserve source order, visibility, return type, variable sections, and
source identity; duplicate method identities reject export.

Any ST parser error, duplicate POU or data-type identity under
ASCII-case-insensitive comparison, partial global block, or invalid
configuration stops semantic publication before the output XML or source-map
sidecar changes. On success, every report count equals the corresponding
published XML-node count. POUs and source-map entries use one deterministic
identity order, XML attribute data is escaped, CDATA source text remains
unchanged, and a generic export/import round trip recovers the supported POU,
method, data-type, configuration, task, and program identities.

## Transactional Filesystem and Artifact Contract

All imported file names and folder segments are sanitized to one nonempty
portable component. They cannot retain `/`, `\\`, `.`, `..`, a Windows device
name, or a trailing dot or space. Generated paths remain beneath `src/`.
Existing filesystem entries and names reserved earlier in the same import are
compared ASCII-case-insensitively; collisions receive deterministic `_2`,
`_3`, and later suffixes without replacing existing files or directories.

Export source discovery is recursive, literal, and deterministic. Every ASCII
case spelling of `.st` and `.pou` is accepted, unrelated extensions are
ignored, and source paths are sorted. A matching directory entry or symbolic
link is rejected instead of read as source. Output and import-destination
symbolic links are likewise rejected without following or modifying their
targets.

CDATA splitting is byte-preserving for source and vendor-hook payloads,
including embedded `]]>` sequences. The embedded and sidecar source maps are
equal and bind every published POU to a positive source line. Repeated export
of the same model is byte-deterministic except for the documented generation
timestamp; the sidecar source map is fully deterministic.

Export publishes one coherent artifact set: generic export commits XML and
source map; adapter export additionally commits its adapter report; Siemens
export additionally commits the complete SCL bundle. Reports contain only the
paths that were committed. A duplicate semantic identity, output symlink, or
failure to publish any member rolls back the whole export set to the exact
pre-call filesystem state.

Malformed XML and a non-`project` root are side-effect free. Import publishes
complete sources, an optional preserved-vendor-extension artifact, and the
migration report as one coherent set; failure to publish the report or vendor
artifact rolls back the whole set. Existing exact, case-alias, batch, and
post-sanitization source-name collisions are preserved through deterministic
suffixing. The sole report-only failure is a well-formed project with no
importable ST content: it writes the migration report describing the rejected
content, but writes no source or vendor-extension artifact.

## CODESYS ST Fixture Pack and Parity Gate

Deliverable 5 includes deterministic CODESYS ST fixture packs for
`small`, `medium`, and `large` project shapes:

- XML fixtures:
  - `crates/trust-runtime/tests/fixtures/plcopen/codesys_st_complete/small.xml`
  - `crates/trust-runtime/tests/fixtures/plcopen/codesys_st_complete/medium.xml`
  - `crates/trust-runtime/tests/fixtures/plcopen/codesys_st_complete/large.xml`
- Expected migration artifacts:
  - `crates/trust-runtime/tests/fixtures/plcopen/codesys_st_complete/*.expected-migration.json`
- CI parity regression gate:
  - `crates/trust-runtime/tests/plcopen_st_complete_parity.rs`

The parity test enforces deterministic import/export signature stability for
supported ST-project structures and fails CI on schema-drift regressions.

The vendor-family PLCopen fixtures in
`crates/trust-runtime/tests/fixtures/plcopen/synthetic-*.xml` are synthetic,
IP-clean regression fixtures. They are not real exports from vendor tools.
Real CODESYS and TwinCAT export fixtures must be generated from scratch demo
projects and recorded with provenance before they are added to this corpus.

## Supported Ecosystem Detection (Advisory)

Detected values currently include:

- `codesys`
- `beckhoff-twincat`
- `siemens-tia`
- `rockwell-studio5000`
- `schneider-ecostruxure`
- `mitsubishi-gxworks3`
- `openplc`
- fallback: `generic-plcopen`

## Round-Trip Limits

Round-trip means `import -> export -> import -> export` through the
ST-complete contract.

Guaranteed for supported ST-project structures:

- ST POU signature-level stability.
- Ordered ST worksheet import for supported multi-`body` POU inputs.
- Supported `dataTypes` signature stability.
- Supported configuration/resource/task/program-instance wiring intent stability.
- Supported CODESYS globalVars declaration stability.
- Supported CODESYS `FUNCTION_BLOCK` method metadata stability.
- Supported CODESYS folder-placement intent stability for deterministic projectstructure trees.
- Deterministic fallback insertion for missing FUNCTION result assignments (`<FuncName> := <FuncName>;`) during import synthesis.
- Stable source-map sidecar contract.

Not guaranteed:

- Original vendor formatting/layout in XML payloads.
- Preservation of graphical network semantics.
- Import of runtime deployment/safety metadata.
- Exact source file names (imports use sanitized unique names under `src/`).

## Known Gaps

- No semantic import/export for SFC/LD/FBD bodies.
- Export-side `dataTypes` remains subset-based for supported ST `TYPE` forms; unsupported ST type syntax is skipped with warnings.
- Vendor library shim coverage is intentionally limited to the baseline alias catalog.
- No semantic translation for vendor-specific AOI/FB internals and pragmas.
- Vendor OOP objects beyond CODESYS `METHOD` members, such as vendor `PROPERTY` metadata, are not yet semantically imported/exported.
- Vendor extension nodes are preserved as opaque metadata, not executed.
- Core truST does not yet enforce CODESYS `{attribute 'qualified_only'}` as a semantic restriction; qualified access works, but bare global access is still accepted by the vendor-parity language/runtime path.
- Export adapters do not generate native vendor package formats (`.L5X`, TIA project archives, EcoStruxure project archives).

## Example Project

A complete import/export walkthrough project is available in:

- `examples/plcopen_xml_st_complete/`
