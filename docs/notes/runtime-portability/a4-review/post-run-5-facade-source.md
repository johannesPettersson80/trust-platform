# Q4 facade implementation handoff

Authored source only, uncompiled and untested. The paired JSON retains exact path hashes and aggregate framing for independent review.

VM implementation modules are private. HIR-enabled hosted adapters use vm::hosted; compiler-free entry points remain PreparedModule/RuntimeState with typed values and stable errors. VmModule exposes immutable metadata methods. Explicit legacy construction rejects STBC 2.0 and keeps verifier fixtures able to exercise malformed legacy bodies without forging source-free admission. Host fixture mutations now use methods that maintain associated tables or replace a single POU body.

The VM and newly extracted stdlib blanket missing-doc allowances were removed. Portable StandardLibrary, conversions/time and typed function blocks remain documented APIs. Registration-only leaves and shared implementation helpers are reached through stdlib::hosted; public host reexports stay available. Conditional HIR compile-fail examples preserve the exact E0616 metadata-privacy and E0277 state-context separation contracts; they are absent when the hosted adapter feature is absent.

New include-fragment formatting inputs beyond the existing run-6 list: runtime/vm/register_ir/tests/backend_parity_followup.rs and runtime/vm/register_ir/tests/lowering/verifier_and_parity.rs, both under crates/trust-runtime/src. No formatting was executed here.

Independent source review found and corrected two remaining reference-policy path copies in native read_checked bindings; peer author replaced temporary owned references with borrowed ValueRefView. The prior journal/cache source review remains applicable; final context-owned-budget migration requires separate review.
