# Developer Workflow Contract

This document defines normative truST behavior for developer-facing source
discovery and project-scoped Git commits. It does not define IEC 61131-3
language semantics.

## Project initialization

When the product wizard initializes Git, an existing `.git` directory, a
symbolic link that resolves to a directory, or a valid worktree `gitdir:` file
means the project is already initialized. An ordinary or malformed `.git`
file, a worktree marker whose target is absent or not a directory, a dangling
or non-directory symbolic link, and any other filesystem object are errors;
the wizard must not silently report Git initialization success. Without an
existing marker, initialization requires an available `git` command and a
successful `git init` exit status.

The guided CLI, browser setup, and automatic first-run wizard must leave a
coherent runnable project rather than a collection of independently valid
files. A selected resource name and cycle interval are applied to
`runtime.toml`, the setup-owned `src/config.st`, and the rebuilt `program.stbc`
as one prepared change. The complete source set must compile before these
artifacts are replaced. A cycle interval of zero, an invalid or reserved IEC
identifier, or a source/build failure rejects the setup instead of writing a
known-invalid runtime configuration.

Names derived from a project folder are deterministic valid, non-reserved
Structured Text identifiers under `docs/specs/01-lexical-elements.md`. Empty or
punctuation-only names use `Res`; a leading digit or reserved keyword is
prefixed with `Res`. This generation rule is product scaffolding that consumes
the IEC identifier contract; it is not an IEC decision or deviation.

Wizard bytecode generation recursively discovers `.st` and `.pou` files using
ASCII case-insensitive extensions, deterministic path order, and literal
filesystem names. It includes resolved local project dependencies and
propagates directory/file-read failures. Selecting system I/O removes an
existing project `io.toml` regular file or symbolic link, including a dangling
link, but rejects a directory or unsupported filesystem object at that path.
Automatic setup migrates a legacy `sources/` tree to `src/` before discovery,
removes only the exact obsolete generated `io.st` template, and compiles the
migrated project plus resolved local dependencies into validated bytecode.

Remote browser setup tokens contain 128 bits from the operating-system secure
random source. If that source fails, setup aborts before binding the server; it
must not use zero bytes, timestamps, process IDs, or another fallback token.
The configured minute TTL is converted and added to the current Unix time with
checked arithmetic. Overflow is a setup error rather than a wrapped or
effectively unbounded credential lifetime. Local loopback setup remains
token-free; remote setup accepts the unexpired token from `X-Setup-Token` or
the `token` query parameter.

Guided setup modes have explicit side-effect boundaries. Browser and CLI dry
runs report their effective project, access, and device defaults without
creating project or system files or binding a server. System-I/O setup flags
cannot be combined with guided project options. A non-interactive invocation
without an explicit mode fails with guidance rather than waiting for input.
Browser-only access, bind, port, and token-TTL options are rejected for CLI
mode instead of being accepted and silently ignored, including when an
explicit value equals the browser default.

The browser defaults response and a partial apply request use the same safe
Boolean defaults: use system I/O is true, write system I/O is false, and
overwrite system I/O is false. Consequently an apply request must explicitly
set `write_system_io` to true before it can attempt a system-wide I/O write;
omitting that optional field must never acquire the side effect. Project data
and apply routes require the configured unexpired setup token in remote mode.
These setup-mode, HTTP API, and filesystem-side-effect rules are truST product
contracts, not IEC decisions or deviations.

## Executable tutorial examples

The first eight single-file programs under `examples/tutorials/` and the
standalone `examples/tutorials/09_simulation_coupling/` project are shipped
product artifacts. Every Structured Text source must compile through both the
in-process runtime harness and the current source-to-bytecode path. Tutorial 09
must also pass `trust-runtime check --project` from its checked-in project root,
including its runtime and I/O configuration, so the documented Runtime Panel
flow works without repository-root fallback. Success requires these paths to
return without a compile, type-check, lowering, or validation error; this
bounded fixture set does not certify arbitrary Structured Text programs.

Three tutorials additionally define executable behavior locks:

- `02_blinker.st` starts with `Lamp = FALSE`. After 250 ms and one cycle it is
  `TRUE`, remains `TRUE` after another 1 ms and cycle, and returns to `FALSE`
  after another 250 ms and cycle.
- `03_traffic_light.st` starts red and advances at each reviewed 500 ms plus
  transition cycle through red+yellow, green, yellow, and back to red.
- `05_motor_starter.st` starts stopped, latches `MotorRun` and
  `SealInContact` after `StartPb`, remains latched when `StartPb` is released,
  unlatches on `StopPb`, can start again, and unlatches on `OverloadTrip`.

Every observed cycle in these behavior locks must complete without runtime
errors. The exact delays and I/O sequences above describe the shipped tutorial
fixtures, not general IEC timer, traffic-control, or motor-safety semantics.
They are truST product contracts and are neither IEC decisions nor IEC
deviations.

## Shipped project example corpus

The reviewed shipped project corpus is a product artifact rather than an
arbitrary-language conformance claim:

- every communication example directory contains its required project,
  runtime, I/O, and source files and succeeds through `trust-runtime build`
  followed by `trust-runtime validate`;
- the plant demo configuration retains its reviewed resource, task, program,
  and directly represented I/O bindings;
