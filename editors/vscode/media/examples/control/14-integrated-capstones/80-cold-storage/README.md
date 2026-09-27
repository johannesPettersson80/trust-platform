# 4. Cold-room commissioning

You will write and test a complete cold-room ON/OFF controller using only the feedback, hysteresis, dwell, and basic ST ideas from Lessons 1–3. You will choose a changed-load experiment and defend the result against fixed temperature and compressor timing requirements.

## The problem

Lesson 1 identified the cold-room process, temperature measurement, refrigeration command, and door disturbance. Lesson 2 showed that one switching temperature chatters. Lesson 3 separated ON and OFF thresholds and required a minimum state time. This assessment asks you to combine those ideas in a controller you write and inspect.

The core assessment uses the first three lessons. Optional startup, manual-transfer and measurement-fault exercises become available after the later controller-mode and recovery lessons.

## What we need

We need to establish whether the complete controller keeps the room within
its temperature band while respecting compressor switching times, including
when the heat load changes. Testing each rule separately does not establish
that the combined controller meets those requirements.

## The new idea

**Cold-room commissioning** checks whether the complete refrigeration
controller meets its temperature and operating requirements. Here we use
the simulator to test the rules together under changing heat load.

Hysteresis and dwell solve different problems. Hysteresis decides the requested state: at or above 2.5 °C request ON, at or below 1.5 °C request OFF, and between thresholds retain the previous request. Dwell decides when a changed request may become applied. If refrigeration has been ON for 6 s when PV reaches 1.5 °C, the request becomes OFF but the applied command remains ON until the normal ON duration reaches 8 s, provided the request remains OFF.

The threshold width is H = T_on - T_off = 2.5 - 1.5 = 1.0 °C. The controller
uses a 0.1 s cycle, so an 8 s normal dwell requires
8.0 s / 0.1 s = 80 elapsed updates. Here T_on and T_off are
measured-temperature thresholds, H is their temperature width, and the
dwell calculation is a timing requirement. These are design choices for this
simulation; the commissioning decision still comes from the recorded PV,
requested state, applied state and transition times.

The exercise uses one continuous experiment clock whose time zero is **Confirm and start**. The baseline run lasts 120.1 s. The door opens from 60 s through 90 s. This fixed run is long enough to show the initial cooldown, the closed-door cycle, and the changed heat load. Times in your records must be measured from that single start, not from simulator startup or from a later screen action.

Declare the temperature requirement before running: after the initial cooldown, beginning at 40 s, PV must remain in the product band 1–3 °C. This band is a teaching requirement, not a claim about a real cold room. Also require every normal completed ON and OFF interval to be at least 8 s and every applied command to be exactly 0 or 100. A command that changes at 7.9 s fails the dwell requirement; do not hide it behind an arbitrary tolerance. Sample resolution may be reported separately from the requirement.

For the changed-load run, choose and declare one different door-open duration, such as 60–105 s, or a supplied additional heat-load event. Keep SP, thresholds, dwell, initial state, clock, and product band unchanged. Your experiment design must explain why the changed load tests the same controller requirement without changing the requirement to make a result pass.

## Prediction

Before running, answer:

1. At PV=4 °C, what request should the controller make?
2. What happens as PV crosses 2.0 °C while it remains above 1.5 °C?
3. If an OFF request arrives after 6 s of ON time, when may it apply?
4. Why should hysteresis reduce switching while dwell lengthens normal intervals?
5. Will the fixed door event and your changed-load event test the same product band?
6. What evidence would make you reject the controller rather than quietly change thresholds or dwell?

Explain that command-state bounds are not temperature limits. The simulator does not guarantee a product-safe temperature merely because the command is OFF or ON.

## Implementation

Use the fixed teaching case:

- setpoint SP=2 °C;
- initial measured temperature PV=4 °C;
- compressor initially OFF;
- initial OFF-time already qualified as complete;
- refrigeration command exactly 0 or 100;
- ON threshold 2.5 °C;
- OFF threshold 1.5 °C;
- minimum normal ON and OFF dwell time 8 s.

The controller must preserve its previous requested state between 1.5 and 2.5 °C. Dwell delays a normal requested change until the current applied state has lasted 8 s. A stop or abort forces OFF immediately; that interruption is not counted as a normal completed interval. After a normal restart, the OFF timer must be qualified again before an automatic ON transition.

