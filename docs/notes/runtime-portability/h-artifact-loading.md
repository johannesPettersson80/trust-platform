# Hosted supplied-artifact loading

Source-only H caller integration; tests are authored, not executed.

The previous CLI source loader built a runtime, built bytecode again and applied
it. Bundle override then applied the bundle bytes a third time. The admitted H
factory now constructs directly from the supplied artifact; source-only loading
calls the ordinary source factory once. Service policy setup is named separately
and never installs code. The explicit startup restart selected by CLI options is
retained: eliminating duplicate loader construction does not erase warm
AFTER_RESTART or durable checkpoint behavior.

`CompileSession::build_runtime_from_bytecode` owns the dual-major boundary. 1.1
requires matching authoring context and uses the existing importer's exact table
projection and appended construction bytecode. 2.0 accepts an empty source list.
When sources exist, source/debug association requires identical artifact records
and instructions except equivalent file-label relabels. For 1.1 the retained
explicit legacy producer supplies the comparison outside RUN: a mismatch or
unrepresentable comparison clears source text/statement associations while
preserving verified declarations/types/USING context. This does not replace
supplied code or claim a cryptographic source hash. A 2.0 mismatch uses explicitly
source-free metadata. Supplied ordinary code is never discarded or regenerated.
Deployment validation uses the same factory rather than an empty live Runtime.

Native authoring covers supplied body changes versus unchanged source context,
legacy-context rejection, unchanged existing owner on failed context construction,
2.0 without sources, correct source path association, CLI bundle loading of both
majors, and deployment's source-required legacy/source-free split. A formerly
empty placeholder artifact in the source-discovery fixture is now valid produced
bytecode; source/dependency assertions are unchanged.
