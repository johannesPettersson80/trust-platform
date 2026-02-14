# Beginner Structured Text Tutorials (VS Code Guided Path)

This is the primary onboarding path for learning truST in VS Code.

You will move from simple ST syntax to runtime interaction and unit testing,
while intentionally using one VS Code feature per tutorial.

## One-Time Setup

1. Open repository root in VS Code:

```bash
code /path/to/trust-platform
```

2. Install extension if needed:

```bash
code --install-extension trust-platform.trust-lsp
```

3. Optional confidence check:

```bash
cargo test -p trust-runtime tutorial_examples_parse_typecheck_and_compile_to_bytecode
```

## How To Work Through This Path

For each tutorial:

1. Open the file.
2. Read the "VS Code Feature Spotlight" first.
3. Follow the guided interaction steps.
4. Complete the challenge.
5. Trigger one pitfall intentionally, then fix it.

---

## 01_hello_counter.st

### VS Code Feature Spotlight

- Diagnostics + squiggles
- Hover tooltips
- Semantic syntax highlighting

### Guided Steps

1. Open `01_hello_counter.st`.
2. Hover `count`, `step`, and `enable` to inspect inferred types.
3. Intentionally remove one semicolon and confirm a red diagnostic.
4. Restore the semicolon and confirm diagnostics clear.

### Challenge

- Add `MAX_COUNT : INT := 100;` and wrap `count` to `0` when `count > MAX_COUNT`.

### Common Pitfalls

- Missing semicolon after assignment.
- Using `=` instead of `:=` for assignment.
- Forgetting `END_IF`.

---

## 02_blinker.st

### VS Code Feature Spotlight

- Completion for standard FBs (`TON`)
- Snippet insertion and parameter hints

### Guided Steps

1. Open `02_blinker.st`.
2. Create a temporary line and type `TO` then trigger completion (`Ctrl+Space`).
3. Select `TON` and inspect inserted call shape.
4. Hover timer fields (`IN`, `PT`, `Q`) to inspect semantics.

### Challenge

- Change period from `T#250ms` to `T#500ms` and document expected behavior change.

### Common Pitfalls

- Writing `250ms` instead of typed literal `T#250ms`.
- Forgetting to keep timer call active each cycle.

---

## 03_traffic_light.st

### VS Code Feature Spotlight

- Go to definition on enum types/members
- Document outline (`Ctrl+Shift+O`)

### Guided Steps

1. Open `03_traffic_light.st`.
2. Place cursor on enum state usage and press `F12` (or Ctrl+Click).
3. Open outline (`Ctrl+Shift+O`) and jump between type/program sections.

### Challenge

- Add a maintenance state and transition into it from a new condition.

### Common Pitfalls

- Missing enum member in `CASE` handling.
- Inconsistent typed-literal style for enum values.

---

## 04_tank_level.st

### VS Code Feature Spotlight

- Inlay hints
- Format Document (`Shift+Alt+F`)

### Guided Steps

1. Open `04_tank_level.st`.
2. Run Format Document and inspect spacing/alignment changes.
3. Enable inlay hints if disabled and review call-site parameter names.

### Challenge

- Tighten threshold band to reduce output oscillation.

### Common Pitfalls

- Type mismatch between `INT` sensor and `REAL` threshold math.
- Missing `END_IF` in nested conditions.

---

## 05_motor_starter.st

### VS Code Feature Spotlight

- Find All References (`Shift+F12`)

### Guided Steps

1. Open `05_motor_starter.st`.
2. Run Find All References on `motor_run`.
3. Verify all latching/unlatching writes are visible in one place.

### Challenge

- Add `fault_reset` behavior requiring explicit reset after overload.

### Common Pitfalls

- Accidental combinational loop in latch logic.
- Forgetting stop/overload precedence.

---

## 06_recipe_manager.st

### VS Code Feature Spotlight

- Code folding for `CASE` branches

### Guided Steps

1. Open `06_recipe_manager.st`.
2. Fold all `CASE` branches and expand one-by-one while tracing outputs.
3. Confirm each branch assigns all required outputs.

### Challenge

- Add one new recipe ID with full parameter mapping.

### Common Pitfalls

- Missing default/else behavior.
- Partially assigned outputs in one branch.

---

## 07_pid_loop.st

### VS Code Feature Spotlight

- Rename Symbol (`F2`)

### Guided Steps

1. Open `07_pid_loop.st`.
2. Rename one control variable (for example `control_output` -> `u_cmd`) with `F2`.
3. Review rename preview before applying.

### Challenge

- Add clamping limits to output and anti-windup condition.

### Common Pitfalls

- Renaming to reserved keyword.
- Breaking semantic meaning by unclear variable names.

---

## 08_conveyor_system.st

### VS Code Feature Spotlight

- Signature help during FB calls (`Ctrl+Shift+Space`)

### Guided Steps

1. Open `08_conveyor_system.st`.
2. Place cursor inside a call argument list and trigger signature help.
3. Validate parameter ordering and intent.

### Challenge

- Add a jam-reset input that clears jam latch under safe conditions.

### Common Pitfalls

- Passing wrong parameter order in positional calls.
- Missing safety condition around restart.

---

## 09_simulation_coupling.st

### VS Code Feature Spotlight

- Runtime Panel first-use walkthrough

### Guided Steps

1. Open `09_simulation_coupling.st`.
2. `Ctrl+Shift+P` -> `Structured Text: Open Runtime Panel`.
3. Start runtime (Local mode).
4. In I/O panel, write `%IW0` below threshold (for example `Word(300)`) and observe alarm output.
5. Write `%IW0` above threshold (for example `Word(600)`) and observe alarm change.

