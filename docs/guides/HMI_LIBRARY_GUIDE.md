# HMI Library Guide

The HMI library holds one block, `Hmi.Display`. A program calls it to serve the runtime's HMI
screens: after the task in which it was called, the runtime applies the commands the screens
queued, copies the values the open screens show and reads the alarms. The screens therefore run
at the interval and priority of the task you choose, and nothing of the HMI runs between scans.
The contract is specification 39, `docs/specs/39-hmi-builder.md`, §18 phase 5.

## Package Setup

```toml
[project]
include_paths = ["src"]

[dependencies]
Hmi = { path = "../../libraries/hmi", version = "0.1.0" }
```

In VS Code, **New HMI Screen** adds the library, the program below and its task for you.

## Use

Call the block once, in a program of its own, in a task with a lower priority than the control
tasks:

```iecst
PROGRAM Tank
VAR
    Level : REAL;
END_VAR
    Level := Level + 0.1;
END_PROGRAM

PROGRAM Screens
VAR
    Display : Hmi.Display;
END_VAR
    Display();
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Control (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH Control : Tank;
    TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);
    PROGRAM Screens WITH ScreenTask : Screens;
END_RESOURCE
END_CONFIGURATION
```

- The screens get new values after each call, at most four times a second, so 250 ms gives four
  updates a second. A slower task gives slower screens, and the display expects it: values that
  arrive every second are not marked stale.
- A command waits for the next call. One the program has not taken within its time limit is
  withdrawn and never applied later.
- The alarms are read at each call, so their records and the OpenOT history follow the call too.
  A program that uses the Alarm library calls the block even when no screen is open.
- Without a call the screens show no live values; the HMI Builder then warns at the top of
  `hmi/screen.toml`.

## Blocks

| Block | Purpose |
| --- | --- |
| `Hmi.Display` | Serves the screens after the task it is called in. Output `Calls`: how often it has been called. |

## Design Rules

- One call serves every screen. More calls, in faster tasks, serve more often, not better.
- Tasks do not interrupt each other: the screens are served after the work of the task that calls
  the block, so a lower priority keeps the control tasks first.
