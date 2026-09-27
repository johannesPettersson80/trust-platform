# 8. Anti-windup

You will correct integral windup so the heater can recover when the requested temperature returns within its capacity. You will calculate how feedback from the applied output changes the integral state, then compare recovery with incorrect and correct declared actuator limits.

## The problem

Lesson 7 showed how I can keep growing while the heater is already at 100%.
Reducing the setpoint changes P immediately but does not remove that stored
demand. We need a correction that accounts for what the heater actually applies.

## What we need

We need the integral demand to remain consistent with output the heater
can actually apply. That should reduce stored excess during a limit and
allow timely recovery when the requested temperature becomes reachable.

## The new idea

**Anti-windup** prevents unusable integral demand from causing excessive
recovery delay. The back-calculation method corrects integral state using
applied output minus requested output. It changes the stored demand, not
the heater's available power.

Use the same Kp = 2 percentage points/°C and Ki = 0.5 percentage points/(°C·s).
All output contributions are on the percentage-command scale. Let $P=K_pe$
be the current proportional contribution, $I^*$ be I after ordinary integration,
and $u_r=P+I^*$ the request. The integral consistent with applied output $u_a$
is $u_a-P$. The required correction is

$$
\Delta I_{track}=(u_a-P)-I^*=u_a-u_r.
$$

Apply a fraction each scan:

$$
I_{next}=I^*+\frac{\Delta t}{T_t}(u_a-u_r).
$$

The **tracking time** $T_t$ sets how quickly this state correction acts. With $T_t=1$ s and $\Delta t=0.1$ s, one tenth of the mismatch is corrected each update. At upper saturation the correction is negative; at lower saturation it is positive; when request and applied output agree it is zero. This is **back-calculation anti-windup**: calculate the integral correction backward from the applied command. It limits stored excess; it cannot make 45 °C feasible.

The correction adjusts the controller state using the output limit. During ordinary operation, $u_a=u_r$ and tracking contributes nothing, so PI behaves as before. During saturation, tracking moves I toward the value compatible with the command that was applied. It does not measure or cancel the heater's delivery delay, and gradual tracking need not make the unlimited request exactly equal to the limit. A shorter tracking time changes I faster; it is a controller choice that must be checked against the delayed thermal process.

## Prediction

For old I=110%, error=5 °C, calculate $I^*$, $u_r$, tracking correction and next I with $K_p=2$, $K_i=0.5$, $\Delta t=0.1$ s. Then calculate the correction for a −20% request applied as 0%, and for equal requested and applied output. Predict how smaller I should affect recovery after SP returns to 35 °C.

## Implementation

The correction is `Parameters.OutputMaximum := LREAL#100.0`, matching the actual
actuator. The `BackCalculation` selection and Tt=1 s are unchanged from
Lesson 7. There is no anti-windup-off mode or separate tracking integrator.
For this static limit, the library uses its declared limited candidate for
back-calculation; applied feedback independently verifies what the actuator
did. The delayed readback is not fed into `PIDMode.Track`.

The complete application is in [src/Main.st](src/Main.st). It calls the real
`Control.PID` with Kp=2, Ki=0.5, Kd=0, proportional weight 1 and
`BackCalculation` with Tt=1 s. The 0.1 s task uses rate bounds that do not
constrain this experiment. `HeaterCommand := Controller.Update(Input := Input)`
owns `%QL0` directly. The native actuator owns its separate 0–100% limit;
measured temperature returns on `%IL0` and applied actuator feedback on `%IL8`.

The controller is initialized once with the measured hot state, previous SP
35 °C and nominal/saturated output 75%. Manual preparation holds 75%. Enabling
applies the 45 °C step immediately, giving the same first 95.5% request. The
setpoint returns to 35 °C at 10 s. Completion or abort selects Manual zero;
there is no separate output equation or independent stored integrator.

`GetState()` supplies value copies before and after Update. The displayed
pre-correction request is reconstructed from the real prior nominal state and
this call's P/I increments. Retained I is the real post-correction nominal
output minus current P; the tracking correction is the change from candidate
to retained nominal output. These calculations are observations only.

Keep the sample phases distinct:

