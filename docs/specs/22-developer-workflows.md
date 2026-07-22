# Developer Workflow Contract

This document defines normative truST behavior for developer-facing source
discovery and project-scoped Git commits. It does not define IEC 61131-3
language semantics.

## Project initialization

When the product wizard initializes Git, an existing `.git` directory or valid
worktree `gitdir:` file means the project is already initialized. An ordinary,
malformed, or dangling `.git` file is an error; the wizard must not silently
report Git initialization success. Without an existing marker, initialization
requires an available `git` command and a successful `git init` exit status.

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

Remote browser setup tokens contain 128 bits from the operating-system secure
random source. If that source fails, setup aborts before binding the server; it
must not use zero bytes, timestamps, process IDs, or another fallback token.
The configured minute TTL is converted and added to the current Unix time with
checked arithmetic. Overflow is a setup error rather than a wrapped or
effectively unbounded credential lifetime. Local loopback setup remains
token-free; remote setup accepts the unexpired token from `X-Setup-Token` or
the `token` query parameter.

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
other optional-project workflows.

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
