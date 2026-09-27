# 27. Parameter updates

You will apply PI gains, limits and filter settings as one validated parameter set. You will adjust controller state to avoid unintended changes in heater power when the parameters change.

## The problem

A controller revision can change proportional gain, integral gain, setpoint weight, filter time, limits and timing limits. Updating fields independently can run one scan with a new gain and an old filter or limit. Even a valid change alters the current proportional contribution when error is nonzero, so the stored integral must be adjusted from the actual old and new calculations.

## What we need

We need to change controller settings as one coherent set, without running
a mixture of old and new values. We must also preserve the intended output
when those settings change the current calculation.

## The new idea

An **atomic parameter update** adopts one complete validated set at a single
update boundary. The controller never uses an accidental mixture of old and
new settings. We must also adjust retained state when the new settings change
the current calculation, so publication itself does not introduce a power jump.

The current weighted proportional contribution and the new contribution are calculated from the same snapshot of SP, raw measurement and retained filter state. For a gain or setpoint-weight change with the same measurement selection:

$$
P_{\mathrm{old}}=K_{p,\mathrm{old}}\left[\beta_{\mathrm{old}}(SP-35)-(PV_f-35)\right],
$$

$$
P_{\mathrm{new}}=K_{p,\mathrm{new}}\left[\beta_{\mathrm{new}}(SP-35)-(PV_f-35)\right].
$$

If the old request is $u=P_{\mathrm{old}}+I_{\mathrm{old}}$, preserve it by setting

$$
I_{\mathrm{new}}=I_{\mathrm{old}}+P_{\mathrm{old}}-P_{\mathrm{new}}.
$$

Then $P_{\mathrm{new}}+I_{\mathrm{new}}=P_{\mathrm{old}}+I_{\mathrm{old}}$. For $SP=35$, $PV_f=33$, equal $\beta$, $K_p$ changing from 2 to 3 gives $P_{\mathrm{old}}=4$, $P_{\mathrm{new}}=6$. If $I_{\mathrm{old}}=71$, use $I_{\mathrm{new}}=69$; both requests are 75%. If filter time changes, preserve the filter state at the boundary and use the new time in subsequent updates. If switching to raw-PV bypass, calculate Pnew with the current raw measurement. The general integral adjustment still uses the difference between the actual old and new P contributions.

Validate the complete candidate before publication: gains and gap positive, $0\le\beta\le1$, $T_{\mathrm{filter}}\ge0$, $0\le minimum\le maximum\le100$, and finite values. A zero filter is the explicit raw-PV bypass from Lesson 23, not an invalid negative time. Publication copies the whole record and revision together. The parameter adjustment must not perform a second ordinary integral update in the same scan.

## Prediction

Use the nonzero-error example above to calculate $P_{\mathrm{old}}=4$, $P_{\mathrm{new}}=6$, $I_{\mathrm{new}}=69$, and unchanged request 75%. Predict that a candidate with minimum 80% and maximum 20% is rejected without changing active parameters or retained integral state. Predict that tightening limits can require an applied-output change even when the unlimited request is continuous; in that case track the new limited applied output.

## Implementation

The record must contain $K_p$ in percentage points/°C, $K_i$ in percentage points/(°C·s), $\beta$ from 0 to 1, filter time $T_{\mathrm{filter}}$ where zero means the permitted bypass, minimum and maximum output, maximum accepted gap, and a revision number. A controller task owns one active record and applies the entire candidate at a specified scan boundary. No other task may publish partial fields. Keep the anti-windup tracking time fixed at 1 s for this exercise; it is not part of the parameter changes being compared.

The requirements remain the fixed Lesson 9 cases: 35→38 °C target step, peak ≤39 °C and $|error|\le.2$ °C from 30–60 s; and, in a separate test, a 30 kW added load at 35 °C, minimum ≥33 °C and $|error|\le.2$ °C from 30–60 s. Both require applied command within 0–100%; 90% is their nominal equilibrium prediction, not an additional acceptance condition.

