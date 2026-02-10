# trueST UX Glossary

Purpose: keep user-facing terminology consistent across CLI, UI, and docs.

## User-facing terms

- **PLC**: a running control instance. *(Seen in the sidebar header and status pill.)*
- **PLC name**: the human-friendly name for a PLC (maps to internal resource name). *(Seen in Overview + Setup wizard.)*
- **Project folder**: the folder that contains runtime.toml, io.toml, sources/, and program.stbc. *(Seen in setup and CLI prompts.)*
- **System-wide I/O config**: the device-level I/O settings used by multiple PLC projects. *(Seen in Setup wizard advanced options.)*
- **Cycle time**: how often the PLC task runs (example: 100 ms). *(Seen in Setup wizard + Overview metrics.)*
- **RETAIN**: values preserved across restarts (if enabled). *(Seen in restart and fault behavior docs.)*
- **Driver**: the I/O backend (example: gpio, loopback, simulated, modbus-tcp, mqtt). *(Seen in Setup wizard and I/O page.)*
- **Safe state**: the output values applied when a fault occurs. *(Seen in safety/fault settings and docs.)*
- **Fault**: a runtime error that stops or degrades the PLC. *(Seen in Logs and Overview health.)*
- **Cold restart**: full restart (retained values reset). *(Seen in Controls and Deploy options.)*
- **Warm restart**: restart without clearing retained values. *(Seen in Controls and Deploy options.)*
- **Control token**: a secret used to authorize remote control access. *(Seen in pairing and networking setup.)*
- **Discovery**: local network device discovery (mDNS/Bonjour). *(Seen in Network page.)*
- **Mesh**: runtime-to-runtime data sharing over the network. *(Seen in networking/mesh docs.)*

## Internal terms (avoid in UI unless needed)

- **Runtime**: internal engine powering a PLC.
- **Resource**: internal scheduler/resource name.
- **Bundle**: internal packaging term for a project folder.

**Migration note:** older docs/flags used the word “bundle.” It means **project folder**.

## Mapping

- PLC name -> resource name
- Project folder -> bundle directory
- PLC -> runtime instance
