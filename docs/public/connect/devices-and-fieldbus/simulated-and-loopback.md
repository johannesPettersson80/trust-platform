# Simulated And Loopback

## Which one should you start with?

| Driver | Start with it when | What it proves |
| --- | --- | --- |
| `loopback` | you want the fastest `%Q -> %I` local feedback path | mappings and basic runtime control work |
| `simulated` | you want software-only process behavior without hardware | runtime, scenarios, and test workflows work |

## Why start here

- fastest path to first success
- safest early commissioning loop
- ideal for tests and agent-driven workflows

## Minimal loopback config

```toml
[io]
driver = "loopback"
params = {}

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
```

## Minimal simulated config

```toml
[io]
driver = "simulated"
params = {}

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
```

## Transitioning to real hardware later

1. keep the same ST logic and `VAR_CONFIG` mapping
2. replace the driver/backend in `io.toml`
3. preserve safe-state outputs
4. re-run validate, then commission against the real transport

## Good uses

- tutorials and onboarding
- CI and deterministic test loops
- agent-driven diagnose/build/reload workflows
- early HMI or runtime-cloud integration without hardware

## Next

- [Simulation](../../operate/simulation.md)
- [Driver Matrix](driver-matrix.md)
