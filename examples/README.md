# Examples: Curated Runnable Catalog

This directory is the runnable example catalog for truST.

It is also the only maintained source for projects bundled by the VS Code
**Start from example** gallery. `examples/catalog.json` declares those copyable
projects; the extension build generates `editors/vscode/media/examples/` from
this directory. Do not edit the generated package copy.

Bundled projects use the explicit non-secret token `trust-example-token` so
their runtime configurations validate and run locally. Replace it with a
deployment-managed credential before exposing a runtime beyond the example
environment.

The canonical public navigation is the docs-site examples section:

- `docs/public/examples/index.md`
- `docs/public/examples/tutorials.md`
- `docs/public/examples/test-and-debug.md`
- `docs/public/examples/connectivity.md`
- `docs/public/examples/runtime-cloud.md`
- `docs/public/examples/visual-editors.md`
- `docs/public/examples/vendor-profiles.md`
- `docs/public/examples/libraries-and-motion.md`
- `docs/public/examples/capstones.md`

Two former directories were intentionally removed from `examples/`:

- `browser_analysis_wasm_spike/` (internal prototype moved under `docs/internal/prototypes/`)
- `openplc_interop_v1/` (OpenPLC notes absorbed into the PLCopen ST-complete tutorial)

## Run an example

In VS Code with the truST extension: open the **truST** view, choose **Start from example**, pick a
project and open the copy, then press **Start**. The simulator runs it; nothing needs to be built,
because the extension brings `trust-runtime`. The examples that the gallery does not offer run from
a terminal with the released `trust-runtime`:

```bash
trust-runtime build --project examples/<folder>
trust-runtime run --project examples/<folder>
```

Each example's README names what else it needs, as the table below does per category.

## Catalog By Docs Category

