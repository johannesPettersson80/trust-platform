# Temperature control with Control.PID

This standalone project runs the actual `Control.PID` library through ordinary
PLC I/O. Control Studio identifies the temperature process, tunes PI, verifies
the ST, applies the complete parameter record, observes Auto operation and
restores the previous settings in Manual.

Use a matching Control Studio extension and runtime build that supports this
project's `control-pid-v1` state binding. The bundled library sources are unchanged;
`src/control-library/SOURCE-MANIFEST.txt` records their identities.

## Run the example

1. Open this folder in VS Code and start the local simulator using the normal
   runtime control. The process starts at 20°C, with zero output and Manual mode.
2. Open Control Studio and select **Identify model**. The default bounded
   experiment lasts 16 simulated seconds and uses a separate validation run.
   Inspect the measured and predicted temperature curves.
3. Select **Continue to Tune**, choose **PI**, and use a target change of 1°C,
   allowed error of 0.1°C and response time of 12s. In Settings → Advanced
   requirements, set the trial duration to 24s, then close Settings.
4. Select **Tune controller** and inspect the predicted response. Select
   **Run verification** to execute the actual ST and state-initialization cases.
5. After verification passes, select **Review changes**. Inspect the current
   and proposed parameter rows, then select **Apply and start Auto**.
6. Observe the response toward 21°C. Control Studio records the commissioning
   run and checks the response requirements. **Review rollback** followed by
   **Restore previous settings** restores the old parameter record in Manual
   while holding the current actuator output.

`fopdt-model.json` describes the configured plant for the Supply-model route.
It is deliberately marked unvalidated; its label alone grants no application
or validation authority.

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