- the reviewed EtherCAT, Mitsubishi GX Works3, PLCopen ST-complete, and Siemens
  SCL project examples parse, type-check, and lower to bytecode;
- every visual example companion/runtime pair compiles through both the
  runtime and source-to-bytecode paths, and at least one such pair exists; and
- every tracked Structured Text example in the repository example corpus
  parses without diagnostics.

Fixture discovery, copying, and filesystem setup are test preconditions. The
product oracle is the bounded tracked corpus and the named build, validation,
parse, type-check, and bytecode outcomes; it does not certify arbitrary vendor
projects or every IEC language partition.

## Source Discovery

`trust-dev test` recursively discovers regular files below the selected project
root whose final extension is `.st` or `.pou`, compared with ASCII
case-insensitivity. Every ASCII case spelling of those extensions is supported.
Directory and file names are treated literally, including spaces, Unicode, and
glob metacharacters.

Unsupported extensions are excluded. When no supported source is found, the
command must report that the supported extension set is `.st` and `.pou`; it
must not report a successful empty test run. An unreadable supported source or
an unsupported discovery shape fails visibly rather than disappearing from the
result.

## Test POU discovery

After loading supported source files, `trust-dev test` discovers only truST
`TEST_PROGRAM` and `TEST_FUNCTION_BLOCK` declarations. Discovery reports the
test kind, namespace-qualified name, source path, one-based declaration line,
and declaration source line. Comments and other parser trivia following a test
name do not change its identity. Results are ordered by source path, declaration
offset, and name so repeated discovery of the same source set is deterministic.

`TEST_PROGRAM` and `TEST_FUNCTION_BLOCK` are truST test-workflow extensions,
not additional IEC 61131-3 POU kinds and not IEC deviations.

## Test case execution

`trust-dev test` prepares one compiled runtime for the selected source set and
cold-restarts it before every selected case. The cold restart restores declared
initial values, so state written by one test case is not inherited by the next.
Shared discovery, compilation, and runtime preparation occur outside the
per-case execution deadline; that deadline begins immediately before the
selected case is invoked.

Execution mode also compiles the complete project when discovery finds no test
POU. An invalid project therefore fails visibly instead of being reported as a
successful empty test run. Listing remains discovery-only, and an explicit
filter that matches none of the discovered tests does not execute or compile a
test case.
The selected declaration is then invoked at its own boundary:
`TEST_PROGRAM` as a program and `TEST_FUNCTION_BLOCK` as a function-block
instance.

When a production `CONFIGURATION` exists, selected test programs are registered
as additional program instances for the test session. Ordinary runtime
construction without that explicit registration continues to reject an
unconfigured test program; the test runner does not silently alter the
production configuration.

An `ASSERT_*` failure is a failed test. A non-assertion runtime failure is an
error. A configured per-case deadline that is reached produces the distinct
execution-timeout result. Successful execution therefore requires entering the
selected test body; merely compiling or locating it is not success.

These runner and assertion behaviors are truST product extensions. They do not
change IEC 61131-3 program execution outside the developer test workflow.

## Test reporting and CI output

The test command preserves three result states: `passed`, `failed`, and
`error`. JSON output is version 1 and reports aggregate totals, total duration,
and per-test status, source context, and duration. TAP output uses a version-13
plan and reports each test by `TEST_PROGRAM::<name>` or
`TEST_FUNCTION_BLOCK::<name>`, with source diagnostics for non-passing cases.
JUnit output uses one compatibility suite named `trust-runtime`, reports
separate failure and error counts, and XML-escapes diagnostic text.

Human output identifies every non-passing test with its kind, name, source
path, line, duration, reason, and available source line, followed by an ordered
failure summary and aggregate counts. A filter that removes every discovered
test reports both the filter and the unfiltered discovery count. List mode
reports kind, name, project-relative path, line, and final listed count.

`--ci` changes the default human selection to JUnit. An explicitly requested
JSON, TAP, or JUnit format is preserved. Timeout prose uses singular
`1 second` and plural `<n> seconds`.

### Native runtime CI fixture compatibility

The runtime compatibility entrypoint preserves the following reviewed native
project-fixture contract:

- successful `build --ci` emits version 1, command `build`, status `ok`, the
  selected project path, and a source count of at least the two fixture source
  files;
- successful `validate --ci` emits version 1, command `validate`, status `ok`,
  the selected project path, and resource `Res`;
- successful `test --ci --output json` reports the selected project, zero
  failures and errors, numeric aggregate and per-case durations, at least one
  discovered test, and the `CI_Passes` case;
- the green fixture's explicit JUnit run contains a test suite with zero
  failures; and
- the broken fixture's explicit JUnit run returns process class 12, contains a
  failure entry, and reports `ST test(s) failed` on stderr.

These are bounded compatibility fixtures. They do not certify arbitrary
projects, every JSON field, XML schema conformance, or stable diagnostic prose
beyond the named failure marker.

### CI reliability and release-gate artifact contract

The reviewed native CI support surfaces preserve the following executable
contract:

- the flake probe emits schema version 1, the selected project, exactly the
  requested run count, pass and failure totals whose sum equals that count, a
  numeric flake rate, and one ordered `pass` or `fail` sample per run;
- the nightly reliability workflow remains manually dispatchable, runs the
  load, soak, and ST flake probes, samples 20 ST runs with zero accepted
  failures, enforces the reliability summary gates, and uploads run-scoped
  machine-readable artifacts;
