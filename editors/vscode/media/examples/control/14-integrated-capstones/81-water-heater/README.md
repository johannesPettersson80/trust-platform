# 29. Heater commissioning

You will commission the electric heater using the model and controller you developed. You will write the complete ST integration, assess setpoint and disturbance response, and test startup, modes, limits and measurement-fault recovery.

## The problem

We have developed the heater's tuning and operating-state rules separately.
That does not establish that the combined controller meets both temperature
tests or handles startup, transfer and faults without conflicting state or
output decisions. We now test the complete integration on the same heater.

The heater stores energy and loses heat to a 20 °C environment. The assessment uses

$$
50\frac{dT}{dt}=2u(t-1)-10(T-20)-D,
$$

where T is °C, u is applied command in percent, the 1 s delay affects delivered heater power, and D is extra heat extraction in kW. Full power is 200 kW, so one command point is 2 kW.

At equilibrium, $0=2u-10(T-20)-D$, therefore

$$
T_{eq}=20+0.2u-\frac{D}{10}=17+0.2u\quad\text{when }D=30\text{ kW}.
$$

With D=0, 35 °C requires 75% and 38 °C requires 90%. With D=30 kW at SP=35 °C, 35 °C also requires 90%. Both cases are feasible and both are mandatory assessment tests. At 100% with D=30 kW, the maximum equilibrium is 37 °C. A 45 °C target would require 140% under that load and is only a paper capacity question, not a required live test.

## What we need

We need to establish that the complete heater controller meets temperature
requirements and handles startup, mode changes, limits and measurement faults
consistently. Correct individual calculations do not establish the integration.

## The new idea

**Heater commissioning** checks whether the complete controller meets its
temperature and operating-state requirements. Individual working features
are not enough; their integration must use consistent state and one clear
owner of applied output. This is a simulation assessment, not physical
equipment approval.

Use your validated model and selected gains rather than the introductory
$K_p=2$, $K_i=0.5$ values. Retain weighted P, filtered PV, PI, applied-output
tracking and the one-second delivery delay. Expose mode and ownership,
setpoint/PV/error, P/I, requested/applied power, revision, measurement quality
and age, fault latch, recovery state and enable permission.

Lifecycle behavior is part of the controller: initialize the filter from fresh PV and set I=applied−P using the current weighted proportional term, track manual output while inactive, transfer without a command jump, apply a complete parameter record atomically, reject bad/stale PV before state updates, latch the fault, require fresh samples, and require deliberate re-enable. Use the current Lessons 25–28 contracts and record which values and thresholds are relevant to your integration.

## Prediction

Before testing, calculate 90% nominal holding output for both feasible tests and 37 °C maximum equilibrium for D=30 kW at 100%. Predict which test stresses peak and which stresses minimum. Predict the safe output and ownership during invalid PV, the integral state after initialization at 35 °C/75%, and the evidence needed to prove a bumpless transfer and atomic revision.

## Implementation

Write a coherent controller integration from the PI calculation and stateful controller interface you developed. Select and justify gains, weighting beta, filter setting, delay/history initialization, limits, and measurement-validity thresholds from the retained model and evidence. Use one persistent controller instance and one output selector, as in the initialization lesson. Your record must map each required behavior to a named state, parameter, signal, and test observation.

## Experiment in Control Studio

Write a commissioning record before opening results. It must state the learner’s validated model and uncertainty, selected Lesson 21 gains, 35 °C operating point, weighted-setpoint/filter settings, limits, delay history, and lifecycle behavior. Requirements are:

| Requirement | Evidence through 60 s |
| --- | --- |
| Setpoint test | SP 35→38 °C; peak ≤39 °C; $\lvert SP-PV\rvert\le0.2$ °C by 30 s and thereafter |
| Load test | SP=35 °C, D=30 kW; minimum PV ≥33 °C; $\lvert SP-PV\rvert\le0.2$ °C by 30 s and thereafter |
| Output | Applied command 0–100%; requested/applied distinction retained |
| Integration | Initialization, manual transfer, atomic parameter revision, invalid/stale measurement fault, deliberate recovery and separate re-enable |

The nominal 90% holding output is a calculation, not measured qualification. A candidate may fail or remain unqualified.

1. Before **Confirm and start**, enter the chosen model revision, gains, beta/filter settings, limits, validity thresholds, and requirements. Reset to SP=PV=35 °C, applied power 75%, I=75%, P=0, and delayed history 75%.
2. Run the separate 60 s SP 35→38 °C test from time zero. Display all requirement and lifecycle signals; record peak and error window.
3. Restore filtered PV to 35 °C and D to zero as part of the full hot reset, cancel any previous load, and run the separate 60 s D=30 kW test with SP fixed at 35 °C. Record minimum and error window. Do not combine events.
4. Run independent lifecycle checks for initialization, manual transfer, one complete parameter revision, and invalid/stale measurement fault and recovery. Use known simulation truth when the learner’s own sensor is intentionally invalid; keep fault injection separate from unexpected physical-bound violations.
5. Stop on actual simulated water temperature outside 0–80 °C, applied output outside 0–100%, invalid timing/quality outside the planned fault case, or unresponsive signals. At every end or abort, zero heat, cancel D, clear/recover state as specified, and restart before repeating. Select **Abort test** for an early stop and stop the simulator if its output monitor is unresponsive.

## Evidence and decision

Mark every row pass, fail, or untested with trace timestamps and units. Assess both thermal tests independently, then assess output limits, initialization, transfer continuity, parameter revision atomicity, fault latch, fresh-sample recovery, and separate re-enable. Keep requested and applied power distinct and report model prediction versus measured response.

A smooth trace does not qualify an untested lifecycle behavior. A nominal 90% calculation does not qualify installed capacity. If a requirement fails, report the failed evidence and whether the cause is tuning, capacity, model uncertainty, actuator behavior, measurement validity, or incomplete integration. “Not qualified” is a valid commissioning result.

## Conclusion

Independent commissioning defends a complete heater integration from physical feasibility through lifecycle ownership and measurement recovery. Both feasible thermal tests and all required state transitions must be evidenced before qualification.

## Reason for the next lesson

This assessment controls an electric heater whose limited command can be applied directly. A different heating installation uses a motor-driven hot-water valve. Its requested position can change immediately in software, but the stem takes time to move, so a command inside 0–100% does not mean the requested position has been reached.

[Actuator tracking](../../01-feedback-foundations-and-temperature-control/08-heating-valve-rate-tracking/README.md) follows that moving actuator and uses measured position to keep the PI state consistent with what it can deliver.
