# Architecture Improvements Checklist

Status: In Progress

- [x] `ARCH-PLCOPEN-01` Add a dedicated PLCopen motion architecture diagram describing the shipped ST public profiles, shared kernels, internal carriers, and deferred-feature guard coverage.
- [x] `ARCH-PLCOPEN-02` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after adding the PLCopen motion architecture diagram.
- [x] `ARCH-VM-01` Add a dedicated bytecode VM execution architecture diagram that separates the production VM path from residual `EvalContext` / legacy-interpreter-only flows.
- [x] `ARCH-VM-02` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after adding the detailed bytecode VM execution diagram.
- [x] `ARCH-VM-03` Add a focused interpreter-removal map diagram that classifies VM runtime pieces as keep, extract, rewrite, or delete.
- [x] `ARCH-VM-04` Regenerate the PlantUML outputs and refresh `docs/diagrams/manifest.json` after adding the interpreter-removal map diagram.
- [x] `ARCH-VM-05` Refresh the interpreter-removal map after landing `program_model` / `helper_eval` extraction so the diagram matches the current ownership split.
- [x] `ARCH-VM-06` Refresh the runtime execution/debug/system diagrams after removing the production interpreter backend so they show direct VM dispatch plus `helper_eval` helper flows.