- release-gate aggregation fails with verdict `FAIL` and the exact missing
  required artifact identities when any required gate artifact is absent, and
  returns verdict `PASS` with an empty missing set when all required artifacts
  and successful gate results are present;
- reliability-summary enforcement returns nonzero when a configured load,
  soak, or flake budget is breached;
- the public native CI template builds both CLIs, then performs runtime build,
  validate, JUnit test, and report upload in the reviewed order; and
- the VS Code extension job remains a required release-gate input, so a failed
  or absent extension artifact cannot be aggregated as release-ready.

These checks prove the named scripts, workflow wiring, machine-readable fields,
and fail-closed aggregation boundaries. They do not prove that an unexecuted
GitHub workflow ran successfully, that uploaded artifacts are publicly
available, or that the final release candidate is green.

### Native test and docs-capture lifecycle portability

Native Unix control-endpoint fixtures must derive a collision-resistant socket
path that stays within the portable 100-byte pathname budget even when the
configured temporary directory is longer. A long `TMPDIR` must not turn an
otherwise valid runtime test into a socket-bind setup failure.

The automated docs-capture launcher owns every browser, runtime, and Docker
process that it starts. On success, command failure, interruption, or timeout,
it must terminate the complete owned process session and remove the named
code-server container before returning. It must not terminate a reused or
externally managed server. The executable lifecycle tests use fake commands and
processes to prove cleanup ownership; rendered Playwright journeys separately
prove the captured product surfaces.

The Docs Captures workflow must run for pull-request and main-push changes to
the runtime Web UI source tree so browser-visible runtime changes cannot bypass
their registered Playwright journeys. Its pull-request and push path filters
must remain identical.

### Cargo integration-test binary-path portability

Workspace integration tests that launch a workspace binary must remain
compilable when Cargo checks or lints all targets without building that binary.
They must resolve Cargo's `CARGO_BIN_EXE_*` value from the execution environment
instead of using either compile-time `env!` form. This preserves ordinary
`cargo test` executable discovery while allowing the reviewed host and
cross-target warning and clippy gates to compile every crate's integration-test
targets. The CI portability scanner must inspect every Rust source below each
workspace crate's `tests` directory and reject compile-time lookup for every
binary name, not only `trust-runtime`.

### CI performance reference-environment contract

The three native CI timing probes are reference-environment contracts, not
portable performance guarantees. Their reviewed reference environment is:

- Ubuntu 26.04 on Linux x86-64;
- an AMD EPYC-Genoa Processor with 8 logical CPUs;
- 16,361,160,704 bytes of memory;
- `rustc 1.97.0 (2d8144b78 2026-07-07)`,
  `cargo 1.97.0 (c980f4866 2026-06-30)`, and
  `cargo-nextest 0.9.140 (a9fef2964 2026-07-05)`;
- the already-built `trust-runtime` test binary and copied repository
  fixtures, with a warmed shared Cargo target; this is not an installation,
  clean-toolchain, or clean-compilation measurement;
- the test runner's default thread selection unless the retained command says
  otherwise; and
- one observed wall-clock sample per assertion, no in-test warmup, and direct
  comparison with the asserted bound rather than a median or percentile.

On that profile, the first passing copied-fixture path must complete within
600 seconds; the filtered single-test path must complete within 2 seconds and
report exactly one passing case with no errors; and the 10-test versus 40-test
fixture pair must remain inside the asserted total-time, per-test, and scaling
envelopes.

Every accepted execution artifact binds the source revision, exact test,
hostname and profile above, command, terminal result, and observed duration.
A result from different hardware, memory, operating system, toolchain,
prebuilt state, cache state, concurrency, sample count, or aggregation policy
is informative only and cannot satisfy this contract. These probes do not
claim portable latency, clean setup, compilation speed, or arbitrary-project
scaling.

### Registry initialization CLI contract

`trust-runtime registry init` preserves the explicit root, visibility, and
optional token selected by the operator. `--visibility private` remains
private, and an explicitly supplied token reaches the private initialization
action without being reclassified as a public registry or another registry
subcommand. This parser contract does not establish storage authorization,
network authentication, or secret-handling proof beyond exact argument
binding.

### Package registry storage and integrity contract

The local package registry is a fail-closed content store. Its configuration,
index, package metadata, and package payload form one integrity boundary; a
successful CLI message or the mere presence of a package directory is not
publication proof.

Registry roots use schema version `1`. Initialization creates
`registry.toml`, `index.json`, and `packages/`. A private registry requires a
non-empty normalized token before a usable configuration or index is written.
Reinitializing an existing registry may update its access configuration, but
it must first validate the existing index and must preserve every existing
package and index entry. An unreadable, malformed, or unsupported existing
configuration or index is an error and is never replaced with an empty
default.

Package names and versions are normalized by trimming surrounding whitespace.
The remaining value must be one non-empty path segment containing only ASCII
letters, digits, `-`, `_`, and `.`; the complete segments `.` and `..` are
invalid. The same validation applies to publish, download, and verify
requests. Separators, traversal components, absolute paths, and symbolic-link
redirection must not escape the configured registry root. Registry payloads
contain regular files and directories only. A symbolic link or another
special filesystem entry in a source bundle or stored package is rejected
rather than followed or silently omitted.

