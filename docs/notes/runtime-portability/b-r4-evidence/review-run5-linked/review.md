# B-R4 run5 measured call boundary review

Independent artifact/source review by `/root/br1_static_registry`. No build, test, formatter, linker, ELF-inspector rerun or board operation. The prepared source matches the accepted start_call attribute/comment exactly and frozen manifest 2fb19a24c1d344bf3e991b984a422ba7558314f4eb95ea290a0a77ca263da928. ELF 820b2571053da7dd28caba3d5fafbebc40c82f59211f348926ed71ef895c3871 matches the coordinator identity. Existing-ELF disassembly was read with llvm-objdump21.

Measured own native frames: dispatcher1872 bytes, down464 from run4's2336; separate start_call888. Main fixture2488, instantiate1824, evaluator968, construct536, node328, array304, instance initialization256 and declaration action424 remain unchanged. All numbers include saved registers and exclude callees/interrupts.

The source-supported Main.counter→Counter.history ARRAY→INT path retains two dispatcher invocations. Its fixed-frame sum therefore falls from15328 to14400. This is still64 bytes above the14336 maximum usage implied by the unchanged2KiB headroom requirement, before any leaf helpers or interrupt cost. The new start_call is not on this instantiation path, but its extraction reduces the dispatcher's common frame. The measurement is an improvement, not stack-readiness or physical acceptance. Root was informed before marking the candidate measured for later validation.

Upper free space is18624 bytes, up1984 from run4 and2240 above the16KiB flash margin. Link/inspection pass remains distinct from native/hardware evidence. Recommendation: retain this measured gain but do not treat it as sufficient proof for hardware replay; address the remaining owning path before a physical run. No memory/depth/margin/functionality changes are proposed or made.
