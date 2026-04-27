# Structured Text Naming Standard Audit

Date: 2026-04-26

Status: Complete for the OSCAT OOP release.

Scope:

- `examples/OSCAT`
- `examples/tutorials/**/*.st`
- low-risk authored examples touched by the public tutorial/catalog path:
  `examples/plant_demo`, `examples/filling_line`,
  `examples/communication/ethercat_field_validated_es`, and
  `examples/ethercat_ek1100_elx008_v2`

## Standard

Default rule:

- new truST-authored public ST names use readable PascalCase
- inherited PLCopen, OSCAT, IEC, vendor-profile, direct-I/O, and generated
  visual-editor names keep their source spelling
- internal package-scope globals may use lower snake case with `g_` when they
  are implementation state, not user API

User-facing standard:

- `docs/public/develop/st-naming-standard.md`

## Verification

Commands used:

```bash
find examples/OSCAT -mindepth 1 -maxdepth 1 -type d | sort | wc -l
find examples/tutorials -type f \( -name '*.st' -o -name '*.ST' \) | sort | wc -l
rg -n "\b[a-z][A-Za-z0-9]*_[A-Za-z0-9_]*\b" examples/OSCAT examples/tutorials -g '*.st' -S
rg -n "\b[a-z][A-Za-z0-9]*_[A-Za-z0-9_]*\b" examples/plant_demo examples/filling_line -g '*.st' -S
```

Observed results:

- 49 paired OSCAT example folders exist under `examples/OSCAT`; each contains
  `non-oop` and `oop` projects.
- 15 tutorial ST files were checked.
- No truST-owned snake_case ST identifiers remain in the new OSCAT OOP
  examples. The only hits are inherited classic OSCAT `BAR_GRAPH` parameter
  names (`trigger_Low`, `trigger_High`, `Alarm_low`, `Alarm_high`,
  `log_scale`) in the classic machine-diagnostics comparison.
- No snake_case ST identifiers remain in the authored tutorial ST files.
- No snake_case ST identifiers remain in `plant_demo` or `filling_line` ST
  after renaming authored types, function blocks, variables, and constants.
- Post-review hardening removed simple-example interface variables from the
  Components examples. Interface-typed locals remain only in the core library
  fixture and the closed-loop polymorphism example where interface dispatch is
  the explicit pattern under test.
- The OSCAT OOP dependency alias is `OscatOop`.

## Updated Authored Examples

- `examples/tutorials/01_hello_counter.st`
- `examples/tutorials/02_blinker.st`
- `examples/tutorials/03_traffic_light.st`
- `examples/tutorials/04_tank_level.st`
- `examples/tutorials/05_motor_starter.st`
- `examples/tutorials/06_recipe_manager.st`
- `examples/tutorials/07_pid_loop.st`
- `examples/tutorials/08_conveyor_system.st`
- `examples/tutorials/09_simulation_coupling.st`
- `examples/tutorials/10_unit_testing_101/src/main.st`
- `examples/tutorials/10_unit_testing_101/src/tests.st`
- `examples/tutorials/11_unit_testing_102/src/main.st`
- `examples/tutorials/11_unit_testing_102/src/tests.st`
- `examples/plant_demo/src/types.st`
- `examples/plant_demo/src/fb_pump.st`
- `examples/plant_demo/src/program.st`
- `examples/filling_line/src/Types.st`
- `examples/filling_line/src/LevelController.st`
- `examples/filling_line/src/PumpDrive.st`
- `examples/filling_line/src/ValveActuator.st`
- `examples/communication/ethercat_field_validated_es/src/main.st`
- `examples/ethercat_ek1100_elx008_v2/src/Main.st`
- `examples/ethercat_ek1100_elx008_v2/sources/Main.st`

## Documented Exceptions

These files intentionally keep non-PascalCase names:

- `examples/blockly/**/*.st`: generated Blockly companion output.
- `examples/ladder/**/*.st`: generated ladder companion output and rung names.
- `examples/statecharts/**/*.st`: generated statechart companion output and
  source state IDs.
- `examples/plcopen_motion_single_axis_demo/src/Globals.st` and
  `examples/plcopen_motion_single_axis_benchmarks/**/Globals.st`: internal
  PLCopen demo/benchmark global state with `g_` prefix. This follows the
  naming standard's implementation-global exception.
- Direct I/O aliases such as `DI0`, `DO0`, `%IX0.0`, and `%QX0.0`: hardware
  and address-oriented names are preserved for field wiring clarity.
- Imported/vendor compatibility examples keep vendor-owned spellings where the
  example is demonstrating interoperability rather than the new truST style.

## Acceptance

- [x] Public naming standard documented.
- [x] OSCAT OOP examples checked.
- [x] Tutorial ST files checked and updated.
- [x] Low-risk authored examples checked and updated.
- [x] Generated/vendor/compatibility exceptions classified.
- [x] Targeted runtime/example tests rerun after the renames.
- [x] Post-review component/example naming cleanup reran:
  `cargo test -p trust-runtime --test oscat_oop_library` and
  `cargo test -p trust-runtime --test oscat_oop_examples`.
