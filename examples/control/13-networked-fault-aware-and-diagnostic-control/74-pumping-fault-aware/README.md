# Isolate multirate pumping faults and recover deliberately

Course position: Lesson 74 of 84. Prerequisite: bounded network ownership at the remote pumping station. Next lesson: diagnose a degraded loop before retuning it.

**Lesson objective:** Detect the declared sensor and actuator faults, isolate only supported causes, choose degraded or safe operation, and restore automatic control through explicit qualified authority.

## The problem

The equipment is a high-zone pumping station with redundant pressure, suction level, and drive feedback. It is the station from Lesson 73. Two physical discharge pressure transmitters update every 100 ms. A suction-reservoir level transmitter updates every 500 ms. Independent drive-speed feedback updates every 200 ms. The station task runs every 100 ms.

Pressure is measured in bar, reservoir level in percent full, and requested, applied, and measured pump speed in percent. The drive request is limited to 0 through 100 percent. During restoration it may change by no more than 10 percent per second.

Two equivalent simulated header paths make the comparison honest. The baseline controller always trusts pressure transmitter A. The fault-aware controller may select transmitter A, select transmitter B after justified isolation, or remove automatic authority. The sensor bias, missing updates, low level, and stuck actuator are deterministic Simulation injections. They are not physical transmitter, drive, reservoir, network, or pump evidence.

Lesson 73 established when a remote packet was timely, ordered, and synchronized. A timely packet can still contain a biased sensor value. A healthy command packet also does not prove that the pump drive followed its request.

The station's devices update at different rates. If the controller treats a held 500-ms level sample as though it were newly measured every 100 ms, it hides age exactly when the safety constraint matters. We therefore need explicit multirate handling, redundancy, fault decisions, bounded degraded modes, watchdogs, and recovery authority before trusting the loop.

## What we need

We need the pumping station to distinguish supported sensor and actuator faults, continue only in a justified degraded mode, and enter a safe state when independent evidence is insufficient. Automatic authority must return through an explicit qualified recovery sequence.

## The new idea

### Keep every device on its own timeline

A sample interval is the time between new observations from one device. Multirate sampling means connected devices use different intervals. Here pressure is sampled every station task, level every fifth task, and drive feedback every second task.

Resampling places those observations on the controller task timeline. A zero-order hold retains the latest qualified value between device updates. Holding a value does not make it new: its source timestamp and age continue to advance. Linear interpolation is not used to invent a future reservoir-safety observation.

Freshness is the maximum permitted age of retained evidence. The pressure limit is 0.25 s, the level limit is 1.0 s, and the drive-feedback limit is 0.40 s. A watchdog is stateful supervision that detects when an expected update or execution event misses its deadline.

### Test plausibility before using a number

Plausibility means that a value is finite, inside its physical range, and consistent with the declared rate limits. Pressure must remain from 0 through 12 bar, level from 0 through 100 percent, and speed feedback from 0 through 100 percent. A value can pass all those checks and still be biased, so plausibility cannot replace redundancy.

Physical redundancy uses independent devices to measure the same physical quantity. Analytical redundancy compares a measurement with a prediction from a qualified model. Two disagreeing transmitters reveal a fault but cannot identify which one is wrong. The model prediction supplies a third independent relation for this bounded isolation decision.

### Define the pressure residuals

Pressure transmitter A ($p_{A}$) and pressure transmitter B ($p_{B}$) are measured in bar. Predicted pressure ($\hat{p}$) is the model's expected header pressure. Pressure discrepancy ($r_{\mathrm{AB}}$) is the absolute difference between the two transmitters. Model residuals ($r_{A}$) and ($r_{B}$) are each transmitter's absolute difference from the prediction:

$r_{\mathrm{AB}} = |p_{A} - p_{B}|$

$r_{A} = |p_{A} - \hat{p}|$

$r_{B} = |p_{B} - \hat{p}|$

The prediction uses preceding evidence only. The preceding predicted pressure is $p_{\mathrm{hat}}$[k-1], preceding applied speed is $u_{\mathrm{applied}}$[k-1], station interval is T, zero-speed pressure is $p_{0}$, plant gain is $K_{\mathrm{plant}}$, and pressure time constant is $\tau$. The executable Euler update is:

