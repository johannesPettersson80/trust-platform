# Run 9 independent reconciliation and import correction review

The existing A4 checkout still matches canonical agent instructions and skills.
Reviewer did not author implementation or run tests, builds, formatters or validators.

All 50 indexed raw artifacts match locally and on the builder. The artifact-index
SHA-256 is `c92467f45981427281c5dc466489a713eab1f0e82fed343d12e4236637cacfff`.
The 612-record frozen manifest has SHA-256
`0b37c8fbaacd6f101e916f793cc549f3c211b8beebd2062a04a44a72eaf162f2`.
The full post-batch archive matches the frozen records.

The ledger reconciles to 26 required passes, three required failures and two
passing advisories. Core all-features passed 316, portable passed 276, i686
passed 197, and hosted unit tests passed 3,841. Those suites have no failed or
ignored tests. MCU checks passed. The integration invocation, Clippy and cross-target
warnings failed on the same inaccessible ensure_global_call_depth import. Expected
compile-fail doctest diagnostics are passing negative tests, not additional failures.
No integration-suite success or physical board execution is claimed.

The correction changes only the import in vm_resource_limit_cases.rs to
trust_runtime::runtime_core::vm::hosted::ensure_global_call_depth. The hosted facade
publicly re-exports the same function from the shared frame implementation, and
trust-runtime enables the required HIR feature. The root re-export of runtime_core
still refers to trust-runtime-core. Public limit constants remain at their existing
root paths. The scenario calls, tested limit and rejection assertions are unchanged.

Archived before bytes match the retained builder source. Before/after hashes are
recorded in run-9-correction-independent-source.json. This fixes the owning caller
without reopening the private portable implementation or suppressing visibility
checks. No further source-level blocker found in this focused review. Integration
execution and the corrected full candidate still require their consolidated batch;
source review cannot establish the unexecuted assertions.
