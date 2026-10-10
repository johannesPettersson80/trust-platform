# A2 evidence

Expanded A2 is verified for its allocated scope through the passing union of runs 6–8.
The [execution record](../a2-execution.md) maps each failure to its correction and later
passing check. Earlier failed runs remain failed; no single run is presented as proving
hardware execution, firmware linking, WCET, native Windows/macOS behavior or publication.

- Run 6: 204 core assertions including one doctest, 53 portable x86_64 assertions and
  167 i686 assertions passed. Both MCU core checks, no-dev feature graphs, supply chain,
  architecture, rendered diagrams and drift passed. Hosted integration passed 197 assertions
  and failed one new fixture; other failures are retained in its ledger.
- Run 7: 3,858 runtime unit tests and 102 tooling tests passed. Clippy, host/Windows warning
  checks and the 1,029-record metadata check passed. VM integration passed 43 assertions
  and failed one fixture; explicit fragment formatting also failed.
- Run 8: all nine required steps passed, including the 44-test VM integration suite and
  corrected two-instance NULL regression, formatting, lint and cross-target warnings.

The final run-8 manifest contains 252 file/deletion records and matched local source after
copying back formatted files and generated diagrams, before closeout/evidence edits.
The subsequent support-fragment change after run-7 unit execution is rustfmt-only;
those unit tests were not executed again. Core production code has not changed since run 6.

`artifact-sha256.json` records hashes of the retained files. `.txt` outputs are byte-for-byte
copies of original `.log` files, avoiding the repository's `*.log` ignore rule. Full raw logs,
scripts and the final frozen-source archive remain in the workspace A2 artifact directory.
The retained batch scripts and command logs identify exact invocations and configurations.

Historical records:

- Runs 1–3 qualified the original relocation, before the expanded correction scope.
  Zero-test helper binaries were not counted as native evidence.
- Run 4 retained four required failures and two dependent unrun diagram checks.
- Run 5 passed three formatting steps, then failed ambiguous mutation-selector preparation.
  Its raw ledger contains four executed steps; the companion complete ledger lists twenty
  unrun steps. Its stopped-source manifest is a recovery snapshot, not a successful freeze.
- `pre-run-5-review-manifest.json` enumerates that historical source-review candidate's
  paths and SHA-256 digests (null means deletion), plus its explicit exclusions. Its JSON
  digest identifies a source review, not an executed batch or the final candidate. The older
  ambiguous 118-path identity remains unconfirmed and is not used for final qualification.

Active mutation shards remain planned. Historical measured artifacts are preserved, not
promoted to current results. No new mutation campaign or fuzz-adequacy claim is made.
