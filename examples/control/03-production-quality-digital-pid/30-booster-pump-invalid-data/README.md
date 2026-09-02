# Invalid data and safe recovery on a booster pump

Course position: Lesson 30 of 84. Previous lesson: [Atomic PID updates and
signal ordering](../29-booster-pump-atomic-update/README.md). Next lesson:
[Controller selection](../../04-controller-selection-and-plant-models/31-controller-selection/README.md).

**Lesson objective:** Map each invalid input class to a deterministic fault,
prove the invalid scan cannot update the PID, and restore Auto only after
acknowledgement, valid evidence, and state reinitialization.

This lesson uses simulation. Its fault injections are not physical-plant
evidence and shall not be reproduced on a running pump for training.

**Equipment:** Variable-speed booster pump with recirculation valve.

## Where this lesson begins

Lesson 29 guaranteed coherent ordering when all inputs were valid. Coherence
does not make a value trustworthy. The booster pump depends on pressure, valve feedback, sensor quality, timing,
and configuration data every scan. A plausible number can be stale. A fresh
number can be outside the equipment range. A complete record can still contain
reversed limits.

The station must decide whether its inputs are trustworthy before calling the
controller.

One bad scan must be rejected before it changes retained PID state. Recovery
then needs a known safe output, operator acknowledgement, qualified current
data, state reinitialization, and separately renewed authority.

## Building the validity guard

The **valid numeric domain** contains finite numbers that arithmetic can use.
The **physical plausibility range** contains values the equipment can actually
produce safely. Both checks are required.

The **data quality** says whether the acquisition path considers a sample
usable. The **measurement age** is elapsed time since that sample was acquired.
The **freshness limit** is its maximum permitted age. Here good quality and age
from 0 through 0.5 seconds are both required.

The **measured cycle time ($\Delta t$)** is actual elapsed time between controller
updates. A **deadline** is the latest acceptable completion time. A
**watchdog** independently detects a missed deadline. This lesson accepts `dt`
above zero through 0.2 seconds and separately rejects a missed-deadline input.

A **fault cause** identifies the invalid class. The **first-fault latch** keeps
the first detected cause visible even if another fault appears later. The
**acknowledgement** records that an operator saw the fault; it does not clear
the latch.

A **hold-last output** preserves the previous actuator command. That can
preserve the action that created a hazard. The reviewed teaching topology
therefore chooses a **safe output** of 30 percent recirculation. A real plant
requires its own process-safety review.

Nonfinite data is rejected by the runtime's typed I/O boundary before the PLC
scan. The station guard then rejects finite but implausible values. Pressure
must be 0 through 12 bar and valve feedback must be 0 through 100 percent.

Each range check is a conjunction. For example,

\[
PV_{\mathrm{valid}}=(PV\ge0)\land(PV\le12).
\]

Both comparisons must be true. Positive infinity fails the upper bound;
negative infinity fails the lower bound; and an unordered NaN comparison is
not true. The runtime boundary remains the first defense, while the station
range guard prevents physically implausible finite values from reaching PID
arithmetic.

The stable diagnostic map is: code 1 means invalid numeric or physical range;
code 2 means stale or bad-quality measurement; code 3 means invalid cycle
time; code 4 means missed deadline; and code 5 means invalid configuration.
Code 0 means no active fault.

An invalid scan does not call the PID and does not call `Configure`. It cannot
advance the active revision. Recovery requires acknowledgement, three
consecutive valid scans after the acknowledgement scan, explicit reset, state
initialization, and a separate renewed Auto request on a later scan. The
acknowledgement scan clears no fault and leaves the recovery count at zero.

## Deriving the recovery sequence

Begin at 6 bar and 40 percent with active revision 0. Apply a candidate with an
80 percent output minimum and a 20 percent output maximum.

The limits are reversed, so the prediction is code 5, revision 0, no PID or
configuration call, and 30 percent safe output. Correct values alone cannot
clear the latch.

The recovery counter follows

$$
N_k=
\\begin{cases}
\\min(N_{k-1}+1,3), & \\text{acknowledged and every input valid},\\\\
0, & \\text{otherwise}.
\\end{cases}
$$

The acknowledgement scan is deliberately excluded, so it leaves the count at
zero. Valid scans 1, 2, and 3 produce counts 1, 2, and 3. Reset is accepted only
when

$$
\\text{acknowledged}\\land(N=3)\\land\\text{reset request}.
$$

Reset copies present SP, PV, and actual valve feedback into controller state.
With actual feedback at 30%, the reset scan therefore holds 30%. It calls
Configure and Initialize but not PID Update. At zero error, the separate later
Auto scan starts from the retained 30% and also predicts 30%.

## Implementation

The station guard owns validity and authority. The same single `Control.PID`
from previous lessons is called only on a valid, authorized scan:

```st
IF FaultLatched THEN
    ValveCommand := LREAL#30.0;
ELSIF ControllerReady AND EnableAutoRequest THEN
    ValveCommand := Controller.Update(Input := Input);
END_IF;
```

Qualified reset configures the last accepted active record and calls
`Controller.Initialize` with current SP, PV, and actual valve output. It does
not call `Update` or grant Auto on that scan.

## Experiment in Control Studio

