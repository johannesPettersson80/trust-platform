---
name: trust-release-hygiene
description: Enforce trust-platform release hygiene for user-visible changes. Use when implementing features, fixes, CLI/runtime behavior changes, documentation updates reflected in release artifacts, changelog/version sync, or tag/release verification.
---

# Trust Release Hygiene

## Workflow

1. Confirm release scope.
   - Treat runtime behavior, CLI flags/output, standard-library behavior, docs/tutorial updates, and test harness behavior as release-notable.
   - If a change is purely internal with no user-facing effect, keep changelog/version churn out unless explicitly requested.

2. Update changelog first.
   - Edit `CHANGELOG.md` under `## [Unreleased]`.
   - Keep entries under `### Added`, `### Changed`, or `### Fixed`.
   - Mention user-observable behavior and test/developer experience changes.
   - Update the target release line when the version is bumped.

3. Bump versions when release-notable.
   - Bump `[workspace.package].version` in `Cargo.toml`.
   - If VS Code extension behavior changes, bump `editors/vscode/package.json` and matching root entries in `editors/vscode/package-lock.json`.
   - Whenever the workspace version changes, keep the VS Code package versions synchronized to the same value.

4. Sync docs and examples.
   - Ensure tutorials, examples, specs, and coverage docs reflect shipped behavior.

5. Validate.
   - Inventory the current pull-request workflow jobs and exact command shapes before candidate
     preparation. For runtime changes, require the exact-SHA artifact's
     `remote_cross_target_warnings` record from
     `./scripts/check_runtime_cross_target_warnings.sh --install-missing --require-cross`; Linux
     Clippy or tests alone are not Windows proof.
   - In trust-platform checkouts on a Raspberry Pi or other slow local host, use the remote builder for broad/full validation first, especially `just test-all`.
   - Ask before starting expensive local commands such as workspace `cargo test`, `cargo test -p trust-runtime ...`, local `just test`, local `just clippy`, or local `just test-all`.
   - Cheap local checks are allowed when narrowly scoped.
   - Required full gates before completion: remote `just fmt`, `just clippy`, and `just test-all`,
     plus feature-specific runtime/VS Code gates when behavior changed.
   - If the candidate changes `editors/vscode/**`, `scripts/captures/**`, capture assets, or the
     capture workflow, require a successful `Docs Captures` pull-request run on the exact candidate
     SHA before merge. A post-merge capture failure is not candidate proof.

6. Preflight the first push.
   - Record `git remote get-url origin`, `git remote get-url --push origin`, and
     `gh auth status --hostname github.com` without exposing credentials.
   - If the push changes `.github/workflows/**`, do not wait for GitHub to reject an HTTPS OAuth
     token that lacks the `workflow` scope. Keep HTTPS for fetches, configure an authenticated SSH
     push URL, and verify it without mutating the remote:
     `git config remote.origin.pushurl git@github.com:johannesPettersson80/trust-platform.git` then
     `git ls-remote --exit-code "$(git remote get-url --push origin)" HEAD`.
   - Report the fetch and push transports used. Never print tokens or credential-helper output.

7. Final release proof when a version is bumped.
   - Merge or fast-forward the exact green PR SHA to `main`.
   - Create and push annotated tag `v<workspace-version>` from that same `main` SHA.
   - Monitor both main CI and the tag-triggered Release workflow.
   - If the main version guard expires only because the matching Release is still running, wait for
     Release success and rerun the failed main jobs on the same SHA; never invent another tag.
   - Confirm the GitHub release is published and marked Latest.
   - Verify the expected release-asset inventory and confirm published checksums/digests match the
     uploaded artifacts; workflow success alone is not public artifact proof.
   - When VS Code Marketplace publishing is configured, poll the official Marketplace Gallery API
     until the target version is publicly latest for every published target (`darwin-arm64`,
     `darwin-x64`, `linux-arm64`, `linux-x64`, and `win32-x64`). Publish-job success alone is not
     propagation proof.
   - Do not report release complete until tag, workflow, GitHub Latest/assets, and Marketplace
     propagation state are verified separately.

8. Verify issue closure.
   - Use `Closes`, `Fixes`, or `Resolves` in PR text when merge should close an issue;
     `Addresses` alone does not auto-close it.
   - Verify the intended issues are closed after merge and close them explicitly when required.