$\hat{p}_{k} = \hat{p}_{k-1} + (T/\tau) \cdot (p_{0} + K_{\mathrm{plant}} \cdot u_{\mathrm{applied},k-1} - \hat{p}_{k-1})$

This expression comes from the same continuous pressure balance used in Lesson 73,
$d\hat{p}/dt=(p_0+K_{\mathrm{plant}}u-\hat{p})/\tau$. Over one short task interval, forward Euler approximates the derivative by
$(\hat{p}_k-\hat{p}_{k-1})/T$. Substituting that approximation and solving for $\hat{p}_k$ produces the update above. Here $T/\tau=0.1/4=0.025$, which explains the literal `0.025` in the program. The predictor is diagnostic redundancy. It is not a hidden Simulation-truth input and does not drive the pressure controller directly.

A pressure discrepancy greater than 0.30 bar for three consecutive new pressure samples confirms disagreement. If exactly one transmitter has a model residual greater than 0.50 bar while the other remains no greater than 0.50 bar, the larger-residual transmitter is isolated. The controller may continue from the other qualified transmitter in degraded single-sensor mode. If the evidence cannot distinguish a sensor, automatic authority is removed.

### Protect a slow reservoir constraint

Retained level can fall between measurements. Conservative available level ($level_{\mathrm{safe}}$) subtracts the greatest credible drawdown since the last sample. The retained level sample ($level_{\mathrm{sample}}$) is percent full, maximum drawdown rate ($v_{\mathrm{level}}$) is 0.5 percentage points per second, and level age ($age_{\mathrm{level}}$) is seconds:

$level_{\mathrm{safe}} = level_{\mathrm{sample}} - v_{\mathrm{level}} \cdot age_{\mathrm{level}}$

The subtraction follows from the worst allowed case: if the reservoir has fallen at its maximum rate for the entire unobserved interval, the true level can be lower than the held sample by at most $v_{\mathrm{level}}age_{\mathrm{level}}$. The low-suction trip is 20 percent. Recovery requires at least 30 percent. The conservative projection does not extend the 1.0-s freshness limit; stale level still removes authority.

### Check whether the drive moved

The applied drive request ($u_{\mathrm{requested}}$) is the command sent after controller limits and restoration rate limiting. Measured drive speed ($u_{\mathrm{feedback}}$) comes from the independent feedback channel. Actuator tracking residual ($r_{u}$) is their absolute difference:

$r_{u} = |u_{\mathrm{requested}} - u_{\mathrm{feedback}}|$

A residual greater than 5 percentage points for three consecutive new drive-feedback samples confirms an actuator tracking fault. Three samples occupy two 200-ms gaps from the first affected sample to the decision, or 0.4 s. The 0.6-s requirement is deliberately conservative because a fault can begin just after one update and wait almost 0.2 s before the first affected sample.

### Separate the fault decisions

Fault detection decides that evidence violated a rule. Fault isolation assigns the fault to a named component only when the evidence distinguishes it. Graceful degradation continues reduced service with one identified healthy pressure transmitter. A safe state removes automatic pump authority when evidence cannot support control.

Acknowledgement records that an operator has seen the fault; it does not clear anything. Cause removal makes the input evidence healthy again. Recovery qualification requires ten consecutive healthy 100-ms scans. Reset approval is a separate explicit authority that clears the latch only after acknowledgement and qualification. Restored pump speed is then limited to 10 percent per second.

## Prediction

At the normal equilibrium, both pressure transmitters and the predictor read approximately 6 bar. Their discrepancy and both model residuals are approximately zero.

Adding 1.0 bar to transmitter A while leaving transmitter B unchanged predicts:

- pressure discrepancy of approximately 1.0 bar;
- pressure-A residual of approximately 1.0 bar; and
- pressure-B residual of approximately 0 bar.

The fault-aware controller should isolate A on the third new pressure sample. With a 100-ms pressure interval, that is 0.2 s elapsed from the first biased sample and no later than the declared 0.30-s limit. The baseline controller should incorrectly reduce pump speed because it interprets the false high pressure as physical pressure.

