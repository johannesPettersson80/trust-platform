# A1 independent source-review record

These are retained messages from the independent read-only reviewer in the implementation conversation, exported here for ordinary repository access. The reviewer identifier used in that conversation was a logical agent identifier, not a filesystem evidence path. No access to a privileged directory is needed for this record. The author exported these messages; they are not a signed third-party attestation.

## Review before run 2 (verbatim final-message content, identity lines omitted)

One remaining actionable finding:

- **P2 — TLS validation selection:** `host::mesh::tests::mesh_tls_publish_applies_updates` checks local readiness after setting a configuration flag; it does not exercise the upgraded rustls builders. Include existing `host::security::contract_tests` covering valid and invalid client/server TLS configuration in batch 2.

The spec 11 allocation contradiction identified during review is now corrected. No additional source defects found in Salsa return ownership/equality, dependency changes, MySQL predicate equivalence, TIME fixture correction, or allocation metadata contracts and assertions.

Remediation identity: SHA-256 `9560361ad7b34b24877d0d29be9d4358dcf9c80b802cafe3abb4d098f4ad5e0f`, over the 14 reviewed manifest/lockfile, source/test, metadata/schema and spec files, sorted by path and hashed as `path + NUL + contents + NUL`.

No edits, builds, tests or validators executed. No runtime claims.

Disposition: actual security configuration tests and HTTPS handshake integration were added before run 2. The blanket allocation prose was corrected. This review did not catch the duration operand-context compiler defect exposed by run 2.

## Review before run 3 (verbatim finding text)

No actionable findings in the post-run-2 changes.

- Shared lowering now separates duration and scalar contexts; the existing registry helper resolves TIME/LTIME aliases. Unary/parenthesized scalar expressions retain numeric contexts.
- The original failing fixture remains intact. Added tests cover duration widths, alias destinations, nested factors, truncating division and reversed-division rejection. Full `LTIME#` syntax is consistent.
- LSP guarded arms preserve both first-reference recording and duplicate rejection.
- Metadata wording changes preserve the existing validation rule.

Five-file review SHA-256: `64550b9f0909f4292b53cdfcc583c79a469e63418d77a1dbf0c31a26e7abd992`—sorted path/NUL/content/NUL for lowering, numeric tests, LSP runtime values, taxonomy and spec 10.

Read-only review; no builds, tests, formatting or validators ran. These fixes remain unverified by execution.

Disposition: run 3 subsequently executed the reviewed corrections successfully. Later bookkeeping edits to the taxonomy are outside that five-file review and the run-3 snapshot; the frozen manifest preserves the tested identity.

## Earlier review provenance

The initial A1 review and separate scheduling review reported no actionable findings. This is the author's summary, not a recovered verbatim transcript. Initial reviewed dirty-path hash: `68a0675da6d79c39a4738fd553f63305762b7ecb61a8dd3a3860183c5cacb7e0`. Its missing full transcript remains a provenance limitation; the exported final reviews above and native logs are directly inspectable.

## Authorization provenance

Three batches were explicitly authorized in the implementation conversation: initial A1 implementation/review/batch, one additional batch after run 1, and one additional batch after run 2. This is the author's conversation record, not independently authenticated approval evidence. No fourth batch was run for this bookkeeping correction.

## Post-closeout documentation/metadata source review

The independent reviewer read schema/validator sources without executing them. Two introduced inventory defects were found and corrected: the artifact name must match the literal matrix expression in the workflow; the new gate must also appear in the PR suite's commands, command bindings and inventory IDs.

Final reviewer message (verbatim):

> Confirmed: `pr.toml` now contains the portable-core command, matching command binding and inventory ID. The remaining review finding is resolved.
>
> No unresolved findings from this documentation/metadata source review. No validators, tests or builds ran; metadata validation remains unclaimed.

This review applies only to the post-closeout bookkeeping and does not replace run-3 implementation evidence or the missing full pre-push gates.
