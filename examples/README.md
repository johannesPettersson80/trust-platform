# Examples: Curated Runnable Catalog

This directory is the runnable example catalog for truST.

The canonical public navigation is the docs-site examples section:

- `docs/public/examples/index.md`
- `docs/public/examples/tutorials.md`
- `docs/public/examples/test-and-debug.md`
- `docs/public/examples/hmi.md`
- `docs/public/examples/connectivity.md`
- `docs/public/examples/runtime-cloud.md`
- `docs/public/examples/visual-editors.md`
- `docs/public/examples/vendor-profiles.md`
- `docs/public/examples/libraries-and-motion.md`
- `docs/public/examples/capstones.md`

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

## Catalog By Docs Category

| Category | Start Here | What You Learn | Typical Time |
|---|---|---|---|
| Tutorials | `examples/tutorials/README.md` | language basics, testing, bootstrap, deploy, networking, simulation, safety, HMI, CI/CD, observability | 60-360 min |
| Test and debug | `examples/memory_marker_counter/README.md` | runtime/process-image mental model and debugger confirmation | 20-30 min |
| HMI | `examples/tutorials/12_hmi_pid_process_dashboard/README.md` | HMI pages, bindings, live refresh | 35-55 min |
| Connectivity | `examples/communication/README.md` | Modbus/TCP, MQTT, OPC UA, EtherCAT, GPIO, multi-driver | 120-220 min |
| Runtime cloud | `examples/runtime_cloud/README.md` | profiles, preflight/dispatch, federation allowlists | 30-60 min |
| Visual editors | `examples/ladder/README.md`, `examples/statecharts/README.md`, `examples/blockly/README.md` | visual authoring surfaces tied to companion ST | 15-45 min |
| Vendor profiles | `examples/siemens_scl_v1/README.md`, `examples/mitsubishi_gxworks3_v1/README.md`, `examples/plcopen_xml_st_complete/README.md`, `examples/vendor_library_stubs/README.md` | vendor-oriented authoring and migration | 20-50 min |
| Libraries and motion | `examples/plcopen_motion_single_axis_demo/README.md`, `examples/plcopen_motion_single_axis_benchmarks/README.md`, `examples/oscat_smoke/README.md` | shipped libraries and performance baselines | 10-40 min |
| Capstones | `examples/plant_demo/README.md`, `examples/filling_line/README.md`, `examples/hardware_8do/README.md` | larger multi-file or hardware-leaning projects | 25-55 min |
| Runbooks | `examples/runbooks/site-runbook-template/README.md` | site-specific operator and technician handoff templates | 10-20 min |

## Archive Policy

- examples that are not strong public defaults belong in `examples/archive/`
- currently, `examples/simulate_process/` is treated as archive-candidate material until it has a curated public entry README and narrower audience framing
- see `docs/internal/testing/checklists/example-catalog-audit.md` for the current keep / tweak / merge / archive decisions

## Recommended Learning Order

This is the detailed repo-level progression. The public docs page
`docs/public/examples/learning-paths.md` remains the high-level route map by
goal; keep the two aligned at the category level.

1. `examples/tutorials/README.md`
2. `examples/tutorials/12_hmi_pid_process_dashboard/README.md`
3. `examples/memory_marker_counter/README.md`
4. `examples/plcopen_motion_single_axis_demo/README.md`
5. `examples/plant_demo/README.md`
6. `examples/filling_line/README.md`
7. `examples/tutorials/13_project_bootstrap_zero_to_first_app/README.md`
8. `examples/tutorials/17_io_backends_and_multi_driver/README.md`
9. `examples/communication/README.md`
10. `examples/tutorials/18_simulation_toml_fault_injection/README.md`
11. `examples/tutorials/19_safety_commissioning/README.md`
12. `examples/tutorials/14_deploy_and_rollback/README.md`
13. `examples/tutorials/16_secure_remote_access/README.md`
14. `examples/tutorials/15_multi_plc_discovery_mesh/README.md`
15. `examples/runtime_cloud/README.md`
16. `examples/tutorials/20_hmi_write_enablement/README.md`
17. `examples/tutorials/21_ci_cd_project_pipeline/README.md`
18. `examples/tutorials/22_neovim_zed_workflow/README.md`
19. `examples/tutorials/23_observability_historian_prometheus/README.md`
20. Choose specialization:
   - Interop: `examples/plcopen_xml_st_complete/README.md`
   - Vendor profiles: `examples/siemens_scl_v1/README.md`, `examples/mitsubishi_gxworks3_v1/README.md`
   - Fieldbus backend: `examples/ethercat_ek1100_elx008_v1/README.md`, `examples/ethercat_ek1100_elx008_v2/README.md`

## Validation Commands

```bash
trust-runtime build --project examples/filling_line --sources src
trust-runtime build --project examples/plcopen_motion_single_axis_demo --sources src
trust-runtime build --project examples/oscat_smoke --sources src
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
./scripts/runtime_motion_example_bench_gate.sh
./scripts/runtime_motion_benchmark_breakdown.sh
```