A package identity is the normalized `(name, version)` pair. Published
identities are immutable: a second publish of the same pair fails without
changing its payload, metadata, or index. Publication validates access, the
bundle, its identity, and the existing index before creating an addressable
package. It stages the payload and metadata outside the final identity path,
then commits a complete package and its index entry. If any publication step
fails, the prior index remains byte-for-byte intact and no final package
identity is left behind. Temporary staging material is not listable and is
removed or safely recoverable before a later publication. JSON/config writes
use same-directory replacement so readers observe the old complete document
or the new complete document, never a truncated intermediate document.

`metadata.json` binds all of the following:

- the package name and version, which must equal the requested identity and
  containing path;
- the runtime resource name and bundle schema version;
- a publication timestamp;
- every regular payload file exactly once, using a `/`-separated relative path
  in ascending lexical order;
- each file's byte count and lowercase SHA-256 digest;
- `total_bytes`, equal to the checked sum of all file byte counts; and
- `package_sha256`, computed deterministically from the ordered path, digest,
  and byte-count tuples.

Verification rejects missing, extra, reordered, duplicated, renamed, resized,
or modified payload facts, inconsistent totals, an identity mismatch, an
invalid digest encoding, malformed JSON, and unsupported registry schema
versions. It does not repair metadata or the index. Download with verification
validates the stored package before mutating the destination and validates the
completed copy before reporting success. A preflight or stored-package
verification failure leaves an existing empty destination empty; a non-empty
destination is rejected without changing its contents. Download without the
verification option is an explicit unchecked copy and supplies no integrity
claim.

`index.json` contains schema version `1`, a generation timestamp, and exactly
one summary for every committed package identity. Entries are ordered first by
name and then by version. Each summary agrees with its package metadata for
resource name, publication timestamp, total bytes, and package digest.
Malformed JSON, a wrong schema version, duplicate or unsorted identities, or a
summary that disagrees with stored metadata is an error. Listing does not
silently normalize, delete, or replace such an index.

Every private-registry operation, including list, publish, download, and
verify, authenticates before reading or mutating package state. Missing,
blank, or unequal tokens are unauthorized. Registry profiles and operation
reports never return the configured token. These local filesystem contracts
do not claim network transport authentication or multi-process transaction
isolation.

## PLCopen XML semantic import contract

PLCopen XML import is a typed translation into the supported truST Structured
Text subset, not text scraping. XML element and attribute names used by the
supported profile are matched case-insensitively. POU identity and declaration
identity remain case-insensitive even when the source XML uses different
spelling. The import report counts every discovered POU exactly once as either
imported or skipped.

The supported POU kinds are `program`/`prg`, `function`/`fun`/`fc`, and
`functionBlock`/`fb`, ignoring ASCII case and punctuation in the kind spelling.
Unsupported or missing POU kinds, missing names, and graphical or otherwise
non-ST bodies are skipped with the stable named migration diagnostic; they are
never scraped into a source file. A supported POU containing both an ST body
and a non-ST executable body is also skipped rather than choosing one silently.
Benign body metadata such as documentation, comments, and `addData` may
accompany exactly one effective ST body.

An ST body that already contains the matching complete top-level declaration
is preserved as the POU source. A body containing only executable statements is
wrapped using the XML POU name, kind, and interface. A missing or empty body may
produce a declaration shell only when the interface or supported vendor method
metadata supplies meaningful declarations. Otherwise the POU is skipped.

The interface translation recognizes `inputVars`, `outputVars`, `inOutVars`,
`externalVars`, `localVars`, `tempVars`, `globalVars`, and `accessVars`, in
that stable order. Ordinary variables require one nonblank name and one
supported type, and may carry a simple initial value. Access variables require
an alias, instance path, type, and a direction mapped to `READ_ONLY` or
`READ_WRITE`. Section Boolean attributes map to `CONSTANT`, `RETAIN`,
`NON_RETAIN`, `PERSISTENT`, and `NON_PERSISTENT`. Mutually contradictory
modifiers, duplicate case-insensitive declaration names within a POU,
unsupported type shapes, invalid ST identifiers, or malformed required
metadata are loss-bearing errors: the POU is not published as apparently valid
ST, and a stable diagnostic names the rejected XML node and remediation.
Silently dropping a malformed declaration while importing the rest of its POU
is forbidden.

Supported type expressions include:

- all IEC elementary scalar tags in the declared profile;
- `STRING` and `WSTRING`, with an optional positive declared length;
- nonblank `derived` type names;
- arrays with one or more complete inclusive dimensions and a supported base
  type;
- structures with at least one valid, uniquely named field;
- enumerations with at least one valid, uniquely named element and optional
  explicit values; and
- subranges with a supported base type and complete lower and upper bounds.

Multi-dimensional array bounds preserve source order. Structure fields preserve
source order and simple initial values. An empty structure or enumeration,
incomplete array dimension, nonpositive string length, missing derived name,
or incomplete subrange is unsupported metadata rather than a fabricated type.
Generated type and POU source must parse as the intended declaration kind
before any source artifact is published.

Functions use `interface/returnType` when present, otherwise an importable
vendor plaintext header. When neither supplies a return type, the compatibility
profile deliberately defaults to `INT` and emits `PLCO211`. A synthesized
function with no real result assignment receives the compatibility
self-assignment and `PLCO212`; occurrences in comments or another identifier
do not suppress that repair.

