# 26. Bumpless transfer

You will keep the PI integral aligned with manual heater power and switch to automatic control without an output jump.

## The problem

Lesson 25 initialized the controller from a 35 °C/75% state. Manual operation can then change applied power while automatic control is inactive. If the integral state remains at 75%, automatic control returns with a stale request. A bumpless transfer means continuity of the command at the mode boundary; it does not freeze the water temperature or make an infeasible manual output sustainable.

## What we need

We need to return from manual to automatic control without an unintended
command jump. The inactive controller must be ready to continue from the
output actually applied, even if manual operation changed it.

## The new idea

**Bumpless transfer** changes control ownership without an unintended jump
in the actuator command. We keep the inactive controller's state aligned with
actual manual output, then resume feedback from that command. Necessary
corrections on later updates are separate from a jump caused by changing mode.

Let $P$ be the current weighted proportional contribution using the filtered PV, $I$ the stored integral contribution, and $u_a$ the applied manual output. While Manual owns the actuator, track

$$
I=u_a-P.
$$

At 35 °C with 75% applied power, $P=0$, so tracking gives $I=75$. If the current weighted $P=4$ points and manual output is 60%, tracking gives $I=56$, so $P+I=60$. If the tracking update is omitted and stale $I=75$ remains, the same $P=4$ would request 79%, a 19-point jump. The illustrative $K_p=2$ value may be used for this arithmetic only; the live controller retains the selected gains.

On the first Automatic scan, calculate P from the current filtered sample and assign I=ua−P again, then reconstruct $u=P+I=u_a$, apply limits and tracking as required, and do not add the normal $K_i e\Delta t$ increment on that scan. On following Automatic scans, use the ordinary selected PI order: calculate the trial integral, form the complete request, limit it, and finally correct the retained integral from applied minus requested output, exactly as in Lesson 8.

The heater's one-second delivery delay means applied heat may still change after a command boundary. A manual 75→60% change held long enough can cool the water toward a lower equilibrium; a live transfer therefore must not assume a 15-point temperature-independent jump. The 15-point and 19-point values are isolated command calculations.

## Prediction

For an isolated state at 35 °C with $I=75$, $P=0$, and manual output 60%, predict tracked $I=60$ and first automatic request 60%. Without tracking, predict a 15-point request jump. With current $P=4$, predict tracked $I=56$, request 60%, and stale-state request 79%, a 19-point jump. Predict that PV may have cooled during a manual hold even when command continuity is exact.

## Implementation

Use the same interface and controller instance as Lesson 25. Continue using the Lesson 22 weighted setpoint path and Lesson 23 filtered PV, plus the selected Lesson 21 gains, integral state, limits and tracking. While Manual owns the output, automatic calculation must not write it. The controller must track the applied manual output, select Automatic on an explicit event, and skip the ordinary integral increment on the transfer scan.

The boundary requirement is command continuity: the requested automatic output immediately after transfer must differ from the applied manual output immediately before transfer by no more than 0.1 percentage point, with one output writer and applied power within 0–100%. Temperature recovery is assessed separately.

Use the mode and output selector from Lesson 25. In Manual, calculate the current weighted $P$ from filtered PV and set $I:=AppliedPowerPct-P$; select applied manual output. A rising edge means `AutomaticRequest` is true now and was false on the previous scan. Retain that previous Boolean value. When a rising edge is accepted while Ready, copy the current applied output into the transfer boundary, recalculate P from the current filtered sample, assign I=applied−P and calculate the first automatic request $P+I$, and skip ordinary integration once. On later Automatic scans, use the normal PI calculation, limits and anti-windup. Set a Boolean `EnteringAutomatic` for this scan, then clear it on the following scan. Set the control-time baseline to the transfer time; no manual interval is integrated.

Keep requested, applied and delayed delivered power separate. Expose mode, transfer flag, $P$, $I$, request and applied output. Do not create a second controller instance or a second output writer for the comparison.

## Experiment in Control Studio

1. Start at PV 35 °C, setpoint 35 °C, applied/manual power 75%, integral 75%, filtered PV 35 °C and delay history 75%. Display mode, ownership, filtered PV, weighted $P$, $I$, request and applied power.
2. In **Control Studio → Test**, review the 35 s sequence. **Confirm and start** establishes time zero and changes the manual command from 75% to 60%. Hold it for 5 s; at 5 s request Automatic, continue observing to 35 s. Keep setpoint, load and selected gains unchanged.
3. At the transfer boundary compare the last Manual applied value with the first Automatic request using the fresh $P$, filter state, $I$ and timestamp. Check the 0.1-point continuity requirement and confirm only one writer is active.
4. Repeat after a full reset with an isolated untracked-state counterexample, not as a combined live run. Report the command jump separately from the temperature trajectory; manual cooling and the one-second delivery delay remain physical.
5. Stop if PV leaves 0–80 °C, applied command leaves 0–100%, data/timestamps are invalid, or signals stop updating. Cancel automatic control, command zero heat, wait for restoration, clear transfer/history state and fully reset before repeating. Select **Abort test** to stop early, and stop the simulator if it is unresponsive.

## Evidence and decision

The tracked run qualifies the transfer only if the boundary request step is ≤0.1 point, applied output stays within limits, and ownership changes once. Check that the transfer scan skipped ordinary integration and that the next scan resumed the selected PI update. A smooth request does not prove that PV remained constant or that later target requirements are met.

Use the isolated arithmetic to explain any untracked counterexample: 75% stale state versus 60% manual output gives a 15-point jump at $P=0$, while $P=4$ gives a 19-point jump. The live run's temperature may differ because the heater dynamics continue during the manual hold. If manual output is infeasible or already limited, tracking preserves continuity but cannot create unavailable heat.

## Conclusion

Bumpless transfer is output continuity at an explicit mode boundary. Tracking $I=u_a-P$ with the current weighted and filtered proportional contribution aligns the controller with the command currently applied, while a one-scan transfer flag prevents an extra integral increment. It does not remove thermal dynamics, delay or output feasibility limits.

## Reason for the next lesson

The same state-continuity requirement applies when gains, limits or filter settings are revised. A complete parameter record must be validated and applied together.

[Atomic parameter update](../41-atomic-parameter-update/README.md) preserves output while committing a coherent revision.
