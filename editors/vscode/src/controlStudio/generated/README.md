# Generated Control Studio engine types

The `.ts` files in this folder are generated. Do not edit them.

Each one declares the request, answer or notification type of one engine message, with the
same field names the engine writes (`camelCase`). They are generated from the engine's JSON
Schema, which the Rust types of `trust-control` derive.

To regenerate after a message type changes, run these two commands on the build server:

1. `cargo run -p trust-control --bin export-schema -- <schema-folder>` writes one `<Type>.json` per message type.
2. In `editors/vscode`: `npm run generate:control-studio-types -- <schema-folder>` writes one `<Type>.ts` per schema into this folder.

`python3 scripts/check_control_studio_types.py` (repository root) regenerates the declarations
into a temporary folder and fails when the files here differ.
