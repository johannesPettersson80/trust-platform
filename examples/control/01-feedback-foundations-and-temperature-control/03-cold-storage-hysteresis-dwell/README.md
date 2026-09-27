# 3. Hysteresis and dwell

In this lesson, you will use separate temperatures for switching refrigeration ON and OFF, then require refrigeration to remain in each state for a minimum time before it may change again.

## The problem

Lesson 2 used one temperature for both switching decisions. The room temperature changed gradually, but every small crossing of 2 °C could immediately reverse the refrigeration command. The command changed state 760 times in 120 s.

A physical compressor may also need to remain ON or OFF for a minimum time before it can safely change state. Preventing command reversals at small temperature crossings and preventing switches before that minimum time has passed are two different problems.

## What we need

We need to prevent small temperature fluctuations from repeatedly reversing
the refrigeration command, and ensure normal switching respects the
compressor's minimum ON and OFF times. These are two separate requirements.

## The new idea

**Hysteresis** uses different switching thresholds and retains the previous
state between them. **Dwell time** is a separate minimum time before the
applied state may normally reverse. Here hysteresis requests refrigeration
ON at 2.5 °C and OFF at 1.5 °C:

| Measured room temperature | Requested refrigeration state |
| --- | --- |
| At or above 2.5 °C | ON |
| Between 1.5 and 2.5 °C | Keep the previous state |
| At or below 1.5 °C | OFF |

The 1 °C distance between the two switching temperatures is the **hysteresis
width**. In symbols, H = T_on - T_off = 2.5 - 1.5 = 1.0 °C, where T_on
and T_off are the upper and lower measured-temperature thresholds. The
subtraction gives a temperature width, not a command percentage. Keeping the
previous state between the thresholds prevents a small movement around 2 °C
from reversing the command.

The 1 °C distance is a teaching choice that makes the effect easy to see. In a physical system, it must be chosen from the allowed temperature variation and the uncertainty of the temperature measurement.

Hysteresis decides which state is **requested**. A second rule decides when that request may be **applied** to the refrigeration system.

This lesson requires the applied state to remain ON or OFF for at least 8 s. This minimum state time is called **dwell time**. Eight seconds is chosen so the postponement is visible during the simulation. A physical value must come from the compressor requirements.

For example, suppose refrigeration has been ON for 6 s when the temperature reaches 1.5 °C:

1. Hysteresis requests OFF.
2. The applied state remains ON because the 8 s minimum ON-time is not complete.
3. After 8 s, the OFF request may be applied if it is still requested.

The state timer uses the controller's 0.1 s update interval. The 8 s minimum
therefore requires 8.0 s / 0.1 s = 80 completed updates. At 7.9 s only 79
updates have elapsed, so the normal OFF transition is still inhibited; at
8.0 s it is eligible. A stop or abort remains an immediate safety action and
is classified separately from a completed normal dwell interval.

Watch both requested and applied states to see when dwell postpones a change. The simulator treats the initial OFF period as already complete, allowing cooling to start immediately. Stopping or aborting the experiment forces OFF even if the minimum ON-time is unfinished; dwell governs normal automatic switching, not shutdown.

## Prediction

Before running, answer these questions:

1. The room starts at 4 °C. Which state will hysteresis request?
2. What happens to the request as temperature falls through 2 °C but remains above 1.5 °C?
3. If refrigeration has been ON for only 6 s when the request changes to OFF, when may the applied state change?
4. Compared with Lesson 2, will refrigeration change state more or less often?
5. Why will using two switching temperatures produce a wider temperature cycle?

## Implementation

Open [`src/Main.st`](src/Main.st) and identify the requested state, applied state and state timer.

**ST practice.** Write the hysteresis decision using `IF` for the upper threshold and `ELSIF` for the lower threshold. `ELSIF` tests its condition when the first condition is false. Do not assign a new request between the thresholds: the Boolean retains its value between scans.

Write a separate condition that applies a changed request only when the elapsed time in the current applied state has reached its minimum. Use `AND` to require both conditions and `<>` to test that the states differ. Increment the state timer by the elapsed seconds and reset it when the applied state changes. Initialize the first OFF interval as complete, as described above.

Check both possible previous requests at 2 °C, then check an OFF request at 6 s and 8 s of ON-time. Keep the shutdown instruction separate from normal dwell so it can force OFF immediately.