Define the record and terminators explicitly:

```st
TYPE
    HeaterParameters : STRUCT
        KpPctPerC : REAL;
        KiPctPerCs : REAL;
        Beta : REAL;
        FilterTimeS : REAL;
        MinimumPct : REAL;
        MaximumPct : REAL;
        MaximumGapS : REAL;
        Revision : UDINT;
    END_STRUCT;
END_TYPE
```

The controller owns ActiveParameters and a separate candidate. Validate the candidate, calculate old/new $P$ from the same current inputs, adjust $I$, then assign the whole record and revision at one boundary. A `STRUCT` groups named fields into one record. Access a field with a dot, such as `Candidate.KpPctPerC`, and copy the complete validated record with `ActiveParameters := Candidate;`. `UDINT` stores an unsigned integer used here as a revision number. Publish the candidate only after all its fields have been written; the single controller task copies that completed candidate to ActiveParameters. Keep normal Lesson 8 scan order for limits, anti-windup and integration; the parameter-only adjustment is followed by exactly one ordinary PI update for the actual control interval. It neither adds a second integral increment nor suppresses normal integration. This also permits scheduled parameters to change on successive scans without freezing I.

## Experiment in Control Studio

1. Start at PV 35 °C, SP 35 °C, applied/requested power 75%, filtered PV 35 °C, integral 75%, and a valid active record. Display all record fields, old/new $P$, $I$, request, applied output and revision.
2. In **Control Studio → Test**, review the 60 s sequence. **Confirm and start** establishes time zero and steps SP to 38 °C. At 2 s submit a valid complete revision with changed $K_p$, $K_i$, $\beta$ or filter time. Compare the boundary request with the normal PI increment that would occur without the publication correction.
3. At 10 s submit the invalid 80%/20% limit candidate. Confirm rejection, unchanged active revision, unchanged candidate-independent state, and continued ordinary control using the active record.
4. Repeat the accepted revision and both fixed 35→38 °C and 30 kW cases from separate full resets. Use the newly accepted gains throughout each follow-up run and compare with the original Lesson 21 candidate in separately reset runs. Keep every temperature requirement unchanged.
5. Set the preceding 1 s command history to 75% at each hot reset. Stop for PV outside 0–80 °C, applied power outside 0–100%, invalid data or an unresponsive revision/status signal. Cancel target/load events, command zero heat, restore, and fully reset before repeating. Select **Abort test** and confirm zero output for an early stop; stop the simulator if it is unresponsive.

## Evidence and decision

At the valid commit, show the same SP, filtered PV and current inputs used for $P_{\mathrm{old}}$ and $P_{\mathrm{new}}$, then verify $P_{\mathrm{new}}+I_{\mathrm{new}}=P_{\mathrm{old}}+I_{\mathrm{old}}$. Compare requests immediately before and after the parameter-only adjustment using the same measurement snapshot. Then verify exactly one ordinary integral increment for that control interval; the resulting control update may legitimately change the final request. Compare the live boundary with the hypothetical unadjusted request.

For the invalid candidate, require no partial field publication, no revision change and no change to retained $I$ except ordinary valid control updates. Report any limit-tightening jump as a deliberate applied-output consequence. A valid revision still must pass each fixed target/disturbance requirement separately.

## Conclusion

Atomic publication applies one complete, validated parameter record. Preserving output requires comparing the actual weighted proportional contributions before and after the revision and adjusting the stored integral by their difference. Limits, bypass filtering, revision identity and ordinary scan order remain explicit.

## Reason for the next lesson

Coherent parameters cannot make an invalid or old measurement safe. The next lesson rejects bad heater data before it changes state and defines deliberate recovery.

[Invalid and stale data](../42-invalid-stale-data/README.md) adds the fail-closed measurement boundary.