Supported CODESYS method objects are imported only for function blocks.
Plaintext method headers preserve their declared visibility and return type.
When a method header is absent, import defaults it to `METHOD PUBLIC`, uses the
structured return type when present, and emits `PLCO214`. Interface sections
and the ST method body are preserved. A method already present
case-insensitively in the ST function-block source is not duplicated; missing
methods are inserted before `END_FUNCTION_BLOCK` in XML order. Method metadata
on another POU kind is skipped with `PLCO213`.

A PLCopen POU declared as `program` but referenced as a derived type is promoted
to `FUNCTION_BLOCK` with `PLCO210`, because a program cannot serve as an
instance type in ST. Import reports the resolved kind after promotion. All
fallbacks, promotions, skips, and unsupported metadata remain visible in the
migration report and semantic-loss accounting.

### PLCopen configuration, resource, task, and program-instance translation

The importer recognizes configurations below
`instances/configurations/configuration` and directly below `instances`.
Direct resources below `instances` are wrapped in one deterministic
`ImportedConfiguration`. Element and supported attribute aliases are
case-insensitive. Each discovered configuration is counted exactly once and
produces at most one complete ST configuration source.

Configuration, resource, task, program-instance, target, type, and task
references must become valid ST identifiers. The compatibility translation
replaces invalid identifier characters with `_`, prefixes a leading digit with
`_`, and supplies the documented `ImportedConfiguration`, `ImportedResource`,
`CPU`, `Task`, `Program`, or `MainProgram` fallback only when the corresponding
optional source identity is absent. Every normalization is reported. Resulting
identities are unique case-insensitively within their ST scope; collisions
receive deterministic `_2`, `_3`, and later suffixes in XML order.

A task requires a nonblank identity and exactly one scheduling mode:

- cyclic tasks use `interval`, `cycle`, `cycleTime`, `period`, or a nested
  interval value;
- event tasks use `single`, `event`, or `trigger`.

An omitted mode defaults to `INTERVAL := T#100ms`. An omitted priority defaults
to `1`. Explicit intervals accept valid IEC `T#`, `TIME#`, or `LTIME#`
durations, plus nonnegative ISO `PT...S` or `PT...MS` values translated without
overflow to an equivalent `T#` literal. Explicit priority must be an integer in
the runtime-supported task priority range. Empty, negative, nonfinite,
overflowing, or otherwise malformed schedule values are loss-bearing; they do
not enter generated ST. Supplying both cyclic and event modes is ambiguous and
rejects the task rather than silently preferring one.

A program binding requires a nonblank instance name and POU type. A nested
program below a task inherits that task when it has no explicit task
reference. An explicit reference must resolve case-insensitively to exactly one
task in the same configuration or resource; it is never silently redirected
to the first task. If a scope has program bindings but no task declarations,
the importer creates one `AutoTask` with `T#100ms` and priority `1`, binds every
otherwise unbound program to it, and emits `PLCO506` for a resource or
`PLCO507` for configuration scope. Explicit unresolved references remain
errors and do not use this fallback.

Generated order is deterministic: configuration tasks, configuration program
bindings, then resources in XML order; within a resource, tasks precede
program bindings. Resource programs nested below a task retain their inherited
binding. The output closes every resource and configuration and must parse as
the intended ST project-model declarations before publication. An empty
instances container emits `PLCO501`; an empty normalized configuration emits
`PLCO508`. Neither condition is represented as scheduling proof.

### PLCopen global-variable-list translation

Each `globalVars` element is one discovered list. The list identity comes from
its nonblank name or child name, otherwise the stable `GlobalVarsN` fallback.
Its imported filename and generated ST identity use the same path-safe and
ST-safe normalization rules, and case-insensitive list collisions receive
deterministic suffixes rather than overwriting or publishing duplicate
namespaces/configurations.

Interface-as-plaintext metadata takes precedence when present; otherwise the
importer synthesizes declarations from structured `variable` entries. A list
must contain at least one complete, parseable declaration. Names are
case-insensitively unique within the list; each declaration has a valid ST
identifier, supported type, and optional simple initial value. Malformed,
duplicate, or partially parsed declarations reject that list with a stable
diagnostic instead of being silently omitted. Section modifiers and leading
vendor attributes are preserved only when they produce valid ST.

In native vendor-parity mode, an ordinary list produces `VAR_GLOBAL`; a
`qualified_only` list produces one `NAMESPACE <list>` containing its
`VAR_GLOBAL` block so `List.Member` remains the access spelling. The marker is
recognized from plaintext attributes or structured CODESYS attribute metadata
case-insensitively.

In strict-IEC adapter mode, an ordinary list produces one deterministic
configuration-owned `VAR_GLOBAL` block. A qualified-only list produces a
`<List>_TYPE` structure plus a `<List>_Globals` configuration holding one
global `<List>` instance. POUs that reference `List.Member` receive the
corresponding `VAR_EXTERNAL List : <List>_TYPE;` declaration exactly once,
unless they already declare it case-insensitively. Injection preserves the
POU header and existing variable-section order and occurs before executable
statements. `PLCO603` records every synthesized adapter.

List order, declaration order, initial values, modifiers, and generated paths
are deterministic. Counts and written-source paths in the import report match
the complete artifact set; a skipped list contributes semantic-loss evidence
and never appears in the imported count.

