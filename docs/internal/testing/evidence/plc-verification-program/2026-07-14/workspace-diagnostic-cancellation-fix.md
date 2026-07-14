# Workspace diagnostic cancellation product fix

Date: 2026-07-14

## Missing contract and test

`SPEC_GAP_EDITOR_DIAGNOSTIC_CANCELLATION_001` asked how push, pull, and
workspace diagnostics distinguish cancellation from successful empty output.
The product contract is now written in `docs/specs/14-lsp.md` section 6.2.6.
Existing focused tests cover push and pull cancellation. A new deterministic
workspace test invalidates a semantic request ticket before collection and
requires `ContentModified` for the complete request.

## Red result

Clean commit `e7ec6c4d8f53c80b200811ab37df3c2a4a876c9b` was run on
`trust-builder` with:

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-lsp \
  lsp_workspace_diagnostics_cancelled_request_returns_content_modified \
  -- --nocapture
```

The new test failed with exit 101 because the stale request returned a
successful report:

```text
cancelled workspace diagnostics should return ContentModified:
Report(WorkspaceDiagnosticReport { items: [] })
```

This was a product defect: the workspace loop stopped on cancellation but
returned the documents collected so far as successful output. In the
zero-document reproduction, that became a false empty success.

## Green result

At clean commit `4ab4f5c4e7367102d9d72b5d3cf4ac834757652e`, workspace
diagnostics check the request ticket before collection, around each document
and configuration pass, and before returning success. Cancellation now returns
`ContentModified` through the production handler.

```text
cargo test -p trust-lsp \
  lsp_workspace_diagnostics_cancelled_request_returns_content_modified
# 1 passed

cargo test -p trust-lsp diagnostics_cancelled
# 3 passed: pull, workspace, and push cancellation

cargo test -p trust-lsp lsp_workspace_diagnostics
# 2 passed: cancellation and unchanged-result behavior
```

## Honest posture

The written contract and exact push, pull, and workspace tests close the
specification gap. `EDIT_DIAG_CANCEL_001` remains `S0/gap_open`: these ordinary
Rust tests do not emit same-run case artifacts, so producer-authentic red/green
proof and a causal broad gate remain explicit debt.
