# B-R4 run3 instantiation native-stack diagnosis

Independent read-only analysis by `/root/br1_static_registry`. No edits, tests, builds, formatter, linker, resets or board commands. Disassembly comes from the already-linked final run3 ELF SHA3ad794db9695f5106c22d1bdc0fc4901c51dfca370999bc8520adef28fae8b7c. The retained UART reports main-prepare stack10256/free6128, then main-instantiate stack16384/free0 and an intentional runner stack-headroom failure. Saturation establishes failure of the reserved paint range, not the precise maximum beyond that range.

Ordinary POU continuation work is not the failing path yet: no PLC scan has run. Program instance construction still recursively invokes the shared initializer dispatcher for nested function-block declarations. The fixture has Main.counter:Counter, whose history field is ARRAY[1..3] OF INT. Its saved artifact explicitly contains DEFAULT_TYPED actions for Counter and history. Source construction order initializes Main, evaluates the counter declaration, constructs/initializes Counter, then evaluates history and recursively constructs its scalar element.

## Source-supported simultaneous path

Each number is the final ELF's own fixed frame, including pushed registers. Inlined helper costs are already included. This is a concrete feasible source path supported by direct linked call edges, not a sampled runtime backtrace or a complete worst-case proof.

| Active owner | Bytes |
| --- | ---: |
| firmware outer main | 152 |
| main_fixture | 2488 |
| instantiate_with_services (build and resource construction inlined) | 1824 |
| initialize_instance(Main) | 256 |
| run_declaration_action(counter) | 424 |
| evaluate_initializer(counter) | 968 |
| execute_with_buffers(counter default) | 2336 |
| construct(Counter) | 632 |
| node(Counter POU) | 672 |
| initialize_instance(Counter) | 256 |
| run_declaration_action(history) | 424 |
| evaluate_initializer(history) | 968 |
| execute_with_buffers(history default) | 2336 |
| construct(array) | 632 |
| node(array, with array loop inlined) | 672 |
| construct(INT element) | 632 |
| node(INT) | 672 |
| **Sum of these fixed frames** | **16344** |

This leaves only40 bytes before the full16384 reservation, even before leaf helpers, interrupt stacking or call-site adjustment; it cannot satisfy the required2048 headroom. It explains why reducing individual evaluator/dispatcher prologues and removing ordinary POU recursion did not solve instantiation. Logical depth4 still admits this fixture: native helper frames are not one-to-one with the logical depth.

The dominant avoidable common-frame costs are construct632 and node672 repeated at every type level. construct owns recipe lookup/entry clone plus Apply-Struct recipe/override merging; node includes array/struct construction and other substantial branches. The current linker has no separate construct_array or construct_struct symbols: these branches are inlined into node. Targeted separation of these existing branches into non-inlined helpers can keep aggregate-only temporaries out of scalar/POU common frames while preserving all cases, charges, error precedence and order. This is a source-grounded candidate and needs measured confirmation; no saving is promised.

instantiate_with_services also retains an1824-byte construction frame, containing build+construct_resource. Moving EngineState to Box alone does not establish lower peak because Box::new may still first construct the whole value on the native stack and introduces a new heap/API cost. If the common-frame reduction is insufficient, the structural next step is explicit construction/initializer continuations, with existing staging/rollback semantics carried over; do not raise stack/depth/margin limits or drop functionality to conceal the problem.

All actual physical phase outcomes remain owned by the completed capture/verifier review. This diagnosis identifies a feasible exhausting path and the owning code, not the unique instruction that reached the observed high-water mark.