| Category | Start Here | What You Learn | What You Need | Typical Time |
|---|---|---|---|---|
| Control engineering | `examples/control/README.md` | feedback, modelling, PID, loop engineering, digital control, and modern control | VS Code with truST (Control Studio); simulated plants, no hardware | 25-45 min per lesson |
| Tutorials | `examples/tutorials/README.md` | language basics, testing, bootstrap, deploy, networking, simulation, safety, CI/CD, observability | `trust-runtime`; a few name an extra tool (`openssl` and `curl`, an editor, Prometheus) | 60-360 min |
| Test and debug | `examples/memory_marker_counter/README.md` | runtime/process-image mental model and debugger confirmation | VS Code with truST, or `trust-runtime ctl` | 20-30 min |
| Operate | `examples/retain_counter/README.md` | what `RETAIN` keeps over a warm restart, a cold restart, a stop and a power loss, and the retain file that keeps it | `trust-runtime` and two terminals | 10 min |
| Rust | `examples/rust_function_block/README.md`, `examples/rust_barcode/README.md`, `examples/rust_signal/README.md`, `examples/rust_service/README.md`, `examples/rust_first_cell/README.md` | a function block written in Rust and called from Structured Text; GS1-128 barcode labels read in Rust; a median filter and a Modbus CRC in Rust; a Rust service beside the runtime and the program's heartbeat gate; a press cell whose whole program is Rust, with its sequence and machine tests | VS Code with truST; Rust from rustup for the service and the machine tests | 10-25 min |
| HMI | `examples/hmi_tank_level/README.md` | screens built in the HMI Builder: live values, setpoint, trend, alarms, Live mode, deploy and roll back | VS Code with truST (HMI Builder) and a browser | 30-45 min |
| HMI | `examples/hmi_equipment/README.md` | the Plant library's pump, control valve, tank and PID loop, each opening its shipped faceplate | VS Code with truST (HMI Builder) and a browser | 20-30 min |
| HMI | `examples/room_heating/README.md` | a room heating built in the HMI Builder: tank and room PID loops written by Create PLC code and set up in Control Studio, start/stop sequence, protections, alarms, settings and three languages | VS Code with truST (HMI Builder) and a browser | 45-60 min |
| Connectivity | `examples/communication/README.md` | Modbus/TCP, MQTT, OPC UA, EtherCAT, GPIO, multi-driver | the peer of each protocol: an MQTT broker, a Modbus/TCP server, an OPC UA client; EtherCAT and GPIO hardware for their device runs (EtherCAT's mock profile runs without) | 120-220 min |
| Runtime cloud | `examples/runtime_cloud/README.md` | profiles, preflight/dispatch, federation allowlists | two to five `trust-runtime` processes on one computer | 30-60 min |
| Visual editors | `examples/ladder/README.md`, `examples/statecharts/README.md`, `examples/blockly/README.md` | visual authoring surfaces tied to companion ST | VS Code with truST | 15-45 min |
| Vendor profiles | `examples/siemens_scl_v1/README.md`, `examples/mitsubishi_gxworks3_v1/README.md`, `examples/plcopen_xml_st_complete/README.md`, `examples/vendor_library_stubs/README.md` | vendor-oriented authoring and migration | `trust-runtime`; the vendor's tool to import the exports | 20-50 min |
| Libraries and motion | `examples/plcopen_motion_single_axis_demo/README.md`, `examples/plcopen_motion_single_axis_benchmarks/README.md`, `examples/oscat_smoke/README.md`, `examples/OSCAT/README.md`, `libraries/plant/README.md` | the shipped libraries (OSCAT, PLCopen Motion, Control, Plant, Alarm, HMI and Comm; the HMI, Control and Alarm examples above use theirs) and performance baselines; each library's manual is under `docs/public/develop/libraries/` | `trust-runtime`, and `trust-dev` for the OSCAT tests | 10-40 min |
| Capstones | `examples/plant_demo/README.md`, `examples/filling_line/README.md`, `examples/hardware_8do/README.md` | larger multi-file or hardware-leaning projects | `trust-runtime` in simulation; `hardware_8do` needs its I/O board for the device run | 25-55 min |
| OpenOT telemetry | `examples/openot_multi_program/README.md` | attribute-driven OpenOT logging, generated producers, one shared-memory ring | `trust-runtime` | 20-30 min |
| OpenOT database persistence | `examples/openot_database/README.md` | one canonical alarms/messages/values/audit workload with TOML overlays for seven real database products | the database of the chosen overlay (SQLite runs locally) | 20-30 min per product |
| Runbooks | `examples/runbooks/site-runbook-template/README.md` | site-specific operator and technician handoff templates | nothing to run: a template to fill in | 10-20 min |

## Archive Policy

- examples that are not strong public defaults belong in `examples/archive/`
- currently, `examples/simulate_process/` is treated as archive-candidate material until it has a curated public entry README and narrower audience framing

## Recommended Learning Order

This is the detailed repo-level progression. The public docs page
`docs/public/examples/learning-paths.md` remains the high-level route map by
goal; keep the two aligned at the category level.

1. `examples/tutorials/README.md`
2. `examples/control/README.md`
3. `examples/tutorials/12_hmi_pid_process_dashboard/README.md`
4. `examples/memory_marker_counter/README.md`
5. `examples/plcopen_motion_single_axis_demo/README.md`
6. `examples/plant_demo/README.md`
7. `examples/filling_line/README.md`
8. `examples/tutorials/13_project_bootstrap_zero_to_first_app/README.md`
9. `examples/tutorials/17_io_backends_and_multi_driver/README.md`
10. `examples/communication/README.md`
11. `examples/tutorials/18_simulation_toml_fault_injection/README.md`
12. `examples/tutorials/19_safety_commissioning/README.md`
13. `examples/tutorials/14_deploy_and_rollback/README.md`
14. `examples/tutorials/16_secure_remote_access/README.md`
15. `examples/tutorials/15_multi_plc_discovery_mesh/README.md`
16. `examples/runtime_cloud/README.md`
17. `examples/tutorials/21_ci_cd_project_pipeline/README.md`
18. `examples/tutorials/22_neovim_zed_workflow/README.md`
19. `examples/tutorials/23_observability_historian_prometheus/README.md`
20. Choose specialization:
   - Interop: `examples/plcopen_xml_st_complete/README.md`
   - Vendor profiles: `examples/siemens_scl_v1/README.md`, `examples/mitsubishi_gxworks3_v1/README.md`
   - Fieldbus backend: `examples/ethercat_ek1100_elx008_v1/README.md`, `examples/ethercat_ek1100_elx008_v2/README.md`

## For contributors: build from source

The commands above use released binaries. To run the examples against a build of this repository:

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

### Validation commands

```bash
trust-runtime build --project examples/filling_line --sources src
trust-runtime build --project examples/plcopen_motion_single_axis_demo --sources src
trust-runtime build --project examples/oscat_smoke --sources src
trust-runtime build --project examples/tutorials/12_hmi_pid_process_dashboard --sources src
trust-runtime build --project examples/plant_demo --sources src
trust-runtime build --project examples/openot_multi_program --sources src
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
trust-runtime build --project examples/vendor_library_stubs --sources src --sources vendor/siemens
```

Tutorial regression checks:

```bash
cargo test -p trust-runtime tutorial_examples_parse_typecheck_and_compile
cargo test -p trust-runtime st_test_cli_command
cargo test -p trust-runtime --test communication_examples_cli
./scripts/runtime_motion_example_bench_gate.sh
./scripts/runtime_motion_benchmark_breakdown.sh
```