| Quantity | Binding | Meaning |
|---|---|---|
| Current requested power | `%ML8` | This call's candidate before anti-windup correction. |
| Controller command | `%QL0` | This call's actual PID return, after its declared limits. |
| Applied heater power | `%IL8` | Native actuator value from the preceding completed step, before heat-delivery delay. |
| Paired requested power | `%ML88` | The preceding request associated with that applied feedback. |
| Unavailable power | `%ML16` | Paired request minus applied feedback, in percentage points. |
| Stored I / tracking correction | `%ML64` / `%ML80` | State and correction from the current PID call. |

Compare paired request with applied feedback when assessing physical mismatch.
Comparing the newly calculated request with the previous applied value would
mix two scans. A zero command is immediate; measured zero applied output arrives
at the next completed input publication. The one-second delivery delay and
stored heat still govern the later temperature response.

From this lesson directory, run:

```sh
trust-runtime check --project .
trust-dev test --project tests
```

The second command checks the worked arithmetic in
[tests/src/Arithmetic.st](tests/src/Arithmetic.st), not a second runtime
controller. From the repository root, the complete native regression command is:

```sh
cargo test -p trust-runtime --test control_curriculum_heater_windup_pid
```

It compares both complete hot runs with their preserved pre-conversion
physical references, checks the real PID owner and scan-aligned feedback, and
checks abort timing. Restore the entire hot reset before repeating a run.

## Experiment in Control Studio

1. Start **Simulator** at the same hot state as Lesson 7: SP=PV=35 °C, I=75%, P=0%, requested/applied command=75%, no extra load, and a preceding 1 s command history at 75%.
2. In **Control Studio → Monitor → Response**, display SP/PV/error in °C, retained I and current requested power in percent, paired requested power and applied feedback in percent, and tracking correction in percentage points.
3. In **Control Studio → Test**, use **Review test** for the 70.1 s recording envelope (70 s active sequence), then select **Confirm and start**. Test automatically records the reviewed sequence. All times begin at **Confirm and start**, with SP 45 °C at time zero and SP 35 °C at 10 s. Run the Lesson 7 project with its deliberately incorrect 200% controller maximum, then run this corrected Lesson 8 project with the same complete hot reset; reset the state between runs.
4. After each run reaches terminal restoration and **Complete recording**, use **Validate → Save result** in its own project. Expand **Saved results**, select that revision and choose **Open** to inspect it. Record I immediately before the reduction, time until applied output falls below 100%, peak PV, and the time after the setpoint reduction when PV first enters 34.8–35.2 °C and remains there until shutdown. The 60 s recovery interval allows these effects to be observed; if PV has not settled by the end, record that result.
5. Abort outside 0–80 °C, outside 0–100% applied output, or on unusable measurements. Select **Abort test**, disable heating, confirm immediate 0% command and then 0% applied feedback at the following completed input publication. Stop the simulator if it is unresponsive. At normal completion confirm immediate zero command and then zero applied feedback; after the delivery delay the water cools. Reset the entire hot state before repeating.

## Evidence and decision

The worked update is $I^*=110.25\%$, $u_r=120.25\%$, correction $0.1(100-120.25)=-2.025$ points, and $I_{next}=108.225\%$. At a −20% request applied as 0%, correction is +2 points; equal values give zero.

Compare reduction time, peak temperature, and final-band recovery separately. Faster exit from saturation is evidence of state correction, and the comparison must also report peak temperature and settling separately. A shorter saturation interval alone does not establish a better temperature response. Both runs have the same 40 °C full-power equilibrium, so inability to reach the 45 °C setpoint is not evidence against anti-windup.

Also compare the sign and size of the tracking correction during each run. It should be negative while the request is above 100%, and it should return near zero when the request becomes attainable. If the correction has the wrong sign, the state will reinforce saturation rather than remove it. Record that implementation failure separately from a process response that is simply slow.

## Conclusion

Applied-output feedback corrects the integral state toward the contribution consistent with the limited command. It reduces unnecessary stored heating after saturation while preserving ordinary PI action when the request is attainable.

## Reason for the next lesson

The integral state now accounts for the output limit, and we have assessed recovery from an excessive setpoint. Ordinary operation poses two further questions: how should the loop follow a feasible setpoint change, and how should it recover from increased heat loss while the setpoint stays fixed?

[Setpoint and disturbance response](../09-setpoint-and-disturbance-response/README.md) separates those causes and sets a useful requirement for each response before tuning.
