# Developer Workflow Contract

This document defines normative truST behavior for developer-facing source
discovery and project-scoped Git commits. It does not define IEC 61131-3
language semantics.

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

## Project-Scoped Commit

`trust-dev commit --project <path>` owns only the selected project path. Before
staging or committing, it must inspect the existing Git index:

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
silently absorbs an existing staged change into the commit it creates.