Write the controller in the lesson's src/Main.st. Declare REAL values for SP, PV, thresholds, elapsed state time, and dt, and BOOL values for requested and applied compressor state. Use the ST constructs already taught: IF/ELSIF/ELSE, Boolean comparisons, AND, assignment, and a retained previous state.

First calculate the hysteresis request. Do not assign a new request between thresholds. Then calculate whether the requested change differs from the applied state and whether the applied state has lasted at least 8 s. Apply the change only when both conditions are true. Keep the stop instruction separate so an abort can force OFF immediately. Increment the state timer using the actual elapsed time and reset it whenever the applied state changes, including a forced stop. Classify forced interruptions separately when assessing normal dwell intervals.

The program must expose requested state, applied state, threshold values, time in current state, dwell eligibility, door state, SP, PV, and experiment elapsed time. Use the later controller-mode and recovery lessons if you add those optional extensions.

The prepared core run keeps `EnableLifecycleChecks=FALSE`. Its original
later-topic fault demonstrations are optional and are not part of this
baseline. `DoorCloseScan=900` closes the door after90s; choose1050 for the
stated105s alternative, then restart before the comparison. One scan is0.1s.
The reset qualifies the first OFF interval; later state intervals are timed
from the actual applied transition.

## Experiment in Control Studio

1. Stop and restart Simulator. Confirm SP=2 °C, PV=4 °C, requested and applied compressor OFF, initial OFF-time qualified, state timer at the declared reset value, door closed, and all prior forces released. Do not rely on an initial history left by a previous run.
2. In Control Studio → Monitor → Response display SP, PV, ON/OFF thresholds, requested state, applied state, time in current state, dwell eligibility, compressor command, door state, and experiment elapsed time.
3. Write down the fixed product band 1–3 °C, 8 s normal dwell requirement, and command-state rule before starting. For the baseline, select Test → Observe without excitation, use disabled amplitude 0, and set duration 120.1 s.
4. Select Review test and Confirm and start. Treat that action as clock time zero. Observe the initial cooldown and the door opening at 60–90 s. Do not change SP, thresholds, dwell, or command manually during the automatic run.
5. Record every normal applied-state change and its interval. Count the normal state changes over the same 120 s enabled window in both runs and compare the counts; no additional switching-count limit is imposed. Check that a request between thresholds retains its previous value and that a requested reversal waits for 8 s.
6. At completion, confirm automatic control is disabled, applied command is OFF, door is closed, and the record is retained. For a changed-load run, restart from the exact initial state, choose and declare your alternate door duration or heat-load event, and repeat the same 120.1 s clock.
7. Select Abort test if PV leaves −10 to 10 °C, a command is not 0 or 100, a signal stops updating, or you cannot supervise the run. Abort forces OFF immediately. If Simulator is unresponsive, use the debug toolbar Stop and confirm OFF before restarting.

## Evidence and decision

The controller passes the core assessment only when:

- the baseline record has one continuous Confirm-and-start clock and the declared 60–90 s door event;
- PV remains in 1–3 °C from 40 s through completion;
- every normal completed ON and OFF interval is at least 8 s;
- every applied command is exactly OFF/0 or ON/100;
- the changed-load run uses a learner-declared different door duration or supplied heat event while retaining the same initial state and requirements; and
- the learner explains any failure from the trace rather than changing requirements afterward.

Compare the normal switching counts for the baseline and changed-load runs, and explain any difference using the temperature and door-state traces. Separate forced shutdown intervals from normal dwell intervals. A stop or abort demonstrates recovery to OFF; it does not prove a normal dwell interval. The product band is an exercise acceptance rule, not a real refrigeration safety limit.

## Conclusion

A complete cold-room controller uses hysteresis to prevent chatter and dwell to protect normal compressor timing. The learner writes the state logic, runs a fixed baseline, chooses a changed-load test, and decides from explicit temperature, timing, and command evidence.

## Reason for the next lesson

The cold-room controller uses the refrigeration equipment’s two available states. It regulates temperature within a band by switching between them. The next application is an electric water heater whose power can be adjusted continuously; we need a rule for how much heating to request, not only whether it is ON or OFF.

[Proportional control](../../01-feedback-foundations-and-temperature-control/04-water-heater-p/README.md) calculates that variable command from temperature error.
