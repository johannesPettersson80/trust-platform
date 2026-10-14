# H historical fixture boundary

Source authoring only; no build or test evidence is claimed by this note.

`runtime/mod.rs` excludes `core`, `cycle`, `faults`, and the entire historical
`vm` module from every non-test build. The historical `core::Runtime` is exposed
only as the crate-private, `cfg(test)` `LegacyRuntimeFixture`. Product `Runtime`
remains `installed::Runtime`, backed by the common owned state. Public register
snapshot types reexport the canonical core directly; they do not depend on this
fixture. OPC UA configuration now takes the installed controller.

The historical VM/register corpus keeps its assertion bodies and explicit raw
fixture setup. Its test-only `LegacyTestHarness` uses the existing authoring
lowerer and explicit legacy materializer, then the private legacy encoder. It
selects STBC 1.1 explicitly rather than changing the public 2.0 producer default.
The helper exposes only the cycle and direct output reads used by that corpus.
Full boundary grammar remains exercised through the public installed harness.

`legacy_restart`, `legacy_retain_snapshot`, `legacy_bytecode`,
`legacy_retain_store`, and `legacy_online_change` contain the historical fixture
support required by that corpus; they are also `cfg(test)`. Their old lifecycle
test modules are not duplicated: the assertions were migrated in the original
`restart`, `bytecode` and retain presentation test owners. Unused transport-start
methods were removed from the private fixture; product transport methods belong
to installed service adapters.

The old HIR local-initializer interpreter is reachable only through the private
historical fixture. Explicit 1.1 production remains non-executable authoring and
serialization, not a product RUN path. Installed source construction, restart,
retained restoration, POU entry and cycles use the common bytecode engine.

Pending proof belongs to H's consolidated batch: compilation of both test and
non-test graphs, existing historical corpus, admitted 2.0 register parity,
installed lifecycle/debug/protocol tests, and architecture checks. A private
fixture assertion is not itself evidence that the installed engine passed.

The production environment adapter is separate from that fixture. Its immutable
`RegisterExecutionConfiguration` delegates to the canonical core parsers for the
five existing `TRUST_VM_*` controls. Hosted services capture it once, before
execution-state reservations. Selection capacity limits the fixed prepared LRU;
it does not reserve an environment-sized array or permit lazy compilation.
Injected-resolver native cases preserve all boolean spellings, numeric defaults,
overflow fallback, zero-threshold behavior and minimum capacities without global
environment mutation. Core native cases pin bounded reservation, rejection at
the construction allowance, and reset/restart behavior. These tests are authored
but have not run.

Same-module restart preserves the existing register counters, enabled settings
and selected LRU after successful candidate construction, matching the historical
`core::Runtime::commit_restart` which did not replace those fields. Explicit
profile reset clears observations without clearing selection; explicit tier reset
clears selection and hits while retaining prepared code. Replacing an artifact
carries the current settings into fresh reservations for its new prepared plans.
