# Phase 18 versioned-deploy focused evidence

Scope: versioned bundle installation, deployment-pointer preflight, successful
summary preservation, configured ADS and OPC UA client sidecar closure, and
symbolic-link rejection.

## Expected-red evidence

Remote isolated checkout:
`/home/johannes/projects/trust-platform-p18-parser-proof-20260723` on
`trust-builder`.

```text
cargo test -p trust-runtime --test deploy_cli_command
```

Result before the production fix: 6 passed and 3 failed. The failures proved
that an invalid `previous` slot allowed `current` to advance, enabled protocol
sidecars were omitted from the installed bundle, and a source symlink was
followed.

```text
cargo test -p trust-runtime --test deploy_cli_command \
  deploy_rejects_symlinked_sidecar_parent_directories -- --exact
```

Result before the production fix: failed because the sidecar parent symlink was
followed and the candidate bundle was retained.

The frozen-patch adversarial review then identified two additional uncovered
partitions: a stale `previous` pointer when no valid `current` existed, and
dangling optional bundle-entry symlinks. Focused regressions were added before
the reviewed fix was accepted.

## Focused green evidence

```text
cargo test -p trust-runtime --test deploy_cli_command
```

Result: 12 passed, 0 failed with `--test-threads=1`.

```text
cargo test -p trust-runtime --bin trust-runtime deploy::tests::
```

Result: 11 passed, 0 failed.

No broad workspace test, `just fmt`, `just clippy`, or `just test-all` command
was run for this batch.