Control Studio executes one complete invalid-configuration and recovery
sequence. Test writes one reviewed Boolean authority,
\(\mathtt{P1.ExperimentEnabled}\), for 10.1 seconds. The project initializes
revision 0 at 6 bar and 40%, applies the invalid 80%/20% candidate, corrects the
candidate without clearing the latch, acknowledges, supplies three separate
valid scans, resets at actual valve 30%, and requests Auto on the following
scan. Returning authority false disables Auto and commands safe 30%.

Fresh native runs own isolated codes 1 through 4 as well as the code 5
sequence. The UI run must not claim that it injected nonfinite, stale, timing,
and deadline faults. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait for **Simulation**, **Running**, current samples, and good quality. Confirm SP/PV = 6 bar, safe valve command = 30%, Auto unauthorized, controller not ready, fault code/count = 0, and experiment authority false. Add quality, age, cycle time, deadline, candidate limits, revision, fault/latch/acknowledgement, recovery count/readiness, call flags, Auto authority, output, and experiment authority to **Signals**.
   - **Decision before action:** Write the code-5 prediction: no PID/configure call, revision unchanged, and 30% safe output. Write the required recovery count sequence 0→1→2→3, reset hold 30%, and later Auto hold 30%.
   - **Successful evidence:** The plot advances, values are finite/current, quality is good, and **Start recording** is enabled. Select **Start recording**.
   - **Recovery:** For stale, unavailable, nonfinite, or poor-quality baseline data, stop the Simulator, correct the displayed fault, restart, and repeat the baseline check. Do not begin with an unexplained active fault.
   - **Record:** Note the initial validity, state, ownership, output, and predictions.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **10.1**. The reviewed authority is \(\mathtt{P1.ExperimentEnabled}\); the project owns all fault and recovery pulses.
   - **Decision before action:** Confirm the plan writes only that Boolean and restores it false. The injected reversed limits must be rejected before configuration or PID arithmetic. Pressure must remain within 0–12 bar and valve output within 0–100%.
   - **Successful evidence:** Select **Review test**, verify authority, timeout, stop conditions, and false restoration, then **Confirm and start**. Keep **Abort test** available. Observe code 5 latch, safe 30%, acknowledgement without clear, counts 1/2/3 on separate scans, reset without Auto, and later renewed Auto.
   - **Recovery:** Abort if an invalid scan calls PID/configure, changes revision, leaves 30%, clears on acknowledgement, counts the acknowledgement scan, or grants Auto during reset. Confirm authority false and safe 30% before retrying.
   - **Record:** Retain terminal state, restoration, every sequence scan, fault code/latch, call flags, revision, acknowledgement, recovery count, reset, Auto authority, output step, and seed.

There is no separate disabled Validate stage. Native tests own the isolated
fault-code matrix; Monitor and Test expose the complete code-5 recovery path.

## Evidence and decision

### Measures and comparison

**Diagnostic correctness** is exact equality between the injected fault class
and published code on the first fault scan.

**Invalid-scan state mutation** counts any PID call, Configure call, or active
revision change on the invalid scan. Its required value is zero.

The **safe-output error** is
\(e_{\mathrm{safe}}=|MV_{\mathrm{applied}}-30\%|\) while latched. The absolute
value measures deviation in either direction; the required maximum is 0.001
percentage points.

**Recovery qualification** requires exactly three consecutive valid scans.
**Reset acceptance** requires acknowledgement, qualification, and reset request
together.

The **recovery output step** is
\(\Delta MV_{\mathrm{recovery}}=
|MV_{\mathrm{Auto,first}}-MV_{\mathrm{reset}}|\). Comparing adjacent authority
states isolates the handover bump from later feedback movement.

| Injection | Expected code | PID/configure calls | Applied output |
| --- | ---: | ---: | ---: |
| 13 bar pressure | 1 | 0 / 0 | 30.000% |
| 0.6 s age | 2 | 0 / 0 | 30.000% |
| 0 s cycle time | 3 | 0 / 0 | 30.000% |
| Missed deadline | 4 | 0 / 0 | 30.000% |
| Reversed limits | 5 | 0 / 0 | 30.000% |

![Recovery authority advances in separate qualified scans](figures/invalid-data-recovery.svg)

[Retained run](figures/invalid-data-recovery-run.csv) ·
[Plot data](figures/invalid-data-recovery-trace.csv) ·
[Decision summary](figures/invalid-data-recovery-summary.csv)

### Validation

Require the exact code for every isolated injection, zero invalid-scan state
mutations, unchanged revision 0, and safe-output error at or below 0.001
percentage points. Acknowledgement alone must not clear the latch. Readiness
must require three subsequent consecutive valid scans. The acknowledgement scan
must leave the recovery count at zero; valid scans 1, 2, and 3 must advance it
exactly once each. Reset must call configuration and initialization without a
PID update and must leave Auto unauthorized. The later renewed-Auto scan must
call the PID update and introduce at most 0.001 percentage points of output
step.

## Conclusion

Detection, safe action, evidence, acknowledgement, recovery qualification,
state initialization, and renewed authority answer different engineering
questions. Combining them lets a momentarily good value or a button press
silently restore control.

The controller remains deterministic because invalid values never enter its
arithmetic and recovery rebuilds its state from current physical evidence.

## Reason for the next lesson

Use this guard pattern around any controller-facing workflow, with a safe state
reviewed for that equipment. Lesson 31 next uses valid evidence to select the
minimum sufficient controller rather than assuming every plant needs PID.
