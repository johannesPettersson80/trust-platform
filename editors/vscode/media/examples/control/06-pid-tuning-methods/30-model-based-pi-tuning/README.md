# 19. Model-based PI tuning

You will calculate starting PI gains from your validated heater model and a chosen response time. You will explain the tuning relationship and test the controller against the same setpoint and disturbance requirements.

## The problem

When your model meets the independent-validation requirements, it can support a controller calculation for a chosen response time. Manual PI tuning in Lesson 11 gives useful practice with gain changes, output limits, and response requirements. It does not provide a general calculation for how much gain to choose. Lessons 15 and 18 provide a learner model: process gain K in °C per command percentage point, time constant τ in seconds, and a measured delay L. We can use those quantities to choose a desired response time, and calculate gains under stated model assumptions.

The controller must also start from the common hot state. The heater is already holding 35 °C with 75% output. If the integral state were reset to zero while the proportional term were also zero, the experiment would quietly remove the holding power and change the plant before the setpoint test began.

## What we need

We need a way to calculate starting PI gains from the validated model and
the response we want, then judge whether those settings remain suitable
with the actual delay, uncertainty and output limits.

## The new idea

**Model-based PI tuning** calculates proportional and integral settings
from a process model and a chosen response requirement. We use the model
to obtain starting gains, then test them with this heater's actual limits.
The desired response time is a design choice, not a measured plant parameter.

This calculation requires a validated first-order approximation. If an additional lag is essential, retain the full model for response comparisons and validate any proposed reduction before applying these formulas. If no first-order reduction meets the prediction requirements, retain your manual candidate for comparison rather than discarding the extra dynamics.

Start with your local first-order model around the hot operating point. A dot denotes a derivative with respect to time:

$$
\tau\dot y+y=Kv.
$$

Here y=T−35 °C is temperature deviation from the 35 °C operating point, v=u−75 percentage points is command deviation from the 75% holding command, K is °C/point, and τ is seconds. The validated model may have a delay L; temporarily omit that delay only for this algebra. The delay is restored as a feasibility risk during testing.

Let r=SP−35 °C be the desired temperature deviation and e=r−y be control error. Choose the desired ideal response:

$$
\dot y=e/\lambda.
$$

This means error would decay with the selected time scale λ if the model and actuator were ideal. λ is a design choice, not a measured plant constant. A shorter λ asks for a faster response and therefore produces larger gains; a longer λ reduces gains and gives more margin for delay and model uncertainty.

Substitute the desired derivative into the plant. From τy'+y=Kv:

$$
v=\frac{\tau\dot y+y}{K}.
$$

Use y'=e/λ:

$$
v=\frac{\tau e/\lambda+y}{K}.
$$

Now differentiate this expression. K, τ, and λ are fixed parameters during the trial:

$$
\dot v=\frac{\tau}{K\lambda}\dot e+\frac{\dot y}{K}.
$$

Replace y' with e/λ:

$$
\dot v=\frac{\tau}{K\lambda}\dot e+\frac{e}{K\lambda}.
$$

The implemented PI command uses an integral state I that already includes the 75% holding command:

$$
u=K_p e+I,\qquad\dot I=K_i e.
$$

Because the baseline 75% is constant, u'=v'. Differentiating the PI expression gives:

$$
\dot u=K_p\dot e+K_i e.
$$

Match the coefficient of e' and the coefficient of e with the derived v' expression:

$$
K_p=\frac{\tau}{K\lambda},\qquad K_i=\frac{1}{K\lambda}.
$$

This method is called coefficient matching. Kp has units (s)/(°C/point × s)=percentage points/°C. Ki has units 1/(°C/point × s)=percentage point/(°C·s). The derivation assumes a local first-order model, fixed K and τ, no delay during the algebra, no output saturation, and an integral state that represents the baseline command.

