# Beginner Structured Text Tutorials

This folder provides a step-by-step progression of small, runnable ST programs.

## Learning path

1. `01_hello_counter.st`
   - Focus: scan-cycle state updates and simple conditional logic.
   - Expected behavior: `count` increments by `step` each cycle while `enable` is TRUE and resets to `0` when `reset` is TRUE.
2. `02_blinker.st`
   - Focus: TON usage for periodic output changes.
   - Expected behavior: `lamp` toggles every `T#250ms` while `enable` is TRUE.
3. `03_traffic_light.st`
   - Focus: enum-driven state machine with timed transitions.
   - Expected behavior: outputs cycle through `Red -> RedYellow -> Green -> Yellow -> Red`.
4. `04_tank_level.st`
   - Focus: threshold control with hysteresis-like behavior.
   - Expected behavior: inlet opens when level is low, outlet opens when level is high.
5. `05_motor_starter.st`
   - Focus: latch/unlatch starter logic.
   - Expected behavior: `start_pb` seals in `motor_run`; `stop_pb` or `overload_trip` drops it out.
6. `06_recipe_manager.st`
   - Focus: recipe selection using `CASE`.
   - Expected behavior: applying a recipe maps recipe id to target temperature, mix time, and batch size.
7. `07_pid_loop.st`
   - Focus: simple PID loop structure and terms.
   - Expected behavior: computes error, integral, derivative, and updates `control_output`.
8. `08_conveyor_system.st`
   - Focus: start/stop/jam handling and part counting.
   - Expected behavior: conveyor runs on `start_cmd`, stops on `stop_cmd` or jam, and increments `part_counter` on exit detection.
9. `09_simulation_coupling.st`
   - Focus: simulation-first signal coupling (process output based on synthetic input).
   - Expected behavior: `high_level_alarm` is TRUE when `%IW0` is `>= 500`, FALSE otherwise.
10. `10_unit_testing_101/`
   - Focus: ST unit testing with `TEST_PROGRAM` / `TEST_FUNCTION_BLOCK`.
   - Expected behavior: `trust-runtime test` discovers and executes assertion-based tests, supports `--list`, `--filter`, `--timeout`, and CI output formats.
   - Walk-through: open `10_unit_testing_101/README.md` for a step-by-step guide on writing tests in truST.
11. `11_unit_testing_102/`
   - Focus: mock I/O design pattern and failure triage workflow.
   - Expected behavior: tests call logic FB directly with simulated inputs, while runtime I/O mapping remains in production `PROGRAM`.
   - Walk-through: open `11_unit_testing_102/README.md` for red-green-refactor and debugging flow.

## Validation

MP-011 tests verify:
- each tutorial parses, type-checks, and compiles to bytecode,
- timed/runtime behavior for blinker, traffic light, and motor starter,
- no unexpected LSP diagnostics on tutorial files.
