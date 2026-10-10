# B-R3 physical native-stack failure: static diagnosis

Read-only source and retained ELF/map/UART inspection. No compilation, test, link, firmware execution, board command/reset, or source edit. This report does not authorize another hardware attempt.

## Observed evidence

The retained board UART reports depth 1: 9680 bytes used / 6704 free; depth 2: 16344 used / 40 free; depth 3: 16384 used / 0 free, followed by a panic before a depth-4 record. Depth 1 to 2 adds 6664 measured bytes. The paint scanner only examines 0x20014000..0x20018000; depth 3 therefore means at least 16384 bytes, not an exact peak or successful stack admission. The first earlier MEM rows also report saturated boot-wide stack paint; repainting immediately before each depth test is what isolates these depth samples.

The panic counters are 536946432 (0x20012700), 536967904 (0x20017ae0), and 536944568 (0x20011fb8), impossible valid heap accounting for a 72 KiB heap. This is consistent with memory corruption after native-stack underflow, but UART alone does not prove the exact first corrupting instruction or allocator failure. It is not evidence of ordinary out-of-heap rejection.

## Owning frames, directly measured from ARM prologues

Each row includes the prologue's fixed SP subtraction plus pushed saved registers, not callees/interrupts. Retained excerpts are in owning-prologues.txt; full disassembly in disassembly.txt.

| Function | Entry | Local subtraction | Saved registers | Own native bytes |
| --- | --- | ---: | ---: | ---: |
| dispatch::execute_with_buffers | 0x08029878 | 2828 | 36 | 2864 |
| call::execute_native_vm_pou_call | 0x08035144 | 836 | 36 | 872 |
| call::execute_vm_target | 0x08037280 | 180 | 36 | 216 |
| dispatch::execute_pou_stack_with_parameter_presence | 0x0803f7e0 | 452 | 36 | 488 |
| initialization::evaluate_initializer | 0x08027fe0 | 1276 | 36 | 1312 |
| ExecutionContext::initialize_frame | 0x08034064 | 212 | 36 | 248 |
| RuntimeState::execute_cycle | 0x08046e40 | 220 | 36 | 256 |
| engine::execute_cycle_inner | 0x08047c84 | 876 | 36 | 912 |
| firmware::run | 0x080104b8 | 3700 | 36 | 3736 |

The assembly confirms dispatcher -> execute_native_vm_pou_call -> execute_vm_target -> execute_pou_stack_with_parameter_presence -> dispatcher calls. These retain at least 4440 bytes per additional user-function activation (872+216+488+2864), before transient argument binding, initialization, cleanup, interrupts, or the outer runner. This is not claimed to explain every byte of the measured 6664 delta. Default/return initialization can itself re-enter the same dispatcher through evaluate_initializer, adding transient depth and a 1312-byte evaluator frame.

## Source cause

The fixture (tests/fixtures/portability/f401/gpio.st) is not unbounded ST recursion: it chooses Gpio alone, then Gpio -> ProbeLeaf, then Gpio -> ProbeMiddle -> ProbeLeaf, then Gpio -> ProbeOuter -> ProbeMiddle -> ProbeLeaf. The firmware admits max_call_depth=4.

The heap-backed OperandStack/FrameStack and compact activation metadata do not remove Rust call frames. The 0x05 legacy/direct-call opcode already pushes an explicit VM frame and continues one dispatch loop. In contrast, compiler-produced user-function/function-block/method calls use the native-call path: binding/copyback surrounds execute_vm_target, which recursively invokes execute_pou_stack_with_parameter_presence. Its execute_with_buffers reserves a large frame on every entry. The call-depth check runs after entering these Rust functions and bounds logical depth, not their native-stack byte cost. Initializer bodies also recursively re-enter execute_with_buffers from frame construction.

The final ELF also has no independent gpio/main/numeric fixture entry symbols: the compiler inlined fixture phases into firmware::run. Its 3736-byte frame remains live across runtime execution. This is an additional owning cost, not the entire recursive-engine problem.

RAM layout is fixed: heap backing at 0x20000000..0x20012000, allocator state at 0x2001200c..0x2001203c, reserved MSP bottom 0x20014000, top 0x20018000. The 8132-byte gap below the reservation is not approved extra stack. Underflow can cross it into allocator metadata/heap; the linker boundary and paint scan do not dynamically prevent crossing. No exact overflow depth beyond the saturated sample is inferred.

## Principled options for a newly authorized correction scope

1. Preferred structural direction: use the existing single dispatcher and explicit VM frame stack for compiled user calls as well. Store the required continuation data (bound outputs, return slot, receiver/edge transaction and caller restoration) in bounded admitted execution state; return to the one dispatch loop instead of recursively invoking it. Preserve parameter evaluation order, output transaction atomicity, edge restoration, suspended-local lifetimes, all cleanup on errors, one shared budget/deadline and debugger statement/call semantics. Handle initializer execution similarly or bound its separate native nesting explicitly. This is a shared-engine change, not a second MCU engine or a reduced IEC function set.
2. A smaller evidence-led native-frame reduction can separate the large call/initializer/opcode work from the recursive dispatcher and prevent specific harmful inlining, and preserve the existing named firmware fixture phase boundaries so unrelated phase locals do not stay live during execution. These are real owners visible in this ELF. Such changes require measured resulting prologues, target stack proof and hardware replay; there is no promised reduction or claim they make the current depth-4 profile safe. Blanket inline controls and trimming errors/functions are not justified by this analysis.
3. Any heap/MSP repartition needs a separate measured layout decision against preparation/runtime heap peaks, plus interrupt/error-path headroom. Simply adding stack or reducing the admitted depth does not close the current requirement. A physical stack guard/limit mechanism may improve fail-closed behavior, but does not substitute for making the promised depth safe.

Do not replay depth 4 on this image. First prepare the complete chosen correction, review it, and obtain the user's required new run authorization. Preserve the failed ledger, unchanged depth fixture and stack/headroom criteria. Next proof must distinguish static frame evidence, software equivalence, and physical stack measurement; normal paths alone are not a worst-case stack guarantee.