The learner's delay L is not canceled by this rule. During L, the controller may continue integrating before the plant responds. Model uncertainty and output limits can therefore produce overshoot or saturation even when the ideal equation predicts decay. Lesson 8 anti-windup behavior remains required after the trial output is limited: update the integral consistently with the applied output and declared saturation policy.

For the reproducible nominal example, K=.2 °C/point, τ=5 s, and λ=5 s give Kp=5 percentage points/°C and Ki=1 percentage point/(°C·s). With the common hot reset, SP changes from 35 to 38, e=3 °C, I starts at 75%, and dt=.1 s. The first update is:

$$
u=5(3)+[75+1(3)(0.1)]=90.3\%.
$$

For λ=10 s, Kp=2.5 and Ki=.5; the first request is 75+2.5(3)+.5(3)(.1)=82.65%. These are nominal calculations only. The student candidate must use the learner's validated K, τ, and uncertainty.

## Prediction

Before opening Control Studio:

1. Write your learner K, τ, L, uncertainty, and chosen λ with units.
2. Derive Kp and Ki using the coefficient matching above.
3. Predict the first request for SP35→38 from PV35, I75, P0, and dt=.1.
4. Predict how doubling λ changes Kp and Ki. Both should be halved.
5. Predict how the 1 s delay and model uncertainty could change the measured response.
6. State what evidence would make you keep, slow, or reject the candidate after the exact setpoint and load trials.

The manual gain-change bounds in Lesson 11 are practice choices, not global feasibility limits. The model-based candidate still obeys 0–100% output and must meet the declared requirements.

## Implementation

With the simulator stopped, set `ProcessGainCPerPoint`, `TimeConstantS`, `DelayS`, `DesiredResponseS`, `ModelDataValid` and `ModelRevision` in `src/Main.st` from the learner model selected in the validated record. Use `IF` comparisons to require positive K, τ and λ, nonnegative L, and valid model data before dividing. Store Kp and Ki with explicit units and revision identifiers; the actual `Control.PID` owns the controller update and retained state.

Initialize `Control.PID` once at the common hot reset with SP=PV=35, nominal and saturated output 75%, zero rates and zero feedforward. Do not add a separate 75% base to a controller whose retained nominal output already contains 75%. Configure the complete parameter record, call `Update` every 0.1 s with the measured probe input, and use the block's configured 0–100% limits and 1 s back-calculation. Publish requested and applied output, P, I, model revision, λ, Kp, Ki, and validity from the actual controller state; do not duplicate the PI or anti-windup equations in `Main.st`.

Write `KpPctPerC := TimeConstantS / (ProcessGainCPerPoint * DesiredResponseS);` and `KiPctPerCs := 1.0 / (ProcessGainCPerPoint * DesiredResponseS);` inside those checks. Test the two nominal λ choices and invalid zero gain or response time. The native plant owns the one-second delayed heater prehistory; keep the controller cycle time at 0.1 s and inspect retained PID state through `GetState()`.

The prepared nominal illustration declares K=.2, τ=5 s, L=1 s and λ=5 s, giving Kp=5 and Ki=1. Revision 0 means provided nominal data, not a validated learner model. With the simulator stopped, set the project-owned `Main.st` fields `ProcessGainCPerPoint`, `TimeConstantS`, `DelayS`, `DesiredResponseS`, `ModelDataValid` and `ModelRevision` to the learner record. `TrialKind` in `src/Main.st` selects 0 for the 35→38 °C target step or 1 for 30 kW extraction at SP35. Restart the simulator for each 60 s trial. The native state-space plant records water and the retained 2 s probe state separately; `Control.PID` receives the measured probe input through normal PLC I/O.

Set `SafetyAbort` to stop heating immediately; its latch also sets on invalid
measurement. An abort commands zero heating and zero load. Normal
completion restores 75% heating. After an abort, keep stopped and restart from
the full hot reset before clearing the latch and collecting a new trial.

## Experiment in Control Studio

