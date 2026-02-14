# Examples: Guided VS Code Tutorial Tracks

This folder is curated as onboarding-quality tutorials.

Two former directories were intentionally removed from `examples/`:

- `browser_analysis_wasm_spike/` (internal prototype moved under `docs/internal/prototypes/`)
- `openplc_interop_v1/` (OpenPLC notes absorbed into the PLCopen ST-complete tutorial)

## One-Time Setup

1. Build core binaries:

```bash
cargo build -p trust-runtime -p trust-lsp -p trust-debug
```

2. Install extension:

```bash
code --install-extension trust-platform.trust-lsp
```

3. Open repository:

```bash
code /path/to/trust-platform
```

## Tutorial Catalog (Kept + Improved)

| Track | Start Here | What You Learn | Typical Time |
|---|---|---|---|
| ST fundamentals | `examples/tutorials/README.md` | language basics, VS Code tooling, testing workflow | 60-120 min |
| Advanced operations tutorials (13-23) | `examples/tutorials/README.md` | project bootstrap, deploy/rollback, multi-PLC mesh, secure remote access, I/O backend matrix, simulation, safety commissioning, HMI write enablement, CI/CD, Neovim/Zed workflow, observability commissioning | 210-360 min |
| Communication protocols (grouped) | `examples/communication/README.md` | protocol-focused examples for Modbus/TCP, MQTT, OPC UA, two EtherCAT commissioning profiles (mock-first + field-tested), GPIO, and composed multi-driver configurations with transport gates and commissioning flow | 120-220 min |
| Runtime I/O mental model | `examples/memory_marker_counter/README.md` | `%M/%Q` cycle semantics + debugger confirmation | 20-30 min |
| HMI P&ID tutorial | `examples/tutorials/12_hmi_pid_process_dashboard/README.md` | process SVG pages, bypass mode, setpoint/alarm bindings, and live HMI refresh workflow | 35-55 min |
| Multi-file architecture | `examples/plant_demo/README.md` | type/FB/program/config layering + cross-file refactors | 25-40 min |
| Process-control capstone | `examples/filling_line/README.md` | hysteresis control, interface hierarchy, hot reload | 35-55 min |
| EtherCAT bring-up (DI+DO) | `examples/ethercat_ek1100_elx008_v1/README.md` | `io.toml`, mock-first validation, hardware handoff | 30-50 min |
| EtherCAT bring-up (DO snake) | `examples/ethercat_ek1100_elx008_v2/README.md` | EK1100+EL2008 output sweep, hardware run script, safety mapping | 20-35 min |
| PLCopen XML interop | `examples/plcopen_xml_st_complete/README.md` | VS Code import, post-import exploration, round-trip checks, OpenPLC detection note | 30-50 min |
| Siemens profile | `examples/siemens_scl_v1/README.md` | `#`-prefix behavior, profile comparison, runtime/debug run | 20-30 min |
| Mitsubishi profile | `examples/mitsubishi_gxworks3_v1/README.md` | `DIFU/DIFD` mapping, profile comparison, runtime/debug run | 20-30 min |
| Vendor library stubs | `examples/vendor_library_stubs/README.md` | user-extensible vendor symbol stubs via `[[libraries]]` | 15-25 min |

## Recommended Learning Order

1. `examples/tutorials/README.md`
2. `examples/tutorials/12_hmi_pid_process_dashboard/README.md`
3. `examples/memory_marker_counter/README.md`
4. `examples/plant_demo/README.md`
5. `examples/filling_line/README.md`
6. `examples/tutorials/13_project_bootstrap_zero_to_first_app/README.md`
7. `examples/tutorials/17_io_backends_and_multi_driver/README.md`
8. `examples/communication/README.md`
9. `examples/tutorials/18_simulation_toml_fault_injection/README.md`
10. `examples/tutorials/19_safety_commissioning/README.md`
11. `examples/tutorials/14_deploy_and_rollback/README.md`
12. `examples/tutorials/16_secure_remote_access/README.md`
13. `examples/tutorials/15_multi_plc_discovery_mesh/README.md`
14. `examples/tutorials/20_hmi_write_enablement/README.md`
15. `examples/tutorials/21_ci_cd_project_pipeline/README.md`
16. `examples/tutorials/22_neovim_zed_workflow/README.md`
17. `examples/tutorials/23_observability_historian_prometheus/README.md`
18. Choose specialization:
   - Interop: `examples/plcopen_xml_st_complete/README.md`
   - Vendor profiles: `examples/siemens_scl_v1/README.md`, `examples/mitsubishi_gxworks3_v1/README.md`
   - Fieldbus backend: `examples/ethercat_ek1100_elx008_v1/README.md`, `examples/ethercat_ek1100_elx008_v2/README.md`

## Validation Commands

```bash
trust-runtime build --project examples/filling_line --sources src
trust-runtime build --project examples/tutorials/12_hmi_pid_process_dashboard --sources src
trust-runtime build --project examples/plant_demo --sources src
trust-runtime build --project examples/ethercat_ek1100_elx008_v1 --sources src
trust-runtime build --project examples/ethercat_ek1100_elx008_v2 --sources src
trust-runtime build --project examples/communication/modbus_tcp --sources src
trust-runtime build --project examples/communication/mqtt --sources src
trust-runtime build --project examples/communication/opcua --sources src
trust-runtime build --project examples/communication/ethercat --sources src
trust-runtime build --project examples/communication/ethercat_field_validated_es --sources src
trust-runtime build --project examples/communication/gpio --sources src
trust-runtime build --project examples/communication/multi_driver --sources src
trust-runtime build --project examples/siemens_scl_v1 --sources src
trust-runtime build --project examples/mitsubishi_gxworks3_v1 --sources src
trust-runtime build --project examples/vendor_library_stubs --sources .
```

Tutorial regression checks:

```bash
cargo test -p trust-runtime tutorial_examples_parse_typecheck_and_compile_to_bytecode
cargo test -p trust-runtime st_test_cli_command
cargo test -p trust-runtime --test communication_examples_cli
```