### Challenge

- Add second alarm level (warning/critical) with two thresholds.

### Common Pitfalls

- Writing wrong address type (`%Q` instead of `%I`).
- Not starting runtime before writing values.

---

## 10_unit_testing_101/

### VS Code Feature Spotlight

- Test Explorer + CodeLens run actions

### Guided Steps

1. Open `10_unit_testing_101/sources/tests.st`.
2. Use CodeLens `Run Test` above each test.
3. Open Testing sidebar (beaker icon) and run all discovered tests.
4. Inspect pass/fail indicators and test output details.

### Challenge

- Add one failing assertion intentionally, run tests, then fix it (red-green cycle).

### Common Pitfalls

- Writing tests in `PROGRAM` instead of `TEST_PROGRAM`/`TEST_FUNCTION_BLOCK`.
- Forgetting deterministic setup per test case.

---

## 11_unit_testing_102/

### VS Code Feature Spotlight

- Test Explorer triage + focused reruns

### Guided Steps

1. Open `11_unit_testing_102/sources/tests.st`.
2. Run all tests from Testing sidebar.
3. Break one expected value intentionally.
4. Re-run failed test only from sidebar.
5. Restore fix and verify green state.

### Challenge

- Extend mock I/O test matrix with one new operating-band scenario.

### Common Pitfalls

- Testing hardware-mapped `PROGRAM` directly instead of FB logic.
- Carrying test state between scenarios.

---

## 12_hmi_pid_process_dashboard/

### VS Code Feature Spotlight

- HMI descriptor workflow (`hmi/*.toml`)
- Process SVG page rendering (`kind = "process"`)
- Live descriptor refresh for TOML + SVG edits

### Guided Steps

1. Open `12_hmi_pid_process_dashboard/README.md`.
2. Build and run the tutorial project.
3. Open `/hmi` and verify operator pages:
   - `Operator Overview`
   - `P&ID Process`
   - `P&ID Bypass`
   - `Trends`
   - `Alarms`
4. Toggle `%IX0.0` / `%IX0.1` / `%IX0.2` / `%IX0.3` to drive start/stop/spike/bypass scenarios.
5. Verify setpoint, deviation, and alarm widgets update in overview + process pages.
6. Edit `hmi/plant.toml` and swap `plant.svg` <-> `plant-minimal.svg`; save and verify live refresh.
7. Capture one screenshot/GIF using the tutorial media commands.

### Challenge

- Add one extra instrument ID in `hmi/plant-bypass.svg` and bind it from `P1` in `hmi/plant-bypass.toml`.

### Common Pitfalls

- Selector IDs in `[[bind]]` not matching SVG element IDs.
- Using unsupported process selectors (must be `#id` style).
- Renaming PLC symbols without updating HMI bind paths.

---

## Advanced Operations Tutorials (13-22)

After finishing Tutorials 01-12, continue with these production-oriented
walkthroughs. Each guide is intentionally detailed and explains both what to do
and why it matters.

1. `13_project_bootstrap_zero_to_first_app/README.md`
   - start from an empty folder and build a first runnable PLC project.
2. `14_deploy_and_rollback/README.md`
   - practice versioned deployment and controlled rollback.
3. `15_multi_plc_discovery_mesh/README.md`
   - run two runtimes, verify discovery/pairing, and enable mesh sharing.
4. `16_secure_remote_access/README.md`
   - configure TCP control with auth token, pairing, and minimal firewall
     exposure.
5. `17_io_backends_and_multi_driver/README.md`
   - configure `loopback`, `simulated`, `gpio`, `modbus-tcp`, `mqtt`, and
     composed `io.drivers`.
6. `18_simulation_toml_fault_injection/README.md`
   - use deterministic `simulation.toml` couplings/disturbances/fault events.
7. `19_safety_commissioning/README.md`
   - verify safe-state outputs, watchdog/fault policy, and restart recovery.
8. `20_hmi_write_enablement/README.md`
   - move HMI from read-only to constrained write mode with explicit allowlist.
9. `21_ci_cd_project_pipeline/README.md`
   - implement CI gates with machine-readable reports and stable exit codes.
10. `22_neovim_zed_workflow/README.md`
    - run a complete non-VS-Code workflow with Neovim/Zed plus terminal gates.

Suggested sequence for operations engineers:

1. `13_project_bootstrap_zero_to_first_app/README.md`
2. `17_io_backends_and_multi_driver/README.md`
3. `18_simulation_toml_fault_injection/README.md`
4. `19_safety_commissioning/README.md`
5. `14_deploy_and_rollback/README.md`
6. `16_secure_remote_access/README.md`
7. `15_multi_plc_discovery_mesh/README.md`
8. `20_hmi_write_enablement/README.md`
9. `21_ci_cd_project_pipeline/README.md`
10. `22_neovim_zed_workflow/README.md`

---

## Communication Protocol Examples (Grouped)

For protocol-first commissioning, use:

- `../communication/README.md`
  - includes dedicated subfolders for:
    - `modbus_tcp`
    - `mqtt`
    - `opcua`
    - `ethercat`
  - each subfolder includes a runnable minimal project and step-by-step flow.

Transport gating reminders:

- EtherCAT hardware transport is gated by `ethercat-wire` and unix-only for non-`mock` adapters.
- OPC UA wire server is gated by `opcua-wire`.

---

## Validation Coverage

Current regression coverage verifies:

- parse/type-check/bytecode compile of tutorial sources,
- deterministic runtime behavior for selected scenarios,
- no unexpected LSP diagnostics on tutorial files.