Choose a desired response time λ using the learner's validated speed, delay, uncertainty, and required settling. Derive Kp and Ki with units, initialize the integral state so it already contains the 75% holding command, and test the resulting candidate under:

- SP 35→38 °C, 60 s, peak ≤39 °C, and absolute error ≤.2 °C from 30 through 60 s;
- separate SP 35 °C with D=30 kW, 60 s, minimum PV ≥33 °C, and absolute error ≤.2 °C from 30 through 60 s;
- applied command within 0–100%, PV within the exercise range 0–80 °C, valid data, and dt=.1 s.

The setpoint and load trials are separate. Combining SP38 with D30 is not the required test.

1. Stop and restart Simulator. Confirm the common hot reset: PV=35 °C, SP=35 °C, applied command=75%, integral state=75%, proportional correction=0%, native delayed-heater prehistory=75%, D=0, valid data, and dt=.1 s.
2. With the simulator stopped, set `ProcessGainCPerPoint`, `TimeConstantS`, `DelayS`, `DesiredResponseS`, `ModelDataValid` and `ModelRevision` in the project-owned `Main.st`. Record K, τ, L, uncertainty, λ, Kp, Ki, and the predicted first request before enabling automatic control. Restart the simulator; the project `Main.st` executes the actual `Control.PID`, and `GetState()` exposes its retained controller state. In **Control Studio → Test**, use the existing **Confirm and start** route after choosing `TrialKind` in the source and restarting.
3. Declare both required trials before viewing results. In **Control Studio → Test**, review and select **Confirm and start** to establish time zero for the setpoint trial: SP35→38 °C, D=0, 60 s. Require peak≤39 °C, |error|≤.2 °C for every sample from 30 through 60 s, applied output 0–100%, PV 0–80 °C, and valid data.
4. Reset fully to PV35,SP35,I75,P0,delayed history75, then run the load trial separately: SP35 fixed, apply D=30 kW, 60 s. Require minimum PV≥33 °C, |error|≤.2 °C from 30 through 60 s, the same output/PV/data limits, and the same dt.
5. Normal saturation inside 0–100% is evidence to inspect, not an automatic failure. Stop only for PV outside 0–80 °C, applied command outside 0–100%, invalid/stale data, stopped time, or an explicit hard fault. Select **Abort test**, cancel D and command zero heat if a stopping condition occurs; stop the simulator if it is unresponsive. Preserve the observed saturation duration and anti-windup behavior.
6. At normal completion, cancel D if active and leave the controller in its manual 75% holding state with the load at zero. Before any repeat, restart Simulator and confirm the complete hot reset again.

## Evidence and decision

The derivation is valid for the stated local model and ideal unsaturated algebra. The experiment decides whether the learner's candidate meets the exact requirements with its measured delay, uncertainty, and limits. Report first request, peak, error windows, minimum under load, saturation, applied output, model revision, λ, and reset evidence.

Keep the candidate when both trials pass and the record supports the model assumptions. Choose a slower λ when the candidate is too aggressive or delay uncertainty dominates. Reject or redesign when requirements fail, data are invalid, or the learner model is not validated. Do not claim that nominal K=.2 and τ=5 apply to every learner.

## Conclusion

Starting from τy'+y=Kv and the desired decay y'=e/λ gives v' = τe'/(Kλ)+e/(Kλ). Matching that derivative to PI's u'=Kp e'+Ki e yields Kp=τ/(Kλ) and Ki=1/(Kλ). The integral state contains the 75% holding command, the delay remains a risk, and the exact reset and tests determine whether the design works.

## Reason for the next lesson

The calculated settings depend on the model parameters and the approximation used in the derivation. We have not yet obtained a candidate directly from a controlled oscillation, which can provide a different experimental starting point for the same heater.

[Relay tuning](../29-relay-tuning/README.md) obtains that candidate from measured cycle amplitude and period. It will still need the same setpoint and disturbance tests.
