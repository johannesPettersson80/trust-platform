# B-R4 metadata and entry-policy boundary review

Source accepted by independent reviewer `/root/br1_static_registry`. Three-file identity 92d07b89c7af4237bc0ea2681738bc823151699adfb49ebc6bc9deec4800493b. No source edits, compiler, tests, formatter, linker or hardware execution.

engine/mod.rs exactly restores the pre-run6 phase-split version pinned in run5; the unfavorable run6 measurement is retained separately. The other two changes add only a purpose comment and inline(never) to existing metadata entry and EngineState enter_type. Bodies, signatures, callers, charges, limits, error messages and cleanup paths are unchanged. These helpers finish their work before recursive recipe/node processing, so explicit boundaries do not introduce another execution implementation or persistent allocation.

The source/linked-owner rationale is specific: metadata entry was inlined, although TypeEntry::clone itself already had a separate270-byte symbol; therefore no claim is made that the complete derived clone body is newly moved. EngineState enter_type depth arithmetic, membership scan, allocation charge/reservation and push were visible directly in run5 construct at0x08030e9a–0x08030f80, with no separate symbol. These temporary entry operations need not remain in every recursive construct frame.

No particular stack/flash saving or readiness is claimed. Another source-bound linked measurement and full applicable native/physical validation remain necessary. Existing construction budget/order, recursion, lifetime and restart assertions remain unchanged.