Predict that loss of both pressure qualities removes authority on the detecting scan. A fresh 19-percent level sample should trip immediately. Missing level updates should trip on the first task after age exceeds 1.0 s. A stuck drive should be confirmed by the third new feedback residual above 5 percentage points. Acknowledgement alone and a premature reset must leave the fault latched. Approved restoration must change applied speed by no more than 1.0 percentage point per 100-ms scan.

## Implementation

The following shortened excerpts come from the Structured Text that actually runs:

```st
PredictedPressure := PredictedPressure + LREAL#0.025
    * (LREAL#4.0 + LREAL#0.04 * PreviousProtectedApplied
        - PredictedPressure);
PressureDiscrepancy := ABS(PressureA - PressureB);
PressureAResidual := ABS(PressureA - PredictedPressure);
PressureBResidual := ABS(PressureB - PredictedPressure);

IF DisagreementCount >= INT#3 THEN
    IF (PressureAResidual > LREAL#0.50)
        AND (PressureBResidual <= LREAL#0.50) THEN
        PressureAIsolated := TRUE;
    END_IF;
END_IF;
```

The slow constraint and actuator supervision are also explicit:

```st
ConservativeLevel := LevelSample - LREAL#0.5 * LevelAge;
ActuatorTrackingResidual := ABS(PreviousProtectedApplied - DriveSpeedFeedback);

RecoveryReady := FaultLatched AND FaultAcknowledged
    AND (RecoveryHealthyCount >= INT#10);
```

Read `src/Main.st` before running. It contains the complete sample retention, ages, plausibility checks, symmetric A/B isolation, deterministic fault priority, mode selection, two real Control.PID instances, zero safe output, and rate-limited recovery.

## Experiment in Control Studio

The visible workflow is exactly **Monitor → Test**. Control Studio supplies authority, while the runnable project supplies the complete 28-second record: healthy multirate sampling, pressure-A bias, qualified recovery, ambiguous pressure, stale level, low level, stuck drive, acknowledgements, approved resets, and rate-limited restoration. That ownership matters because every learner runs the same sequence and cannot accidentally edit a diagnostic input halfway through the comparison. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Header pressure setpoint (SP, bar); Naive pressure transmitter A (PV, bar) and Protected pressure transmitter A (PV, bar); Naive requested pump speed (MV, %) and Naive applied pump speed (MV, %), including units, limits, mode, and quality. The initial operating point is Header pressure setpoint = 6 bar, Naive pressure transmitter A = 6 bar, Protected pressure transmitter A = 6 bar, Protected pressure transmitter B = 6 bar. The unresolved baseline problem is: A timely in-range but biased pressure value silently moves the naive controller because quality and freshness alone cannot establish correctness.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Continue with one isolated healthy pressure transmitter only when the model relation identifies the failed device; otherwise remove automatic authority and require cause removal, acknowledgement, healthy qualification, and approved reset.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **28.1**. The reviewed authority is **Experiment authority** (`P1.ExperimentEnabled`). It does not write a pressure, quality flag, timestamp, acknowledgement, reset, setpoint, or pump command.
   - **Decision before action:** Before selecting **Review test**, predict each mode transition from the schedule above and confirm every pressure and pump-speed limit shown in Monitor.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason and remove experiment authority. The project restores healthy qualities, samples, a 6-bar setpoint, 35-percent level, and false acknowledgement/reset requests before another run.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

Identify, Tune, and Validate are not learner steps here. The fault logic and predictor belong to the runnable project, and the independent native test owns the pass/fail judgment. A disabled generic PID or identification control is not evidence.

## Evidence and decision

### Measures and comparison

Define the decision measures before reading the result:

- **Sensor-isolation latency** is elapsed time from the first biased new pressure sample through confirmed isolation, in seconds. The limit is 0.30 s.
- **Actuator-fault confirmation latency** is elapsed time from the first new feedback sample whose residual exceeds 5 percentage points through the latched actuator fault, in seconds. The limit is 0.60 s.
- **False-trip count** is the number of degraded or safe transitions during the healthy interval. It must be zero.
- **Wrong-sensor selection count** is the number of scans in which an isolated or ineligible pressure value drives the protected controller. It must be zero.
- **Pressure absolute-error area** is the sampled integral of absolute difference from the 6-bar setpoint over a declared fault window, in bar-seconds.
- **Peak pressure departure** is the largest absolute departure from 6 bar in that window, in bar.
- **Safe-output latency** is elapsed time from an ambiguous, stale-critical, low-level, or confirmed actuator decision through zero requested output. It must be no more than one station scan after confirmation.
- **Restoration rate** is the largest consecutive applied-speed change divided by 0.10 s. Its unit is percent per second and its limit is 10 percent per second.
- **Service availability** reports time in normal or justified degraded control separately. It cannot be improved by using ineligible data.

The native executable run measured:

| Result | Measured Simulation evidence |
| --- | ---: |
| Pressure-A isolation time | 5.2 s, the third biased sample |
| False trips before injection | 0 |
| Wrong protected sensor selections | 0 |
| Ambiguous-pressure safe output | Same detecting scan |
| Stale-level safe output | Same detecting scan |
| Fresh low-level safe output | Same detecting scan |
| Stuck-actuator confirmation | 23.0 s, within the 0.6-s confirmation bound |
| Greatest approved-restoration rate | 8.2 percent per second |

Times are positions in the fixed test record. The isolation latency is 0.2 s from the first biased sample, not 5.2 s.

### Validation

Accept this lesson only if the registered native test compiles and executes the shipped Structured Text and shared Control library against the shipped simulation record and independent calculations. It must prove:

- no false healthy transition;
- pressure A isolation on the third biased pressure sample;
- pressure B remains selected throughout the isolated interval;
- no automatic authority when both pressure values are ineligible;
- level age continues to increase while its held value remains unchanged;
- stale and low level each produce zero safe output;
- drive request and feedback remain separate and the stuck actuator is confirmed from three new feedback residuals;
- acknowledgement and premature reset do not clear the fault;
- each reset requires ten healthy scans and explicit approval;
- restored output respects the declared rate; and
- predictor and independent first-order plant calculations match the executable record within tolerance.

Simulation success does not authorize a physical pump. Physical deployment needs reviewed transmitter independence, common-cause analysis, proof-test intervals, drive and motor protection, cavitation protection, hydraulic safe-state analysis, commissioning, and explicit approval.

![Fault mode and protected pump command through isolation, safe degradation, acknowledgement, reset, and recovery](figures/pumping-fault-aware-evidence.svg)

[Complete trace](figures/pumping-fault-aware-evidence-trace.csv) · [Decision summary](figures/pumping-fault-aware-evidence-summary.csv)

## Conclusion

Every measurement first passes through its own sample pulse, source timestamp, retained value, age, quality, and plausibility. Pressure then gains consistency evidence from the second transmitter and model predictor. A single large residual is isolated only after three new samples. The selected healthy pressure drives the protected PID while the fault remains visible as degraded operation.

Two failed or ambiguous pressure paths do not support isolation, so automatic pressure authority is removed. The slow level path remains aged even while held, and its conservative projection cannot outlive the freshness deadline. Requested pump speed is not proof of motion; independent drive feedback closes that evidence gap.

Recovery is deliberately longer than fault detection. Fast shutdown limits exposure, while acknowledgement, cause removal, ten healthy scans, reset approval, and rate-limited return prevent an intermittent or unreviewed recovery from silently regaining authority.

## Reason for the next lesson

Fault handling is an explicitly designed operating mode with named evidence, priority, authority, and recovery. It is not an exception hidden inside the controller equation.

This lesson covers declared deterministic faults and model-consistent residuals in Simulation. It does not cover every failure or prove diagnostic coverage on a physical pump. Lesson 75 introduces diagnosis before changing gains because a commissioned loop can degrade gradually and different causes can produce similar symptoms. Retuning without diagnosing the cause can hide an instrument, actuator, process, interaction, or network problem.
