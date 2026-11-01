# Contributing to truST Platform

Thanks for taking the time to contribute. This project is early-stage and still moving fast.

## Development setup

Prerequisites:
- Rust 1.99+
- Node.js 24.21.0 (for the VS Code extension)
- [just](https://github.com/casey/just) (task runner used by the repo)

Optional:
- [sccache](https://github.com/mozilla/sccache) (for faster local Rust builds)

Clone and build:

```bash
git clone https://github.com/johannesPettersson80/trust-platform
cd trust-platform
cargo build
```

Run formatting and checks:

```bash
just fmt
just clippy
just test
```

## VS Code extension (optional)

```bash
cd editors/vscode
npm install
npm run compile
```

## Pull requests

Work reaches `main` in small pull requests, and CI is the only gate (specification 22, "Change and
release workflow").

- **One topic per pull request**, a few hundred lines. When a pull request builds on another, say so
  in its description and base it on that branch, or wait until the other one is merged.
- **Specification first.** `docs/specs` defines the behaviour. Change the specification in the same
  pull request when the behaviour is missing or wrong there; when the change follows a section it does
  not change, name the section in a commit trailer, for example `Specification: 35 B3.4`.
- **Native tests** prove the behaviour: each test's expected values come from the specification, and
  each test names the wrong behaviour it catches.
- **IEC 61131-3.** truST's default dialect follows the standard. A departure that matches another
  toolchain (for example CODESYS) applies only under that `vendor_profile` and is recorded in
  `docs/IEC_DEVIATIONS.md`; PLCopen choices go in `docs/PLCOPEN_DECISIONS.md`.
- **All logic is in Rust.** The VS Code extension only presents and wires; it keeps no copy of a rule.
- **Changelog**: a user-visible change adds an entry under `## [Unreleased]` in `CHANGELOG.md`. Never
  edit or remove the entries of released versions.
- **Before pushing**, run `cargo fmt`, Clippy with `-D warnings` and the tests of what you changed. CI
  runs the full suites on Linux, macOS and Windows; a pull request merges when its required checks are
  green and the review has no open finding.
- Fill in the pull request template, and keep "Allow edits by maintainers" on. When `main`'s workflow
  files cannot be pushed to a fork, a maintainer carries the pull request: your commits, unchanged and
  with your authorship, plus a merge of `main`, in a branch of this repository. When that merges,
  GitHub marks your pull request merged too.
- Use clear commit messages (imperative mood).

## Code style

- Rust: `cargo fmt` is required.
- Clippy: `cargo clippy --all-targets --all-features -- -D warnings` should pass.

## Reporting issues

Use GitHub Issues with a minimal repro and expected vs. actual behavior.
