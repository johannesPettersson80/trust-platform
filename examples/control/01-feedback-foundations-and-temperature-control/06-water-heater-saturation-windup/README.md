# 7. Saturation and windup

You will distinguish a PI request from heater power that can actually be applied. You will calculate why a 45 °C setpoint is unreachable for this heater and why returning to 35 °C can still leave the heater at full power.

## The problem

PI can remove steady error while the heater has enough power. Now we ask what
happens when the desired temperature exceeds its capacity. Start settled at
35 °C with 75% holding output and request 45 °C, although full power supports
only 40 °C. We will inspect what happens to the stored integral demand.

## What we need

We need to distinguish a temperature the heater cannot reach from delayed
recovery caused by accumulated controller demand. That requires comparing
what PI requests, what the heater can apply and what remains stored in I.

## The new idea

**Actuator saturation** occurs when applied output reaches a physical or
configured limit. **Integral windup** is the continued accumulation of
controller demand while the actuator cannot follow it. A larger calculated
request cannot create power beyond the heater's capacity.

Let I be the integral contribution in percent and e = SP − PV the temperature
error in °C. Requested and applied outputs are:

$$
u_r=K_p e+I,\qquad u_a=\min(100,\max(0,u_r)).
$$

Here max selects the larger value and min the smaller. A 125% request gives
100% applied and a 25-point mismatch; the extra request adds no heat.

The integral update still adds $K_i e\Delta t$, so positive error during upper saturation makes I grow without increasing delivered power. This accumulation beyond what the actuator can deliver is **integral windup**. When SP returns to 35 °C, P changes immediately at the next scan, but I does not reset. If the reduction in P is large enough, the request leaves the limit on that scan. Otherwise the retained I keeps the request above 100%, and negative error must reduce it before applied output can decrease.

The supplied steady-temperature relationship for this teaching heater is $T_{eq}=20+0.2u_a$, with temperature in °C and command in percent. At zero power the equilibrium is 20 °C; each additional percentage point raises the equilibrium by 0.2 °C. Full power therefore supports $20+0.2(100)=40$ °C. This illustrative relationship makes 45 °C infeasible; the [energy-balance lesson](../15-water-heater-energy-balance/README.md) later derives it from delivered power and heat loss. Command changes also have a 1 s delivery delay before their changed heating reaches the water.

The limit acts after the controller has formed its request. It does not change the error, and the limited command is not a measurement of delivered heat. Keep three questions separate: what PI asked for, what the output limiter allowed, and what the process temperature did after the delivery delay. This separation is what makes windup visible in both the signal record and the physical response.

## Prediction

At SP=45 °C, PV=35 °C and I=75%, calculate P, the first integral increment, and the request using $K_p=2$ percentage points/°C, $K_i=0.5$ percentage points/(°C·s), and $\Delta t=0.1$ s. Predict the signs of I and $u_r-u_a$ while PV remains below 45 °C. After SP returns to 35 °C, can applied output decrease immediately? What observation separates brief saturation from substantial windup?

## Implementation

This lesson deliberately declares `Parameters.OutputMaximum := LREAL#200.0`, while
the heater can apply only 100%. The real PID already contains anti-windup, but
it cannot correct a limit that it has not been told about. Its internal maximum
never binds in this experiment; the native regression checks that fact. This
is an intentional simulation commissioning failure, not a configuration to
deploy. Lesson 8 corrects the declared maximum without adding another controller.

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

1. Start **Simulator** and establish the hot state: SP=PV=35 °C, I=75%, P=0%, requested and applied output=75%, and no added load. The preceding 1 s command history must also be 75%, so delivered heating is initially steady. This deliberately replaces the earlier 20 °C warming start.
2. In **Control Studio → Identify**, display SP, PV and error in °C; retained I and current requested power in percent; paired requested power and applied feedback in percent; and their phase-aligned mismatch in percentage points.
3. In **Identify → Method → Project experiment**, click **Review project test** for the 70.1 s recording envelope (70 s active sequence), then select **Confirm and start**. The project experiment automatically records the reviewed sequence. At time zero, SP becomes 45 °C; at 10 s it returns to 35 °C. Use time from **Confirm and start**.
4. Record when request exceeds 100%, whether I continues rising, I at the 10 s return, and how long applied output remains 100% afterward. Observe the 60 s recovery interval.
5. After restoration, inspect the complete recording on Identify and use **Signals** to select its quantities. Confirm 0% controller command immediately and 0% applied feedback at the following completed input publication; after the delivery delay, heat loss cools the water. For a repeat, reset the simulator to the complete hot starting state, including I and command history.

Abort outside 0–80 °C, outside 0–100% applied output, or on unusable measurements. Select **Abort test**, confirm immediate 0% command and 0% applied feedback on the following completed input publication, and stop the simulator if it is unresponsive. An over-range request is intentional; the applied signal must remain bounded.

## Evidence and decision

At the initial 10 °C error, P is 20%, the first increment is $0.5(10)(0.1)=0.5$ point, and the request is 95.5%. If error persists, I rises and the request crosses 100% while applied output stays at 100%. That is evidence of windup only when retained I also grows.

The request can cross the limit before the temperature reaches its maximum because the controller integrates error on every scan while the process responds slowly. Do not infer windup from a high request alone: record its history, the applied signal, and the integral state. Likewise, do not infer a capacity failure merely because PV is below SP during a transient; the equilibrium limit and sustained full output matter.

For an illustrative post-return state PV=39 °C and I=112%, the next request is $112+2(-4)+0.5(-4)(0.1)=103.8$%; applied output remains 100%. This is a calculation, not a measured sample. Judge temperature and controller state separately. Inability to maintain 45 °C is a capacity result, not a tuning failure.

## Conclusion

Saturation limits applied power but does not limit the ordinary integral state. Windup accumulates an integral contribution the heater cannot deliver and can keep output saturated after the setpoint is reduced.

## Reason for the next lesson

The controller needs the actual actuator limit so its integral state follows what the heater can deliver; the separate applied feedback verifies that behavior. [Anti-windup](../07-water-heater-anti-windup/README.md) adds that correction and repeats the same hot 45-to-35 °C test.
