# U dependency decisions

Behavior authority: specification 34 §16 (MOD-01 through MOD-06). Selected
migrations are implemented and the applicable Linux software/F401 checks passed;
the current qualification and limits are recorded in [the execution closeout](u-execution.md#final-software-and-f401-reconciliation).
Registry metadata was refreshed on 10 October 2026 from the primary registry APIs;
raw responses remain outside Git in the task artifact directory. Stable versions
exclude SemVer prereleases but retain build metadata such as `+spec-1.1.0`.

The initial inventory covers 83 direct Cargo packages, 30 direct npm packages, five Git declarations,
all first-party manifests including firmware/fuzz, transitive lock resolutions,
project-owned tools and CI/documentation/packaging versions. Prototype/vendor code
is reviewed as provenance, not silently converted into first-party packages.
**U-CLOSE** means the applicable dependency composition passed the completed U
Linux/portable/editor/WASM and physical F401 checks recorded in the execution
closeout. It does not qualify
native Windows/macOS, C6 hardware, GitHub Actions execution, every optional tool
or a production release. Retained packages and explicit exceptions stay as listed.

## Cargo decisions

| Package | Current root lock resolution | Selected candidate | Decision | Reason / compatibility work | Owning manifests | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| `ads` | 0.4.4 | 0.4.4 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `anyhow` | 1.0.103 | 1.0.104 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `base64` | 0.13.1, 0.21.7, 0.22.1 | 0.23.1 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-dev/Cargo.toml, crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `boxcar` | 0.2.14 | 0.2.14 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-hir/Cargo.toml` | U-CLOSE |
| `clap` | 4.6.1 | 4.6.7 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `clap_complete` | 4.6.3 | 4.6.11 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `cortex-m` | 0.7.7 | 0.7.9 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-platform-stm32f4/Cargo.toml, firmware/trust-nucleo-f401re/Cargo.toml` | U-CLOSE |
| `cortex-m-rt` | root 0.7.7; firmware pin 0.7.5 | 0.7.7 | upgrade firmware pin | Preserve paint-stack/set-sp/set-vtor startup contract and requalify actual firmware. | `firmware/trust-nucleo-f401re/Cargo.toml` | U-CLOSE |
| `crc32fast` | 1.5.0 | 1.5.2 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `crossbeam-channel` | 0.5.15 | 0.5.17 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `crossterm` | 0.29.0 | 0.29.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `dialoguer` | 0.12.0 | 0.12.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `drop_bomb` | 0.1.5 | 0.1.5 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `embedded-alloc` | firmware 0.7.0 | 0.7.0 | retain | Current release; preserve LLFF allocation, measurement and target contract. | `firmware/trust-nucleo-f401re/Cargo.toml` | U-CLOSE |
| `embedded-hal-nb` | 1.0.0 | 1.0.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-platform-stm32f4/Cargo.toml` | U-CLOSE |
| `ethercrab` | 0.6.0 | 0.7.1 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `expect-test` | 1.5.1 | 1.5.1 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `futures-util` | 0.3.32 | 0.3.34 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `glob` | 0.3.3 | 0.3.4 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `gpio-cdev` | 0.6.0 | 0.6.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `hex` | 0.4.3 (existing transitive) | 0.4.3 | direct reuse | Encode sha2 0.11 digest bytes explicitly; its new output array omits LowerHex. Preserve exact lowercase SHA-256 strings and existing prefixes; no cryptographic/PLC change. | `Cargo.toml`, runtime, verification-cases, xtask | pending native vectors and existing hashes |
| `if-addrs` | 0.15.0 | 0.15.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `indexmap` | 1.9.3, 2.14.0 | 2.14.2 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `indicatif` | 0.18.4 | 0.18.6 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `insta` | 1.47.2 | 1.49.0 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `libfuzzer-sys` | separate fuzz graphs | 0.4.13 | freeze current floor | Retain current stable family and freeze both instrumentation graphs. | `crates/trust-ads-server/fuzz/Cargo.toml, fuzz/Cargo.toml` | U-CLOSE |
| `libloading` | 0.8.9 | 0.9.0 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-tcads-native/Cargo.toml` | U-CLOSE |
| `libm` | 0.2.16 | 0.2.16 | retain | Current stable 0.2.16 preserves the recorded numeric contract; qualify compiler/feature changes with existing exact/tolerance oracles. | `Cargo.toml` | U-CLOSE |
| `logos` | 0.16.1 | 0.16.1 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `lsp-types` | 0.94.1, 0.97.0 | 0.97.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `mdns-sd` | 0.18.2 | 0.21.5 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `mysql` | 28.0.2 | 28.0.3 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `native-tls` | 0.2.18 | 0.2.18 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `nb` | 0.1.3, 1.1.0 | 1.1.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-platform-stm32f4/Cargo.toml` | U-CLOSE |
| `notify` | 8.2.0 | 8.2.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `object` | 0.37.3 | 0.40.0 | upgrade | ELF flags are typed in 0.40: migrate every inspector mask to contains/intersects and account for the extended section-flags shape; preserve all device-image checks. | `xtask/Cargo.toml`, `xtask/src/portability/elf.rs` | first freeze failed on the old API; correction prepared, execution pending |
| `once_cell` | 1.21.4 | 1.21.4 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `opcua` | 0.12.0 | 0.12.0 | retain | Current stable remains 0.12.0. Reassess advisory paths/expiry; a protocol-stack replacement requires separately reviewed interoperability evidence. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `owo-colors` | 4.3.0 | 4.4.0 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `parking_lot` | 0.12.5 | 0.12.5 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `percent-encoding` | 1.0.1, 2.3.2 | 2.3.2 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `postgres` | 0.19.14 | 0.19.14 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `postgres-native-tls` | 0.5.3 | 0.5.3 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `proc-macro2` | 1.0.106 | 1.0.107 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `xtask/Cargo.toml` | U-CLOSE |
| `qrcode` | 0.14.1 | 0.14.1 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `rand` | 0.8.6, 0.9.4, 0.10.1 | 0.10.3 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `rapier3d` | 0.32.0 | 0.36.1 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `ratatui` | 0.30.0 | 0.30.2 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `region` | 3.0.2 | 4.0.1 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `rowan` | 0.16.1 | 0.17.0 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `roxmltree` | 0.21.1 | 0.21.1 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `rumqttc` | 0.25.1 | 0.25.1 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `rusqlite` | 0.40.2 | 0.40.2 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `rustc-hash` | 1.1.0, 2.1.2 | 2.1.3 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `rustix` | 0.38.44, 1.1.4 | 1.1.5 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `rustls` | 0.20.9, 0.23.45 | 0.23.45 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `salsa` | 0.28.5 | 0.28.5 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `serde` | 1.0.228 | 1.0.229 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `serde_json` | 1.0.149 | 1.0.152 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml, fuzz/Cargo.toml` | U-CLOSE |
| `sha2` | 0.10.9, 0.11.0 | 0.11.0 | upgrade direct floor | Upgrade direct 0.10 APIs to the current 0.11 family already present transitively; preserve digest outputs. | `Cargo.toml, crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `signal-hook` | 0.3.18 | 0.4.5 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `smol_str` | 0.2.2 | 0.3.6 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `stm32f4` | 0.16.0 | 0.16.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-platform-stm32f4/Cargo.toml` | U-CLOSE |
| `stm32f4xx-hal` | 0.23.0 | 0.23.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-platform-stm32f4/Cargo.toml` | U-CLOSE |
| `syn` | 1.0.109, 2.0.117, 3.0.6 | 3.0.7 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `xtask/Cargo.toml` | U-CLOSE |
| `text-size` | 1.1.1 | 1.1.1 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `thiserror` | 1.0.69, 2.0.18 | 2.0.21 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `tiberius` | 0.12.3 | 0.13.0 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `time` | 0.3.47 | 0.3.55 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `tiny_http` | 0.12.0 | 0.12.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `tiverse-mmap` | 1.0.0 | 1.0.0 | retain | Keep the reviewed local patched source; upstream version metadata alone does not supersede the patch/provenance contract. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `tokio` | 1.52.1 | 1.53.2 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `tokio-util` | 0.7.18 | 0.7.20 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `toml` | 1.1.2+spec-1.1.0 | 1.1.8+spec-1.1.0 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `toml_edit` | 0.25.11+spec-1.1.0 | 0.25.17+spec-1.1.0 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `tower` | 0.4.13 | 0.4.13 | retain | tower-lsp 0.20 implements the Tower 0.4 Service contract. A Tower 0.5 switch alone breaks that boundary; retain the compatible protocol pair. | `Cargo.toml` | U-CLOSE |
| `tower-lsp` | 0.20.0 | 0.20.0 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `tracing` | 0.1.44 | 0.1.44 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `tracing-subscriber` | 0.3.23 | 0.3.23 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `tungstenite` | 0.28.0 | 0.30.0 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `ureq` | 3.3.0 | 3.4.2 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |
| `urlencoding` | 2.1.3 | 2.1.3 | retain | Already resolves the current stable release; retain features/support contract and qualify the new compiler. | `Cargo.toml` | U-CLOSE |
| `wasm-bindgen` | 0.2.121 | 0.2.129 | upgrade | Adopt current stable compatible release; owning API migration and target evidence are required. | `Cargo.toml` | U-CLOSE |
| `zenoh` | 1.7.2 | 1.10.1 | retain 1.7.2 | Client/router/plugin major-minor baseline is a qualified deployment contract; preserve that family until the router and plugin migration has real protocol evidence. | `crates/trust-runtime/Cargo.toml` | U-CLOSE |

## npm decisions

| Package | Current requirement | Selected candidate | Decision / constraint | Evidence |
| --- | --- | --- | --- | --- |
| `@codemirror/autocomplete` | 6.20.0 | 6.20.3 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@codemirror/commands` | 6.10.2 | 6.11.1 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@codemirror/lint` | 6.9.4 | 6.9.7 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@codemirror/state` | 6.5.4 | 6.7.6 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@codemirror/view` | 6.39.14 | 6.43.14 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@playwright/test` | ^1.53.0 | 1.64.0 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@types/mocha` | ^10.0.10 | 10.0.10 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@types/node` | ^25.3.0 | 22.20.5 | align with Node 22 APIs in minimum supported VS Code 1.109; build Node 24 is a different runtime | U-CLOSE |
| `@types/react` | ^19.2.14 | 19.3.0 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@types/react-dom` | ^19.2.3 | 19.3.0 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@types/vscode` | ^1.109.0 | 1.109.0 | retain exact typings for unchanged minimum VS Code 1.109 support; latest API typings would expose unsupported newer APIs | U-CLOSE |
| `@typescript-eslint/eslint-plugin` | ^8.56.0 | 8.71.1 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@typescript-eslint/parser` | ^8.56.0 | 8.71.1 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@vscode/codicons` | ^0.0.44 | 0.0.45 | upgrade latest stable; registry latest tag is a prerelease and is not selected | U-CLOSE |
| `@vscode/test-electron` | ^2.5.2 | 3.1.0 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `@xyflow/react` | ^12.10.1 | 12.12.0 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `blockly` | ^11.1.1 | 13.3.0 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `clsx` | ^2.1.1 | 2.1.1 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `concurrently` | ^9.2.5 | 10.0.6 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `esbuild` | ^0.28.1 | 0.28.2 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `eslint` | ^10.0.0 | 10.12.0 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `konva` | ^9.3.22 | 10.7.1 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `lucide-react` | ^0.575.0 | 1.55.0 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `mocha` | ^11.7.6 | 12.0.3 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `monaco-editor` | 0.55.1 | 0.57.0 | upgrade; preserve renderer/wiring; scoped DOMPurify 3.4.16 security patch | U run-1 upstream audit failure retained; corrected asset, lock, audit and browser contracts passed (U-CLOSE) |
| `react` | ^19.2.4 | 19.3.0 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `react-dom` | ^19.2.4 | 19.3.0 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `typescript` | 5.9.3, ^5.9.3 | 6.0.3 | upgrade to latest stable 6.0 release accepted by typescript-eslint (<6.1); TypeScript 7 is incompatible with the selected parser | U-CLOSE |
| `vite` | 7.3.1 | 8.3.4 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |
| `vscode-languageclient` | ^9.0.1 | 10.1.2 | upgrade; preserve renderer/wiring and peer compatibility | U-CLOSE |

## Project tools, Git and vendor decisions

VS Code 1.109.0's [upstream .nvmrc](https://github.com/microsoft/vscode/blob/1.109.0/.nvmrc)
selects Node 22.21.1. Node 22 typings therefore describe the supported extension
runtime; Node 24.21.0 is isolated build tooling. Its archive hash is verified against
the official SHASUMS256.txt. Rust 1.99.0 installed successfully on the builder;
installation is not compile/test evidence. The builder's existing global tool and
Node installations remain unchanged while the publication agent runs.

All five OpenOT declarations retain `137f0e765f085c262651f479be35298b836ac891`:
`git ls-remote` confirms it is current upstream HEAD. The patched `tiverse-mmap`
1.0.0 remains a reviewed local dependency, with the existing upstream license and
patch provenance. Vendor/prototype editions are not first-party support declarations.

| Tool / family | Selected candidate | Decision / acceptance |
| --- | --- | --- |
| `wasm-pack` | 0.15.0 | Current stable candidate; install privately where the owning gate requires it, preserve target-specific instrumentation constraints. Version/disposition recorded; owning workflow proof is limited to the U allocation. |
| `wasm-bindgen-cli` | 0.2.129 | Current stable candidate; install privately where the owning gate requires it, preserve target-specific instrumentation constraints. Version/disposition recorded; owning workflow proof is limited to the U allocation. |
| `cargo-deny` | 0.20.2 | Current stable candidate; install privately where the owning gate requires it, preserve target-specific instrumentation constraints. Version/disposition recorded; owning workflow proof is limited to the U allocation. |
| `cargo-audit` | 0.22.2 | Current stable candidate; install privately where the owning gate requires it, preserve target-specific instrumentation constraints. Version/disposition recorded; owning workflow proof is limited to the U allocation. |
| `cargo-nextest` | 0.9.148 | Current stable candidate; install privately where the owning gate requires it, preserve target-specific instrumentation constraints. Version/disposition recorded; owning workflow proof is limited to the U allocation. |
| `cargo-fuzz` | 0.13.2 | Current stable candidate; install privately where the owning gate requires it, preserve target-specific instrumentation constraints. Version/disposition recorded; owning workflow proof is limited to the U allocation. |
| `cargo-mutants` | 27.1.0 | Current stable candidate; install privately where the owning gate requires it, preserve target-specific instrumentation constraints. Version/disposition recorded; owning workflow proof is limited to the U allocation. |
| `cargo-llvm-cov` | 0.9.1 | Current stable candidate; install privately where the owning gate requires it, preserve target-specific instrumentation constraints. Version/disposition recorded; owning workflow proof is limited to the U allocation. |
| `cargo-machete` | 0.9.2 | Current stable candidate; install privately where the owning gate requires it, preserve target-specific instrumentation constraints. Version/disposition recorded; owning workflow proof is limited to the U allocation. |
| `cargo-semver-checks` | 0.51.0 | Current stable candidate; install privately where the owning gate requires it, preserve target-specific instrumentation constraints. Version/disposition recorded; owning workflow proof is limited to the U allocation. |
| `probe-rs-tools` | 0.32.0 | Current stable candidate; install privately where the owning gate requires it, preserve target-specific instrumentation constraints. Version/disposition recorded; owning workflow proof is limited to the U allocation. |
| `ast-grep` | 0.50.0 | Selected stable; owning AST/API/cross/orchestration checks remain required. |
| `cargo-public-api` | 0.52.0 | Selected stable; owning AST/API/cross/orchestration checks remain required. |
| `cross` | 0.2.5 | Selected stable; owning AST/API/cross/orchestration checks remain required. |
| `just` | 1.58.0 | Selected stable; owning AST/API/cross/orchestration checks remain required. |
| `mkdocs` | 1.6.1 | Documentation requirements pinned; pyads remains protocol-test tooling. Python support >=3.8. |
| `mkdocs-material` | 9.7.7 | Documentation requirements pinned; pyads remains protocol-test tooling. Python support >=3.8. |
| `mkdocs-mermaid2-plugin` | 1.2.3 | Documentation requirements pinned; pyads remains protocol-test tooling. Python support >=3.8. |
| `pyads` | 3.6.0 | Documentation requirements pinned; pyads remains protocol-test tooling. Python support <4,>=3.9. |

GitHub-hosted workflow actions follow the repository's major-tag convention;
refreshed exact release candidates below establish the researched baseline.
The qualification record must retain the concrete revisions actually executed.
New Node 24 action runtimes require compatible runner software; native hosted
CI remains separate evidence from local scripts.

| Action | Current stable release | Selected major | Evidence |
| --- | --- | --- | --- |
| `Swatinem/rust-cache` | [v2.9.2](https://github.com/Swatinem/rust-cache/releases/tag/v2.9.2) | v2 | pending native GitHub CI; U local script parity only |
| `actions/checkout` | [v7.0.1](https://github.com/actions/checkout/releases/tag/v7.0.1) | v7 | pending native GitHub CI; U local script parity only |
| `actions/configure-pages` | [v6.0.0](https://github.com/actions/configure-pages/releases/tag/v6.0.0) | v6 | pending native GitHub CI; U local script parity only |
| `actions/deploy-pages` | [v5.0.1](https://github.com/actions/deploy-pages/releases/tag/v5.0.1) | v5 | pending native GitHub CI; U local script parity only |
| `actions/download-artifact` | [v8.0.2](https://github.com/actions/download-artifact/releases/tag/v8.0.2) | v8 | pending native GitHub CI; U local script parity only |
| `actions/setup-node` | [v7.1.0](https://github.com/actions/setup-node/releases/tag/v7.1.0) | v7 | pending native GitHub CI; U local script parity only |
| `actions/setup-python` | [v7.0.0](https://github.com/actions/setup-python/releases/tag/v7.0.0) | v7 | pending native GitHub CI; U local script parity only |
| `actions/upload-artifact` | [v7.0.2](https://github.com/actions/upload-artifact/releases/tag/v7.0.2) | v7 | pending native GitHub CI; U local script parity only |
| `actions/upload-pages-artifact` | [v5.0.0](https://github.com/actions/upload-pages-artifact/releases/tag/v5.0.0) | v5 | pending native GitHub CI; U local script parity only |
| `peter-evans/create-pull-request` | [v8.1.1](https://github.com/peter-evans/create-pull-request/releases/tag/v8.1.1) | v8 | pending native GitHub CI; U local script parity only |
| `softprops/action-gh-release` | [v3.0.3](https://github.com/softprops/action-gh-release/releases/tag/v3.0.3) | v3 | pending native GitHub CI; U local script parity only |

Transitive resolutions are refreshed in each of the four Cargo lockfiles through
Rust-version-aware resolver 3, without new override patches. Parent-required older
families and existing advisory exceptions are reviewed against the resulting
paths during the U supply-chain stages. Lock resolution is not advisory evidence.
The real-database container versions remain interoperability fixtures until their
server-specific migration/contract evidence is reviewed; newer server releases
must not silently change the protocol acceptance baseline.

Both fuzz locks are now retained source inputs rather than ignored generated files;
the separate workspaces must reproduce their selected parent/transitive graph.
Their sanitizer compiler stays instrumentation-only. CI installs cargo-fuzz 0.13.2
using the pinned product compiler, then uses the instrumentation compiler for fuzz
execution. The physical PLC image never uses nightly.

Python 3.12 is retained as the currently supported CI interpreter family; refreshed
MkDocs tools support it. Python itself is host orchestration, not a PLC/runtime API.
Its refreshed patch selection is recorded below. The seven digest-pinned database/proxy images in
`scripts/openot_database_runner_prepare.sh` remain explicit protocol fixture
versions, not silently advertised as latest server releases. Their refreshed candidate observations and retention reasons are recorded below.

EtherCrab 0.7 inserts its `RawRwLock` type parameter before the group state.
The owning group alias now names its exported `DefaultLock` explicitly and keeps
`Op` as the operation state. This preserves the existing default-lock behavior;
hosted compile and existing EtherCAT native regressions passed in U. SmolStr 0.3.6
keeps the documented 23-byte inline capacity and static-string constructor;
existing slot-size, copy/drop and registry tests requalify the implementation.

Two older transitive resolutions are mandatory constraints, not resolver mistakes:
Pest 2.9.3 pins PSM `=0.1.24`, and crypto-common 0.1.7 pins generic-array
`=0.14.7`. The registry candidates are 0.1.32 and 0.14.9 respectively. Retain the
maintained parents' pinned contracts and report their audit paths; do not introduce
a third-party source patch solely to force the larger version number. Both parent
families remain in the root and main fuzz graph; neither is in F401 firmware.

The old npm overrides forced `diff` 8.0.3 and `serialize-javascript` 7.0.5.
Mocha 12.0.3 requires `diff ^9.0.0` and `serialize-javascript ^7.1.1`; both old
overrides are removed so the maintained parent owns compatible resolution. Current
candidates are diff 9.0.0 and serialize-javascript 7.1.2. The npm lock/audit and
full extension suites verified the replacement; no weaker security policy
or forced incompatible dependency is introduced.

Documentation CI retains the supported Python 3.12 family and pins its refreshed
3.12.15 patch. Pip 26.2.1 is pinned. The builder currently supplies Python 3.14.4;
that separate execution identity must be recorded rather than claimed as exact
Python-CI parity. No global interpreter is upgraded by this task.

The project CI Dockerfile's Rust 1.84 image could not meet even the previous
MSRV. It now uses `rust:1.99.0-bookworm@sha256:114c7a4425406451c2866b6aafe69fe29b1b298832db1277d411ac73c82d04d6`, matching
U's exact compiler. The tag's multi-architecture digest was refreshed from Docker
Hub's primary API; the actual container execution is part of U acceptance.
Webview type checking explicitly uses ESNext/Bundler, while the extension host
uses Node16 resolution and CommonJS output. These are separate deployment contexts.

Read-only resolved-graph inspection confirms the existing exception paths still
exist: Retained Zenoh 1.7.2 → token-cell → paste; tiny_http 0.12 → rustls 0.20 → ring
0.16; opcua 0.12 → url 1.7/idna 0.1 and derivative 2.2. The exception descriptions
now name selected versions. Owners, deadlines and removal criteria remain intact;
these observations do not replace the final deny/audit stages or claim patched
security. Cortex-M 0.7.9 still requires bare-metal under the existing expiring
ARM-only exception.

Database/proxy image candidates were refreshed from their primary registry on
11 October. Retain the seven exact digest-pinned interoperability fixtures for
this scope: PostgreSQL 18.6, TimescaleDB 2.29.2-pg18, MySQL 8.4.11 LTS, MariaDB
11.8.8 LTS, SQL Server 2025 CU8, InfluxDB 3.11.2 Core, and nginx 1.29.3 Alpine.
A registry `latest` digest is a different server/image contract, not a compatible
patch selection by itself. The client packages are modernized and tested against
the existing specified server matrix; replacing that matrix belongs to its owning
protocol qualification. This is bounded fixture retention, not a claim that all
servers are latest. Raw registry candidates, absent Timescale `latest` tag and
MCR tag observations are retained with the research snapshot.

Builder tool observation before U validation (11 October): cargo-deny 0.19.9,
cargo-audit 0.22.2, nextest 0.9.140, cargo-fuzz 0.13.2, cargo-mutants 27.0.0,
llvm-cov 0.8.7, machete 0.9.2, public-api 0.51.0, wasm-pack 0.15.0 and just
1.45.0. semver-checks, wasm-bindgen and probe-rs are absent; ast-grep may be supplied
through `sg`. Private U installations adopt the selected existing tool upgrades
and matching wasm-bindgen CLI; absent optional semver/probe tools are research
candidates, not invented mandatory gates. The existing OpenOCD board protocol
stays the hardware transport. The private tool identities and exact commands must
be retained in the U batch, with the global publication environment untouched.

The refreshed MCR list includes SQL Server 2025 CU9 and CU8-GDR1; Timescale's
latest-pg18 is 2.30.2. The existing digest-pinned server versions remain the
interoperability fixtures selected above. These retained versions must not be
reported as latest or automatically updated through the `latest` tags.


OpenOCD is retained as the reviewed physical transport: locally installed
0.12.0+dev-snapshot (2026-02-16-16:07), with its executable identity captured in the
new board record. The upstream stable family is 0.12.0 and upstream's current
manual documents a development snapshot. No new flash/probe mechanism is needed
for the compiler migration. This is explicit retention of a previously exercised
transport, not a claim that the development snapshot is the latest stable release.
[Upstream distribution guidance](https://openocd.org/pages/getting-openocd.html)
distinguishes source, OS packages and community binaries.

Selected existing Cargo tools are installed privately from upstream GitHub
release assets, verified against each primary API SHA-256 digest; their archive
and executable receipts are external batch inputs. Public-api has no published
binary asset and will be installed from its locked 0.52.0 source during final
preparation. No optional semver/probe tools are added. Cargo plugin selection
requires the private CARGO_HOME/bin as well as PATH; exact CLI versions are
recorded. cargo-deny 0.20.2 moves --config before the check subcommand; its firmware
policy command and native failure-aggregation assertion are migrated together.
No advisory exception, check category or failure result is relaxed.


Vite 8's maintained Rolldown option names replace the deprecated Rollup aliases
in both browser library configs: `rolldownOptions` with `codeSplitting: false`.
The ESM format, es2020 target, filenames, CSS policy and single-bundle deployment
remain unchanged. This is bundler wiring, not TypeScript product logic.
[Migration contract](https://vite.dev/guide/migration) and
[single-bundle option](https://rolldown.rs/reference/OutputOptions.inlineDynamicImports).
The actual generated assets, artifact assertions and live browser evidence are
required before claiming compatibility.


Final source audit and independent review corrected the provisional Zenoh upgrade.
The runtime version policy, startup advertisement, parser/schema/template defaults
and baseline gate own a 1.7 client/router/plugin family, and the gate checks every
locked Zenoh package. Retain exact 1.7.2 across root/main-fuzz graphs in U. A later
1.10 family migration must qualify the router/plugins and migrate configuration,
not mislabel a new client graph as the old family or replace fixture strings solely
to pass tests. The current protocol and all functions remain intact; the 1.10.1
candidate is researched but unqualified, and its provisional graph observation is
superseded. This retention follows MOD-04's interoperability contract rule.


The extension packager is now an explicit locked dev dependency, @vscode/vsce
4.0.0 (Node >=22), replacing the capture script's 2.27.0 pin and the release
workflow's floating global install. Packaging callers use the installed locked
binary; Marketplace installation pins the same version. This adds one npm tool
to the initial 30-package snapshot. Its maintained CLI still supports target,
skip-license and packagePath. Actual VSIX packaging and installed code-server
wiring remain acceptance work; no Marketplace write is authorized in U.

Canonical diagram renderer retains its exact PlantUML 1.2026.6 image/JAR
SHA-256. The researched latest candidate is 1.2026.8. Retention preserves the
shared rendering environment and the existing generated-diagram contract;
updating that environment requires reviewing/regenerating the complete SVG
baseline. Code-server retains 4.116.0 as the existing capture interoperability
fixture; the researched candidate is 4.141.0. Minimum/current native VS Code are
qualified separately, and the retained code-server installation is actually
replayed in U. Neither retention is described as latest or security qualification.
Primary release metadata is retained externally with the dependency snapshot.

The digest-output API correction adds one explicit Cargo dependency to the original
83-package inventory, reusing the already resolved `hex` 0.4.3 package. Its
[maintained encode API](https://docs.rs/hex/latest/hex/fn.encode.html) produces
lowercase hexadecimal from digest bytes. No hash algorithm, input canonicalization,
prefix, fixture digest or assertion is changed. Root and main-fuzz lock ownership
is refreshed through Cargo; the MCU core/firmware graph gains no hash dependency.

Cargo reported that SemVer build metadata in version requirements is ignored.
The `toml` and `toml_edit` floors therefore use their base versions (`1.1.8` and
`0.25.17`); actual `+spec-1.1.0` package identities remain pinned in Cargo.lock.
This removes misleading requirements and manifest warnings without changing
resolved packages, parser semantics or provenance source inputs.

The retained first-freeze compiler failure exposed the object 0.40 ELF flag API
migration missed during initial source review. The prepared correction uses the
upstream typed flags directly, preserving the prior all-required/any-forbidden bit
semantics. The ARM hard-float constant remains an integer upstream and is wrapped
in FileFlags at the boundary. No firmware admission requirement is relaxed.
Primary API: [object 0.40 FileFlags](https://docs.rs/object/0.40.0/object/elf/struct.FileFlags.html),
plus the resolved crate's `src/common.rs`, `src/elf.rs` and `src/constants.rs`.
The corrected inspector compiled during the approved correction freeze and
passed inspection of run-1’s actual F401 ELF. Its native unit assertions remain
unrun because the full workspace build stopped at HIR; no physical replay ran.

## Run-1 owning corrections

Rowan 0.17 changes SyntaxNode::green to a borrowed GreenNodeData whose ToOwned
implementation produces GreenNode. The HIR parse cache now uses that owning API;
no unrelated Salsa trait import is added. Rust 2024 requires an explicit outer
reference in the registered-name find predicate. Existing anonymous constructed
type-name and source-edit/cache invalidation assertions remain unchanged and
must execute in the next approved native batch. TypeScript 6's default ambient
selection requires explicit Node/Mocha types for the existing extension host
and tests; browser modules retain their separate DOM libraries.

Monaco 0.57's published package pins DOMPurify 3.4.15, which the run-1 audit
rejects for GHSA-p98j-92pf-mc4p and GHSA-6688-9rhm-gjv2. Keep the selected editor
and apply the exact upstream patched DOMPurify 3.4.16 only under its dependency
edge. The lock retains the official SHA-512 integrity. This is a documented
security patch, not an ignored advisory or a forced major downgrade. All editor
contributions and functionality remain; the shipped bundle must be regenerated
and the audit/rendered corpus run before qualification.
[Upstream advisories](https://github.com/advisories/GHSA-p98j-92pf-mc4p),
[second advisory](https://github.com/advisories/GHSA-6688-9rhm-gjv2), and
[patched source](https://github.com/cure53/DOMPurify/tree/3.4.16).

Both standalone fuzz packages declare the repository's MIT OR Apache-2.0
license. The selected libfuzzer-sys 0.4.13 bundles LLVM libFuzzer and declares
(MIT OR Apache-2.0) AND NCSA. A crate-and-version-scoped NCSA allowance preserves
the global policy and private-package checks; retain required attribution from
upstream. [License text](https://spdx.org/licenses/NCSA.html) and
[scoped policy](https://embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html#exceptions).
All four current Cargo locks lack yanked spin 0.9.8; remove its obsolete exception
rather than broadening audit suppression or downgrading resolved packages.
The canonical gate test now pins caller-first PATH precedence plus Cargo's
fallback bin, matching the reviewed tool-selection behavior.
