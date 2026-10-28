# Temperature control with Control.PID

A heater loop that runs the actual `Control.PID` library through ordinary PLC I/O. The simulated plant
gains 1.5 °C per % of output with a 4 s lag and a 0.5 s delay; it starts at 20 °C, in Manual, with 0 %
output. Control Studio identifies the process, tunes a PI controller to your requirements, validates it on
the actual ST, applies it in Auto and can restore the previous settings. The bundled library sources are
unchanged; `src/control-library/SOURCE-MANIFEST.txt` records their identities.

## Set up the loop

The project ships `control-loop.json`, so **Open Control Studio** opens it directly. To set the loop up
yourself, delete that file and run **Configure control loop** with the structure **One PID loop**. The form
takes the controller `P1.Controller`, its parameter record `P1.Parameters`, the measurement
`P1.ProcessValue`, the output `P1.ManipulatedValue`, the setpoint `P1.Setpoint` and the manual output
`P1.ManualOutput` from the compiled program. Enter the units and ranges (degC, 0–100; %, 0–100), a 10 %
identification step for 20 s, the Auto/Manual signal `P1.AutoMode` (TRUE means Auto) and the startup-done flag
`P1.Initialized`, then **Save loop**. Validate starts the program with that flag set, so its startup code
cannot overwrite the controller state under test.

## Measured in Control Studio

- **Identify plant** (the output stepped 10 % for 20 s): G(s) = 1.47/(3.9 s + 1)·e^(−0.7 s), against the
  configured 1.5/(4 s + 1)·e^(−0.5 s). The extra 0.2 s of delay is the output rate limit: the 10 % step
  takes 0.4 s at 25 %/s, and the controller lives with the same limit.
- **Tune** with PI, the Automatic method and these **Requirements…**: setpoint change 1 °C, steady-state
  error 0.1 °C, settling time 12 s, settling band 0.05 °C, validation duration 24 s. Result:
  Kp = 0.8013 %/°C, Ki = 0.2055 %/(°C·s).
- **Validate controller**: settled in 6.8 s, remaining error 0.0122 °C, the actual ST within 1.93 % of the
  model's prediction, and all 44 generated checks of ranges, controller states and transfer passed.
- **Apply** wrote the complete parameter record and started Auto from the validated operating point (20 °C,
  0 %), then recorded the 24 s response. **Restore previous settings** returns the previous record in Manual
  while holding the current output.

How the gains follow from the model (SIMC): the integral time Ti = Kp/Ki = 3.9 s equals the plant lag and
cancels it, and Kp = T/(K·(λ + θ)) sets the closed-loop time constant λ. The More robust response,
λ = 4·max(θ, T/3) = 5.2 s, would still be 0.11 °C from the target after 12 s, outside the 0.1 °C allowed, so
Automatic takes the Balanced response, λ = 2.6 s. A shorter settling time needs a smaller λ; the delay sets
the limit, because λ below θ makes the loop sensitive to any error in the delay.

`fopdt-model.json` is the configured plant as a model file for the **Supplied plant model** method. It is
marked unvalidated, so Validate first checks it against a fresh recording.

## Signals and timing

| Signal | Binding | Unit |
|---|---|---|
| Setpoint | `P1.Setpoint`, `%ML0` | °C |
| Temperature | `P1.ProcessValue`, `%IL0` | °C |
| Controller output | `P1.ManipulatedValue`, `%QL0` | % |
| Manual command | `P1.ManualOutput`, `%ML16` | % |
| Auto/Manual mode | `P1.AutoMode`, internal BOOL | true = Auto |

The task and controller update interval are both 100ms. The configured plant
has gain 1.5°C/%, time constant 4s, delay 0.5s and ambient operating point 20°C.
Output is bounded to 0–100% and its rate to ±25%/s.

`Parameters` has declaration-time defaults. Each scan copies the complete record
through `Controller.Configure`; changing it does not reset controller history.
`ParameterRevision` increments only when that complete record changes.
`Controller.Initialize` runs at normal startup. The reviewed isolated state
contract explicitly includes the application startup flag so startup logic
cannot overwrite the seven controller histories being verified.

## Adaptation boundary

Keep the controller library and its Configure/Initialize/Update ownership.
Change I/O bindings, units, scaling, actuator constraints, process model and
requirements together, then repeat identification/validation and actual-ST
verification. Update the declared cycle interval if the task period changes.

This reference currently targets the local simulator. A physical deployment
needs the site's I/O driver, measurement-quality and fault supervision,
watchdog/safe-state configuration, commissioning permission and plant-specific
qualification. The simulation response does not provide that site acceptance.
