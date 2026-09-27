# 32. Valve stiction

You will distinguish command deadband from valve stiction, where friction holds the stem until the positioner produces enough force. You will decide which behavior explains the observed movement before changing PI tuning.

## The problem

At fixed total supply 128.125 kPa, line pressure loss changes as flow changes. The valve pressure drop therefore varies normally; that variation is not an unstable supply by itself. A small request may be ignored by an interface, or friction may hold the actual stem until breakaway. Flow oscillation alone cannot distinguish these causes.

## What we need

We need to distinguish an ignored command from a target that was accepted
but could not move the stem because of friction. The observations must separate
those causes before we change the temperature-controller gains.

## The new idea

**Stiction** is static friction that holds the stem still until the driving
force exceeds a breakaway threshold. **Command deadband** instead ignores
small requests before they reach the actuator. The accepted target and actual
position let us distinguish these mechanisms.

For the interface deadband, accept a new target only when

$$
|u_{request}-u_{lastAcceptedTarget}|\ge1\ \text{point}.
$$

Otherwise hold the accepted target; after acceptance, normal stem motion proceeds at 0.5 points/s. This is a command-path rule.

For the stiction illustration, a local valve positioner produces an estimated drive force $F=k_s(u_t-x)$, where $k_s=10$ N per percentage point, ut is the accepted position target and x is actual stem position. This inner actuator mechanism is separate from the vessel-temperature PI controller. Static breakaway is 50 N, so the stem holds while $\lvert u_t-x\rvert\le5$ points. At target 80.5% with stem 75%, $F=55$ N and release occurs. During motion, the approximation uses 20 N moving friction; motion stops when the remaining force is at most 20 N, here near 78.5% for a held target of 80.5%. These are supplied illustrative force thresholds. Force and position measurements on an actual valve would be needed to establish its breakaway and moving-friction behavior.

The same target can therefore be accepted while the stem remains still, then begin moving after breakaway. Flow responds after measured stem movement and its 0.5 s lag; heat responds after the separate 1 s delay. Keep accepted target, measured stem, force estimate, q, T and supply pressure observable.

## Prediction

For upward half-point probes 75.5% through 80.5%, predict which requests the deadband accepts, which stiction forces exceed 50 N, and where a released stem stops under the stated approximation. Repeat the reasoning downward from 74.5% through 69.5%. Predict why normal pressure variation must not be diagnosed as supply instability.

## Implementation

Write scalar calculations for request-minus-last-accepted target, `PositionErrorPct := TargetPositionPct - MeasuredStemPct;`, and `DriveForceEstimateN := 10.0 * PositionErrorPct;`. Use `ABS(PositionErrorPct)` for distance regardless of direction; `ABS` returns a nonnegative magnitude. Check 5.0 and 5.5 points of mismatch against the strict greater-than-5 breakaway rule. Treat estimated force as a model calculation, not a force-sensor reading. Preserve actual stem and accepted target separately. Keep the deadband and stiction cases as separate plant variants; do not apply both defects simultaneously. As an additional comparison, observe PI using unchanged Lesson 30 gains, and record any integral growth without retuning.

## Experiment in Control Studio

Start at T=35 °C, target/stem 75%, q=1.5 kg/s, and supply pressure 128.125±0.5 kPa. Use 0.5 percentage points/s stem rate, 0.5 s flow lag and 1 s heat-delivery delay. Declare matched upward and downward probes, 15 s holds, bounds T 33–37 °C, position 0–100%, q 0–2.1, and the evidence required before the clock starts.

1. Reset the fixed-total line to T=35 °C, stem/target 75%, q=1.5 kg/s, supply pressure 128.125±0.5 kPa, flow and delay histories, and PI off for manual probes.
2. At **Confirm and start**, run upward targets 75.5, 76.0, …, 80.5%, holding each 15 s, for a 165 s record. A 3.5-point release movement takes 7 s at 0.5 point/s, leaving time to observe flow after its 0.5 s lag. Restore to 75%, reset fully, then run downward 74.5, 74.0, …, 69.5% with identical timing.
3. Run the deadband and stiction variants in separate records. Display request, accepted target, measured stem, estimated force, q, T, supply pressure, and validity.
4. In **Control Studio → Monitor → Response**, compare the accepted target with actual stem. Interface deadband requires the acceptance rule; stiction requires the target to change while the stem holds, followed by motion after the force threshold. Stop on T outside 33–37 °C, target/stem outside 0–100%, q outside 0–2.1 kg/s, total supply outside 128.125±0.5 kPa, invalid signals or normal completion at 165 s. Issue target 0% to close slowly, then stop the simulator if unresponsive; fully reset before repeating.

## Evidence and decision

Deadband evidence is an ignored request change followed by ordinary rate-limited motion after the acceptance threshold. Stiction evidence is request/target accumulation with stationary stem, force crossing breakaway, release, and subsequent flow response. Pressure must remain within its declared band; otherwise repeat before diagnosing the actuator.

Flow oscillation without position evidence is unresolved and may result from tuning, load, pressure, lag or command handling. Report upward and downward evidence separately. Do not call a normal pressure-dependent flow change a valve fault.

## Conclusion

Deadband is an interface acceptance rule; stiction is a physical hold-and-release mechanism. Matched resets, force/position evidence, flow lag and stable line pressure separate them before any gain change.

## Reason for the next lesson

A valve fault must be diagnosed and addressed as an actuator problem. The investigation has also treated stem position as an observed result: we have not yet derived how a healthy motor produces and controls a requested position.

[Position control](../../02-derivative-action-and-dynamic-response/32-proportional-position-stiffness/README.md) uses a simple rigid shaft to explain torque, inertia and position feedback. It isolates healthy motion; it does not claim that proportional control repairs valve stiction.
