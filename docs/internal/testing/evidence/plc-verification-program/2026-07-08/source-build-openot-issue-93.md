# Issue #93 OpenOT Source-Build Proof

Date: 2026-07-08

Base commit: `6a7492cae6af`

Worktree state: dirty; this proof covers the local fix slice for GitHub issue
`#93`, not a clean `main` release.

Platform:

```text
Linux raspberrypi 6.18.33-v8-16k+ #1983 SMP PREEMPT Wed May 27 17:26:29 BST 2026 aarch64 GNU/Linux
rustc 1.95.0 (59807616e 2026-04-14)
cargo 1.95.0 (f2d3ce0bd 2026-03-21)
```

## Claim

`trust-runtime` source builds must not require a pre-existing sibling
`../open-ot-ref` checkout just to resolve the Rust crate graph.

## Fix Shape

- `crates/trust-runtime/Cargo.toml` now uses pinned public Git dependencies for:
  - `open-ot-definition`
  - `open-ot-carriage`
  - `open-ot-shm`
  - `open-ot-conformance`
  - `open-ot-document`
- `Cargo.lock` records all five OpenOT crates from
  `https://github.com/johannesPettersson80/open-ot-experiments.git` at
  `137f0e765f085c262651f479be35298b836ac891`.
- `docs/public/start/install-from-source.md` states that the sibling checkout is
  optional and only needed for OpenOT IEC examples/tests that read ST source
  files.

## Commands

```bash
cargo metadata --format-version 1 --no-deps
cargo check -p trust-runtime --no-default-features
cargo check -p trust-runtime
cargo check -p trust-runtime --locked
bash -n scripts/checkout_openot_ref.sh
rg -n 'open-ot-[a-z-]+ = \{ path = "../../../open-ot-ref' crates/trust-runtime/Cargo.toml Cargo.lock || true
git diff --check -- crates/trust-runtime/Cargo.toml Cargo.lock docs/public/start/install-from-source.md CHANGELOG.md
```

## Result

- `cargo metadata --format-version 1 --no-deps`: passed.
- `cargo check -p trust-runtime --no-default-features`: passed; Cargo fetched
  and checked OpenOT crates from the pinned public Git revision.
- `cargo check -p trust-runtime`: passed.
- `cargo check -p trust-runtime --locked`: passed.
- `bash -n scripts/checkout_openot_ref.sh`: passed.
- Remaining OpenOT Rust path-dependency grep: no matches.
- `git diff --check`: passed.

The original source-build failure mode is locally fixed for the Rust dependency
graph. Remote-builder/full-release proof still remains a release follow-through
step before issue closeout.
