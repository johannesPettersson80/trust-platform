# Expanded decoder sort inference correction

Independent read-only review confirms the retained ordinary-attempt compiler diagnostics E0282/E0283 identify the generic error type of the new heap_sort closure in bytecode/decode/section_validate.rs. The trailing question-mark conversion leaves multiple From candidates for BytecodeError. Explicitly selecting heap_sort::<BytecodeError> binds the closure result to the owning decoder error type and resolves that ambiguity without changing comparisons, charges, errors or operation order.

Reverting only that annotation reproduces the previously frozen file SHA exactly. It is the only changed path among the 6,661-record frozen source snapshot. Corrected file SHA-256: 9952b669aa6fa68239153289afd4df539958db80f024748c2874d28cf0e22d5a. The other three heap_sort calls are returned directly into declared Result return types, so their error types are constrained by their function signatures.

No build, test, formatter or linker was run by this reviewer. The first expanded attempt stopped before linking and provides no ordinary map or paired ICF baseline. Using the remaining authorized attempt with safe ICF must therefore be reported as a corrected-source measurement, not a same-source ordinary-versus-ICF saving. No extra attempt or automatic retry is authorized by this source review.
