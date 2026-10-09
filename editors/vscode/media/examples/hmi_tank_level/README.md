# HMI tank level

Tank T-101 is a gravity-drained tank. The real `Control.PID` holds its level with the inlet
valve FV-101, and the Alarm library watches the level and the inlet pump P-101. The operator
screens were built in the HMI Builder.

It runs without hardware: the simulator and `trust-runtime` both simulate the tank from
`simulation.toml`.

## What is in it

| File | Holds |
| --- | --- |
| `src/Main.st` | The level control with `Control.PID`, the vessel's and the pump's alarms |
| `simulation.toml` | The tank: 0.2 m², an outlet of c = 0.02 m^(5/2)/s, starting at 1.5 m |
| `runtime.toml` | The runtime display's web server, its users and what they may change |
| `hmi/` | The screens: an Overview page and an Alarms page |
| `libraries/` | Unchanged copies of the Control and Alarm libraries, so the example stands on its own |

The alarms:

| Alarm | Priority | When |
| --- | --- | --- |
| T-101 level high | Medium | above 2.5 m |
| T-101 level high-high | High | above 2.8 m; trips the pump, latches, first out |
| T-101 level low | Medium | below 0.3 m |
| T-101 level rising fast | Low | rising faster than 0.05 m/s; suppressed by design in Auto |
| P-101 motor overload | High | an overload; trips the pump, latches, first out |

## Try it in VS Code

1. Open this folder in VS Code and open `hmi/pages/overview.toml`. It opens in the HMI Builder.
2. Start debugging (F5) to run the program in the simulator.
3. Press **Live** in the builder's top bar. The page shows the simulated values; the level holds
   at 1.5 m.

## Try it on a runtime

```sh
trust-runtime build --project .
trust-runtime run --project .
```

Open <http://127.0.0.1:8080/hmi> and log in as `operator` with the password `operator` when you
change something. The demo passwords equal the user names; replace them with
`trust-runtime hmi-password` before real use.

- Change the setpoint to 2.0 m: the controller follows and the trend shows the step.
- Change it to 2.6 m: the level passes 2.5 m and **T-101 level high** comes in. Acknowledge it in
  the alarm list.
- Put the controller in Manual and open the valve to 100 %: the level rises past 2.8 m, the pump
  trips and **T-101 level high-high** is marked first out. **Reset trip** clears it once the
  level is back below the limit.

To deploy the screens from the editor, make the runtime the run target: in VS Code's settings set
`trust.runtime.mode` to `online`, `trust.runtime.controlEndpoint` to `tcp://127.0.0.1:9100` and
`trust.runtime.authTokenFallback` to the `auth_token` of `runtime.toml`. Then press **Deploy** in
the builder; **Roll back** returns to the previous screens.