### PLCopen semantic export contract

PLCopen export first performs a read-only semantic preflight over the complete
selected source set. Every source must be valid UTF-8 and must parse without ST
syntax errors. A parser error, duplicate case-insensitive POU/type/GVL/
configuration identity, malformed supported declaration, or incomplete
project-model binding blocks publication before XML, source-map, adapter, or
SCL output is changed. Unsupported but well-formed top-level kinds may be
skipped only with a source-path and physical-line warning and only when at
least one supported declaration remains.

The supported POU projection is `PROGRAM`, `FUNCTION`, and
`FUNCTION_BLOCK`. `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK` use the corresponding
standard PLCopen POU kind and retain a warning that the test marker is not
representable. POU bodies preserve normalized LF text and one trailing newline
inside CDATA. Names, kinds, source paths, and one-based physical declaration
lines are reproduced in the embedded and sidecar source maps. Source-map
entries follow the deterministic exported POU order.

TYPE blocks export elementary, bounded and unbounded string, derived,
multi-dimensional array, nonempty structure, nonempty enumeration, and
subrange expressions through their matching PLCopen base-type representation.
Field/element/dimension order and simple initial or explicit values are
preserved. Invalid identifiers, empty structures/enumerations, incomplete
bounds, invalid string lengths, duplicate field/element names, and
unrepresentable expressions are not emitted as plausible XML. A valid but
unsupported type may be skipped with an exact warning; malformed or duplicate
authority blocks the export.

Each complete top-level `VAR_GLOBAL ... END_VAR` block becomes one global list.
The list identity derives deterministically from its project-relative source
stem and block ordinal. Leading attribute pragmas, section modifiers,
declaration order, comma-expanded identities, supported types, and simple
initial values are preserved in plaintext and structured metadata. Unterminated
or partially parsed blocks, invalid or duplicate declaration identities, and
unsupported types block export instead of silently shrinking the list.

Configuration extraction preserves complete `CONFIGURATION`, `RESOURCE`,
`TASK`, and `PROGRAM` bindings under the scheduling rules above. The exporter
normalizes supported interval literals but does not invent meaning for invalid
durations, priorities, ambiguous scheduling modes, missing resource ends, or
unresolved task references. Those are preflight failures. Deterministic XML
order is case-insensitive configuration name, then tasks, programs, and
resources; resource tasks and programs use the same ordering.

Function-block methods are exported in their owning POU metadata. Method
identity is case-insensitively unique within the owner. Header visibility,
return type, supported variable sections and modifiers, body, source identity,
and declaration line are preserved. A malformed method or a method on an
unrepresentable owner blocks its metadata rather than producing a partial
method object.

The export report counters equal the XML nodes actually published. XML
attribute text and CDATA are escaped without semantic change. Re-importing the
generic output must recover the same supported declaration identities, POU
kinds, type expressions, global declarations, configuration bindings, method
order, and normalized ST bodies. Timestamp text is the only permitted
nondeterministic semantic-export field.

## PLCopen XML filesystem transaction and identity contract

PLCopen XML import and export are project migration operations, not IEC
61131-3 language semantics. An explicit library import or export call means the
caller has already completed any required overwrite confirmation. The library
still owns safe path resolution, complete artifact planning, and
failure-atomic publication.

Import reads and parses the complete XML document and validates the `project`
root before creating a source directory or changing any project artifact. It
then plans the complete import result before publication: generated sources,
the optional preserved-vendor-extension file, and the migration report. Source
file and folder components are portable filesystem names. Parent components,
absolute paths, separators, empty components, trailing dots or spaces, and
platform device names cannot escape or make the result platform-dependent.
All generated paths remain below the selected project root, and generated
source paths remain below its `src/` directory.

Generated source identities are compared with ASCII case-insensitivity against
both the complete planned batch and existing destination entries. A collision
uses the deterministic `_2`, `_3`, and subsequent suffix sequence; it never
silently replaces an existing source. Different input names that sanitize to
the same portable component remain distinct through that sequence. A
directory, symbolic link, or other special object at a planned source,
vendor-extension, report, or staging path is rejected rather than followed or
replaced.

A successful import publishes its complete planned artifact set as one
user-visible transaction. If staging or publication fails, every pre-existing
artifact is restored byte-for-byte and no partial source, vendor-extension,
report, empty directory, or temporary artifact remains. Malformed XML, an
invalid root element, an unreadable input, and other preflight failures are
side-effect free. A structurally valid document with no importable ST content
is the one diagnostic-only outcome: it publishes one complete migration report
describing the skipped or unsupported content, returns the no-importable-content
result, and creates no source or vendor-extension artifact.

Export recursively discovers regular `.st` and `.pou` files using ASCII
case-insensitive extension matching and deterministic project-relative path
order. Literal spaces, Unicode, and glob metacharacters in names do not change
identity. A matching directory, symbolic link, or other special filesystem
entry is rejected rather than read or followed. Case-insensitively duplicate
POU identities are ambiguous and reject export before output mutation.

The complete export artifact set is planned before publication. Generic export
contains the XML document and source-map sidecar. A vendor target additionally
contains its adapter report, and Siemens export additionally contains the
complete SCL bundle. The XML embedded source map and sidecar represent the same
profile, namespace, ordered POU identities, source paths, and one-based lines.
An adapter report names the exact XML, source-map, and optional SCL artifacts
committed by that export. Vendor-extension and JSON payloads are split safely
across CDATA terminators so the resulting XML remains well formed and
round-trips the original payload bytes.

