# B-R4 run 1 fixture compile failure (source frozen)

The runtime-integration log reports exactly one failed target, source_free_execution_cost, with five passing tests and one failure in deferred_calls_preserve_null_presence_and_suspended_local_copyback. The failure is CompileSession source parsing, before bytecode preparation or runtime behavior executes. This is not a failing runtime assertion.

All four ranges (80..87, 128..135, 190..197, 399..406) in the raw ST string equal `pointer`. token_kind.rs lines185–186 explicitly define case-insensitive POINTER as KwPointer. The complete fixture correction is four identifier substitutions `pointer` -> `ref_arg`: declaration, NULL comparison, dereference and named argument. No other source or assertion change is needed by this diagnosis.

The existing runtime_core_compiler_free_load.rs test around lines738–744 uses `ref_arg : REF_TO INT := REF(global_value)` plus an explicit `ref_arg := NULL`; its corresponding syntax/behavior path was included in the passing run-1 corpus. The renamed new fixture still expects 133: local5 becomes7; copied becomes10; omitted reference returns7 and supplied NULL returns9, yielding16; Outer returns33; Main adds100. Preserving the exact 133 assertion is required.

Read-only source and retained remote log inspection only. No edits to repository source, compiler invocation, test, formatter, link or hardware command. Other independent batch steps were still running at inspection time; this note cannot certify their eventual result. Proposed correction remains unimplemented and unvalidated pending the completed ledger.
