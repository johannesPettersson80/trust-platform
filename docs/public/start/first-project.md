# First Project

Use this page when you are starting from an empty folder and want one known-good
path to a running PLC project.

## Minimum project shape

The smallest useful truST project looks like this:

```text
my-plc/
  runtime.toml
  io.toml
  src/
    Main.st
    Configuration.st
```

Those files answer four different questions:

| File | Purpose |
| --- | --- |
| `src/Main.st` | the IEC logic itself |
| `src/Configuration.st` | task binding and `%I/%Q` address mapping |
| `runtime.toml` | runtime timing, control, web, retain, fault policy |
| `io.toml` | I/O backend and safe-state behavior |

## Smallest useful ST example

`src/Main.st`:

```st
PROGRAM FirstApp
VAR
    StartCmd : BOOL;
    LampOut : BOOL;
END_VAR

LampOut := StartCmd;
END_PROGRAM
```

`src/Configuration.st`:

```st
CONFIGURATION FirstConfig
TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH Fast : FirstApp;
VAR_CONFIG
    P1.StartCmd AT %IX0.0 : BOOL;
    P1.LampOut AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
```

## What success should look like

After you add `runtime.toml`, `io.toml`, and the two ST files:

- `trust-runtime build --project . --sources src` should emit `program.stbc`
- `trust-runtime validate --project .` should succeed
- runtime control surfaces should see `%IX0.0` and `%QX0.0`

Typical project tree after the first build:

```text
my-plc/
  io.toml
  program.stbc
  runtime.toml
  src/
    Configuration.st
    Main.st
```

## Good next questions

- If you want the exact zero-to-running walkthrough, keep reading the full tutorial below.
- If you want to understand how these files relate long-term, go to [Project Layout](../develop/project-layout.md).
- If you want the quickest route to runtime verification, go to [First Run And Setup](first-run-and-setup.md).

## Tutorial 13: Bootstrap From Zero

--8<-- "examples/tutorials/13_project_bootstrap_zero_to_first_app/README.md"
