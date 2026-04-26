# Architecture Improvements Checklist

Status: Done

- [x] `ARCH-PLCOPEN-01` Add a dedicated PLCopen motion architecture diagram describing the shipped ST public profiles, shared kernels, internal carriers, and deferred-feature guard coverage.
- [x] `ARCH-PLCOPEN-02` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after adding the PLCopen motion architecture diagram.
- [x] `ARCH-PLCOPEN-03` Extend the PLCopen motion architecture diagram for the shipped OOP facade package and document that it delegates to the classic motion kernels instead of owning duplicate motion state.
- [x] `ARCH-VM-01` Add a dedicated bytecode VM execution architecture diagram that separates the production VM path from residual `EvalContext` / legacy-interpreter-only flows.
- [x] `ARCH-VM-02` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after adding the detailed bytecode VM execution diagram.
- [x] `ARCH-VM-03` Add a focused interpreter-removal map diagram that classifies VM runtime pieces as keep, extract, rewrite, or delete.
- [x] `ARCH-VM-04` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after adding the interpreter-removal map diagram.
- [x] `ARCH-VM-05` Refresh the interpreter-removal map after landing `program_model` / `helper_eval` extraction so the diagram matches the current ownership split.
- [x] `ARCH-VM-06` Refresh the runtime execution/debug/system diagrams after removing the production interpreter backend so they show direct VM dispatch plus `helper_eval` helper flows.
- [x] `ARCH-HIR-01` Refresh the HIR semantics architecture diagram so the Salsa query flow shows `file_type_prelude_query`, `project_type_catalog_query`, and project-aware `file_symbols_query` as the source of cross-file type visibility.
- [x] `ARCH-HIR-02` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after the project type catalog refactor.
- [x] `ARCH-VM-07` Refresh the runtime execution diagrams so the shipped `SIZEOF` contract shows compile-time/static-type lowering and no longer presents `SIZEOF_VALUE` as part of normal ST codegen.
- [x] `ARCH-VM-08` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after the `SIZEOF` contract update.
- [x] `ARCH-VM-09` Split the `runtime/vm/register_ir` implementation into focused lowering/profile/tier1/test modules and consolidate composite ref-path walking into `value/reference.rs`; audit the existing VM diagrams and confirm no PlantUML refresh is required because execution/data flow and subsystem ownership stay unchanged.
- [x] `ARCH-RTCORE-01` Add a dedicated runtime-core/native-host split checklist that freezes the core/host ownership decisions, behavior-lock gates, sync capability traits, Linux/PREEMPT_RT host expectations, and the STM32H7/Opta rollout plan.
- [x] `ARCH-RTLINUX-01` Add a dedicated PREEMPT_RT Linux checklist and mark the broader native-host / embedded split plan deferred so the active runtime portability direction is explicit.
- [x] `ARCH-RTLINUX-02` Refresh the runtime/system architecture diagrams so Linux `PREEMPT_RT` posture is shown as a launcher-owned scheduler-thread step with runtime.realtime config feeding verification/memlock/affinity behavior.
- [x] `ARCH-RTVALUE-01` Refresh the runtime execution architecture diagram so enum value identity is owned by validated construction, alias resolution, retained-state canonicalization, and explicit equality semantics.
- [x] `ARCH-RTVALUE-02` Refresh the runtime execution architecture diagram so struct and array values own declared identity, field/type validation, shape validation, and retained-state failure diagnostics without a global value factory.
- [x] `ARCH-RTVALUE-03` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after the runtime value contract updates.