Export stages every file and directory beside its final destination and then
publishes the complete set. If any output, source-map, adapter-report, or SCL
bundle step fails, all pre-existing artifacts remain byte-for-byte unchanged,
no partial new artifact or staging path remains, and a symbolic-link output is
never followed. On success, readers observe one coherent artifact generation.
For identical project bytes, target, and output identity, export content is
deterministic apart from the documented generation timestamp.

These guarantees are single-operation filesystem guarantees. They do not
claim multi-process transaction isolation, crash-consistent durability beyond
successful filesystem replacement, semantic equivalence for unsupported
vendor constructs, or native vendor-project generation.

## Developer CI exit classification

The `trust-dev` workbench uses the same stable CI failure classes as the runtime
entrypoint: invalid project or configuration input is 10, build or compile
failure is 11, ST assertion or runtime-test failure is 12, timeout is 13, and
an unclassified internal failure is 20. Matching is ASCII case-insensitive.

Timeout recognition precedes test, build, and configuration recognition. A
recognized class is not replaced by command context. Only an otherwise
unclassified message uses command fallback: `build` maps to 11, `test` to 12,
`validate` to 10, and any other or absent command to 20. The free-form
substring classifier is a compatibility boundary; it does not imply that
arbitrary prose containing one of its tokens is semantically typed.

## API documentation generation

`trust-dev docs` uses an explicit project path when supplied. Without one, it
uses standard bundle detection and preserves a detection failure; it does not
invent the current directory as a project. Source discovery recursively
includes regular `.st` and `.pou` files under the resolved source root using
ASCII case-insensitive extension matching, deterministic project-relative path
order, and literal filesystem names. A matching directory, symbolic link, or
other special object is rejected rather than read or followed. An unreadable
or non-UTF-8 source and a source with parser diagnostics are generation errors.
The complete source set is read and parsed before the output directory or any
document artifact is changed.

The supported API declarations are `PROGRAM`, `TEST_PROGRAM`, `FUNCTION`,
`FUNCTION_BLOCK`, `TEST_FUNCTION_BLOCK`, `CLASS`, `INTERFACE`, `METHOD`, and
`PROPERTY`. Each item reports its exact declaration kind, namespace/class/
function-block/interface-qualified name, project-relative source path,
one-based declaration line, declared `VAR_INPUT`, `VAR_OUTPUT`, and
`VAR_IN_OUT` names in declaration order, and whether the declaration has a
return value. Ordinary local, temporary, external, access, and global
variables are not parameter metadata. Duplicate qualified names in different
source locations remain distinct source-qualified items.

A documentation block is the uninterrupted line- and/or block-comment sequence
directly preceding a declaration. One line ending between the block and
declaration is allowed. A blank line or any intervening non-comment token
breaks association. Comment delimiters, conventional leading block-comment
stars, and surrounding whitespace are removed while line order is preserved.
An adjacent untagged line is an ordered detail line.

Recognized tags are ASCII case-insensitive `@brief`,
`@param <name> <description>`, and `@return <description>`. Non-tag lines after
a recognized tag continue that tag with one separating space until another tag
begins. Parameter-name matching is ASCII case-insensitive. A return tag is
valid for functions and typed methods or properties; it is invalid on a
non-returning declaration.

Missing tag descriptions, malformed parameter tags, a parameter absent from
the declaration, `@return` on a non-returning declaration, duplicate recognized
tags, duplicate parameter entries, and unknown tags produce visible
documentation diagnostics at the exact physical tag line. The first valid
`@brief`, `@return`, or case-insensitive parameter entry remains authoritative;
a duplicate is diagnosed and cannot replace or duplicate the first rendered
entry. The absence of a tag for a declaration or declared parameter is allowed.

Documentation-tag diagnostics are nonfatal quality findings. Generation
succeeds only when every diagnostic is included in the selected documents and
reported in the command summary. Source discovery, source decoding, parser,
project resolution, output safety, and filesystem publication failures are
fatal and must not publish documents.

Items are ordered by source path, declaration line, and qualified name.
Diagnostics retain source and physical-line order. Markdown and HTML contain
the stable document header, declaration identity, source location, return and
parameter metadata, tag content, and any diagnostic section. HTML escapes
`&`, `<`, `>`, double quotes, and apostrophes in every user-controlled name,
path, description, detail, and diagnostic. Markdown descriptions and details
remain author-controlled Markdown, while generated structural metadata remains
unambiguous.

The selected format owns a fixed artifact set: Markdown writes `api.md`, HTML
writes `api.html`, and `both` writes both. An unselected pre-existing artifact
is unchanged. The complete selected set is rendered and staged beside its
destination before publication. Output components and existing artifacts may
not be symbolic links or special filesystem objects. A successful invocation
atomically replaces the complete selected set and only then prints success. If
any selected artifact cannot be staged or published, all pre-existing output
bytes remain unchanged and no partial document, empty output directory, or
temporary artifact remains. A valid project with sources but no supported API
declarations produces a deterministic empty API index; a project with no
supported source files is an error.

