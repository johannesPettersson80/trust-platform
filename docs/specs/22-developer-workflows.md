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

The nine single-file programs under `examples/tutorials/` are shipped product
artifacts. Every file must compile through both the in-process runtime harness
and the current source-to-bytecode path. Success requires both paths to return
without a compile, type-check, lowering, or validation error; this bounded
fixture set does not certify arbitrary Structured Text programs.

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

### CI performance reference-environment gap

Elapsed-time thresholds for first feedback, one filtered test, and small-suite
scaling are not portable product oracles until a reference environment names
the CPU and memory, operating system, prebuilt-versus-clean state, cache state,
test concurrency, sample count, warmup, and aggregation statistic. The current
“clean setup” test invokes already built binaries against a copied fixture; it
does not measure installation or compilation from a clean toolchain state.

Until that authority exists, those timing assertions remain runnable
environment probes and must not be counted as performance coverage.

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

`trust-dev docs` associates a directly preceding tagged comment block with its
Structured Text declaration. The supported tags are `@brief`,
`@param <name> <description>`, and `@return <description>`. Extraction reports
the declaration kind, qualified name, source path and line, declared
parameters, return presence, and collected tag text.

Missing tag descriptions, a parameter absent from the declaration, `@return`
on a non-returning POU, duplicate recognized tags, and unknown tags produce
visible documentation diagnostics. The reviewed Markdown and HTML renderers
emit deterministic document headers, declaration identity, source location,
parameter and return metadata, and tagged content. HTML rendering escapes
user-controlled text before inserting it into markup.

The existing focused snapshots define the representative Markdown and HTML
shape for their fixtures; they do not establish every escaping or malformed
input partition. Command failure policy for a generated document containing
diagnostics requires separate specification authority.

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