## Experiment in Control Studio

1. Open this lesson's project, start **Simulator**, and open **Control Studio → Identify**. Choose **Project experiment** from **Method**, then use **Signals** to select the recorded channels. Keep setpoint and measured temperature in °C and the door state visible.
2. Confirm temperature setpoint 2 °C, measured temperature 4 °C, requested and applied refrigeration OFF, dwell restriction inactive, automatic control disabled, door closed, and `P1.ExperimentEnabled` false.
3. Add the requested state, applied state, dwell restriction, time in the current state in seconds, and number of state changes to the visible signals. The dwell restriction indicates that a requested switch is waiting for the minimum time to pass.
4. Answer the prediction questions before starting; **Confirm and start** begins the experiment recording.
5. Stay on **Identify**. Select **Review project test** and check the declared **120.1 s** duration, covering the initial cooling and repeated switching with the door closed and open. All event times start at **Confirm and start**.
6. Read the reviewed plan. Starting sets `P1.ExperimentEnabled` to true, enabling both switching rules; ending the experiment disables them and switches refrigeration OFF. Keep the setpoint, switching temperatures and minimum times unchanged during the run.
7. Select **Confirm and start**. Watch the request remain unchanged between 1.5 and 2.5 °C. When requested and applied states differ, confirm that the dwell restriction is active.
8. The experiment opens the door from 60 to 90 s. Continue observing both refrigeration states as the additional heat changes the process variable.
9. At completion, wait for restoration confirmation, then confirm `P1.ExperimentEnabled` false, automatic control disabled, and both refrigeration states OFF.

Select **Abort test** to stop early or if temperature leaves −10 to 10 °C; the temperature check is yours to make. Confirm both refrigeration states OFF. If the runtime is unresponsive, stop the simulator with the debug toolbar. To repeat fairly, restart the simulator at 4 °C with the same initial states, completed OFF-time and door interval.

This project runs hysteresis with dwell. Compare your observations with the three saved cases below; they use the same room, starting temperature, door event and run duration, changing only the switching rule. The comparison does not require editing the program or assuming the project has a case selector.

## Evidence and decision

The [retained simulation data](figures/hysteresis-dwell-comparison.csv) compares three rules. Temperature movement is the highest measured temperature minus the lowest **during the door-open interval, 60 to 89.9 s**. It excludes the initial cooldown from 4 °C. State changes are counted over the 120 s run:

| Rule | Temperature movement | State changes | Shortest completed ON/OFF times |
| --- | ---: | ---: | ---: |
| One switching temperature | 0.025 °C | 760 | 0.1 s / 0.1 s |
| Two switching temperatures | 1.013 °C | 12 | 6.8 s / 5.1 s |
| Two temperatures and 8 s dwell | 1.265 °C | 12 | 8.0 s / 8.0 s |

Two switching temperatures reduce the state changes from 760 to 12 because the measured temperature must travel from one boundary to the other before the request reverses. The wider temperature movement is the cost of waiting for that travel.

The first three predictions follow the request and its timing: at 4 °C the request is ON; crossing 2 °C while cooling leaves that request ON until 1.5 °C; an OFF request after only 6 s of ON-time must wait another 2 s, provided it remains OFF. The table answers the last two predictions: switching is less frequent, but temperature moves through a wider range.

With hysteresis alone, some states last less than 8 s. In this saved run, adding dwell makes every completed ON and OFF period last at least 8 s. In 65 recorded samples, the requested and applied states differ while the dwell restriction is active. This shows that dwell postponed a change even though it did not further reduce the total count of 12. It can also carry temperature beyond a switching boundary while the actuator waits.

We therefore use both rules: hysteresis prevents small temperature movements from changing the request, and dwell time prevents the applied state from changing too soon.

## Conclusion

Hysteresis and dwell time solve different problems. Hysteresis decides when the requested state changes. Dwell time decides when that request may be applied.

Together they replace rapid chatter with slower, deliberate refrigeration cycles.

## Reason for the next lesson

Hysteresis now prevents small temperature changes from reversing the request, and dwell postpones transitions that would occur too soon. Those separate rules must next be combined in a controller that meets the room’s temperature and switching requirements when the heat load changes.

[Cold-room commissioning](../../14-integrated-capstones/80-cold-storage/README.md) asks you to write that complete controller, choose a changed-load experiment and assess it against the stated requirements.
