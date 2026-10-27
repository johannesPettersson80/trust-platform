# Upgrading Projects From The Bytecode Engine

Every truST runtime runs the project's program compiled into a program image for its CPU
(specification 44). The bytecode engine, its `program.stbc` and the `--execution-backend` option
are removed.

## Upgrade A Project

1. Rebuild the project: `trust-runtime build --project <dir>` writes the program image
   `program.tpi` and its deployment record `deployment-record.bin` into the project folder. The
   first build that needs the compiler toolchain installs it; `trust-runtime toolchain status`
   shows the toolchain the tools use.
2. Delete `program.stbc` if the project still holds it. Nothing reads it.
3. Remove `runtime.execution_backend` from `runtime.toml`. It selects nothing: `vm` and `native`
   still load, so a project written before still starts; `interpreter` and any other value are
   refused.
4. Remove `--execution-backend` from scripts. `trust-runtime run`, `play`, `build`, `conformance`
   and `bench`, and `trust-dev test`, no longer take it.

For a device, `trust-runtime build --project <dir> --target esp32c6` (or `stm32u5a9`) writes the
device's image into the project's `build/` folder. A device refuses a program in the STBC format.

## What The Runtime Reports

- A project that holds `program.stbc` and no `program.tpi`: `run`, `play` and `validate` stop
  with ``the project holds program.stbc of the bytecode engine, which this runtime does not run:
  rebuild the project with `trust-runtime build --project <dir>` ``.
- A project with no program: ``missing program.tpi at <path> (run `trust-runtime build` to build
  the project)``.
- `status` and `config.get` report `execution_backend` as `native`. `config.set` refuses
  `runtime.execution_backend`; the key is read only when the runtime starts.

## Retained Values

A computer keeps the retained values in the retain file in the format the bytecode engine wrote,
so a warm restart after the upgrade restores the values the old runtime saved (specification 44
§8.8, §15).