These rules define documentation extraction and filesystem publication. They
do not certify semantic correctness of authored prose, require documentation
for every declaration, or provide multi-process output isolation.

## Interactive numeric prompts

Numeric wizard/setup prompts accept unsigned base-10 values in the complete
`u64` range. Negative values, non-decimal text, and overflow are errors that
name the prompt field; they are not clamped, wrapped, or replaced with the
default. An empty submitted line selects the displayed default.

## Optional project resolution

Developer commands with an optional `--project` use the explicit path when it
is present. Otherwise they use standard bundle detection. If detection fails,
the command preserves that failure and must not substitute the current
directory as an invented project. This applies to package-registry publish and
other optional-project workflows, except when the owning specification
explicitly defines a first-run creation flow. The runtime launcher contract
defines that narrow exception for `trust-runtime play`.

## HMI scaffold command

`trust-runtime hmi init|update|reset` follows the optional-project resolution
contract above. An explicit `--project` path is authoritative; without it, the
command uses standard bundle detection and preserves a detection failure
instead of silently substituting the current directory.

Before changing `hmi/`, the command discovers the complete recursive `.st` and
`.pou` project source set, including resolved local dependencies, and compiles
that source set into runtime metadata and an initial value snapshot. The
absence of any supported source, unreadable or invalid source metadata,
dependency-resolution failure, or compile failure aborts the command before
scaffold files are created, replaced, or backed up.

`init` creates the generated descriptor and fails on a non-empty existing
`hmi/` directory unless `--force` is supplied. `update` preserves existing
descriptor files and user layout while merging missing generated pages and
signals. `reset` creates a backup snapshot of a non-empty existing descriptor
and then regenerates scaffold-owned files. Each successful command prints the
selected project `hmi/` path, the applied mode, and the deterministic scaffold
summary; a failed command must not print a false success summary.

For the same compiled metadata and value snapshot, generation is deterministic.
Externally visible globals and program members are included; internal compiler
or runtime symbols are excluded. If a program has only local symbols, the
generator emits the reviewed inferred-interface fallback instead of an empty
operator surface. Widget selection follows the declared type bucket and
writability, and annotations may refine labels, units, limits, page placement,
and presentation without changing the underlying binding identity. Invalid or
incomplete annotations fall back to the ordinary inferred metadata.

The generated scaffold contains the required overview, control, and process
pages. Repeated instance prefixes form separate deterministic sections. The
automatic process SVG uses grid-aligned instrument templates, and process
bindings preserve their reviewed fill geometry. Generated overview size,
configuration version, and widget count remain within the published scaffold
budget.

`update` adds missing generated signals and files without overwriting custom
widgets or replacing an existing custom process page. It omits the default
control page when there are no writable points. The loader discovers and sorts
descriptor pages deterministically, promotes `process.auto.svg` to the active
custom asset, and merges defaults, annotations, and file overrides in that
order. Invalid descriptor TOML is not published. A legacy single-file HMI
descriptor is used only when the `hmi/` directory is absent; the directory
form takes precedence whenever it exists.

These command, filesystem, and scaffold-lifecycle rules are truST product
contracts. IEC 61131-3 does not define an HMI descriptor generator, so they are
neither IEC decisions nor IEC deviations.

## Deprecated runtime workbench aliases

`trust-runtime test`, `trust-runtime docs`, `trust-runtime commit`, and
`trust-runtime agent serve` are compatibility aliases for the identically
named `trust-dev` workbench commands. Each alias preserves all selected paths,
text values, flags, timeouts, and output/format choices as operating-system
arguments. Test output values are `human`, `junit`, `tap`, or `json`; docs
format values are `markdown`, `html`, or `both`.

Before starting the child command, every alias writes a deprecation warning to
standard error that names both commands and the removal-not-before date
2026-10-05. Removing an alias requires a separate behavior-change release.

The child executable is selected in this order: an explicit `TRUST_DEV_BIN`
path, an executable regular-file `trust-dev` sibling of the current
`trust-runtime` binary, then `trust-dev` from `PATH`. A directory,
non-executable file, or other unusable sibling must not shadow a usable PATH
command. An explicit but unlaunchable path fails with installation guidance;
it is not silently replaced by another executable.

The child inherits the alias process streams so interactive and machine
protocols remain usable. Child success returns success, a normal non-zero exit
status is propagated exactly, and on Unix signal termination maps to the
conventional `128 + signal` status. A launch failure or a termination without
a representable status is an explicit command error rather than success.

## Project-Scoped Commit

`trust-dev commit --project <path>` owns only the selected project path. Before
staging or committing, it must resolve that path canonically as an existing
directory contained by the repository; an unresolved or escaping scope fails
closed. It must inspect the existing Git index both before presenting the
summary and again after interactive prompts, immediately before mutation:

- a pre-staged path intersecting the selected project scope is a collision and
  aborts the operation before index or worktree mutation;
- a pre-staged path outside the selected project scope remains staged and is
  excluded from the project commit;
- when the selected project is the repository root, any pre-staged path is a
  collision;
- staged additions, modifications, deletions, renames, non-ASCII paths, and
  mixed staged/unstaged paths are all subject to the same intersection rule;
- cancellation and `--dry-run` do not mutate the index, worktree, or history.

The collision diagnostic must name the intersecting path. The helper never
silently absorbs an existing staged change into the commit it creates,
including one staged while the prompt is open.
