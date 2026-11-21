# Examples

Runnable example projects grouped by goal. Start with the smallest project that
matches your question, then move to capstones when you want a multi-surface
walkthrough.

## Run an example

In VS Code with the truST extension, open the **truST** view, choose **Start from example**, pick
a project and open the copy, then press **Start**: the simulator runs it, and nothing needs to be
built. The examples also run from a terminal with the released `trust-runtime`:
`trust-runtime build --project <folder>`, then `trust-runtime run --project <folder>`.

## First Examples

1. [Tutorials](tutorials.md): learn the basic edit, test, deploy, simulation,
   and CI/CD loop.
2. [Test And Debug](test-and-debug.md): prove a runtime value or debugger path
   with a small project.
3. [Control Engineering](learning-paths.md#control-engineering-path): progress
   through 84 runnable real-world lessons from feedback fundamentals to modern
   control.

## Pick By Goal

| Category | Start Here | What It Teaches | What It Needs | Est. Time |
| --- | --- | --- | --- | --- |
| Tutorials | [Tutorials](tutorials.md) | language basics, testing, deploy, simulation, CI/CD | `trust-runtime`; a few name an extra tool | 60-360 min |
| Learning Paths | [Learning Paths](learning-paths.md) | ordered paths for first project, connectivity, migration, CI, and visual-editor work | what each path's examples need | 20-90 min |
| Control Engineering | [Control Engineering Path](learning-paths.md#control-engineering-path) | feedback, modelling, industrial PID, digital control, and modern methods | VS Code with truST; simulated plants | 25-45 min per lesson |
| Test and Debug | [Test And Debug](test-and-debug.md) | runtime process image, debugger confirmation, verification loops | VS Code with truST, or `trust-runtime ctl` | 20-30 min |
| Rust | [Rust Examples](rust.md) | a function block written in Rust, called from Structured Text; a Rust service beside the runtime, which the program uses only while its heartbeat is fresh | VS Code with truST; Rust of your own for the service | 10-25 min |
| Connectivity | [Connectivity](connectivity.md) | Modbus/TCP, MQTT, OPC UA, EtherCAT, GPIO, multi-driver | each protocol's peer (MQTT broker, Modbus/TCP server, OPC UA client); hardware for EtherCAT and GPIO device runs | 120-220 min |
| Runtime Cloud | [Runtime Cloud](runtime-cloud.md) | profiles, preflight, dispatch, federation allowlists | two to five runtimes on one computer | 30-60 min |
| Visual Editors | [Visual Editors](visual-editors.md) | ladder, statechart, and Blockly flows tied to companion ST | VS Code with truST | 15-45 min |
| Vendor Profiles | [Vendor Profiles](vendor-profiles.md) | migration-oriented examples and vendor library stubs | `trust-runtime`; the vendor's tool to import exports | 20-50 min |
| Libraries and Motion | [Libraries And Motion](libraries-and-motion.md) | shipped libraries, motion examples, performance baselines | `trust-runtime`; `trust-dev` for the OSCAT tests | 10-40 min |
| Capstones | [Capstones](capstones.md) | larger multi-file or hardware-leaning projects | `trust-runtime` in simulation; I/O hardware for `hardware_8do`'s device run | 25-55 min |
| OpenOT Telemetry | [OpenOT Attribute Authoring](../develop/openot-authoring.md) | variable attributes, generated OpenOT records, multi-PROGRAM producer draining | `trust-runtime` | 20-30 min |
| Runbooks | [Runbooks](runbooks.md) | site-specific operator and technician templates | nothing to run: a template | 10-20 min |
