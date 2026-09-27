# 66. Multivariable design selection

You will choose a control arrangement that must work despite uncertain heat transfer between the two vessels. You will compare paired PI and static decoupling under the same temperature and actuator requirements.

## The problem

The decoupler cancels the nominal steady interaction. Actual coupling can
differ, making its compensation too weak or too strong. We have not yet
established whether fixed paired PI or fixed decoupling better meets the
requirements across those conditions.

The plant uses heat capacities 50 kJ/°C and losses 10 kW/°C, ambient 20 °C and 200 kW heaters. Coupling $H_c$ changes both own and cross gains. A decoupler derived for 8 kW/°C is exact only there; paired PI also changes behavior but does not rely on a fixed inverse.

## What we need

We need to decide which fixed controller arrangement meets the requirements
when the physical coupling differs from its nominal value. Nominal cancellation
alone cannot establish performance across the expected uncertainty.

## The new idea

**Multivariable design selection** compares complete controller arrangements
under the same requirements and justified uncertainty. We keep each design
fixed while changing the physical coupling; retuning each case would answer
a different question.

Let $x$ contain temperature deviations from 35 °C and $v$ the physical
heater-command deviations from 75%. Here $v$ is not the virtual request used
inside the preceding decoupler. The steady balance is

$$
\begin{bmatrix}10+H_c&-H_c\\-H_c&10+H_c\end{bmatrix}x=2v.
$$

Starting from 35/35 °C, a setpoint of 37/35 °C requires 93/67% for Hc=8. For Hc=6, 8 and 10, the corresponding commands are 91/69%, 93/67%, and 95/65%. A D=10 kW load requires 80% in the loaded vessel and 75% in the other at steady state. These are feasibility calculations, not measured results.

## Prediction

Predict why the nominal decoupler loses exact cancellation at 6 and 10, and which transient/error signal will expose that. Predict whether a finite set of 24 cases can guarantee robustness outside those cases (it cannot).

## Implementation

Write a parameter record containing the physical coupling and a separate fixed decoupler coefficient. Calculate the steady command required for each event as a feasibility check; for 37/35 °C the required steady commands are $u_1=75+(10+H_c)$% and $u_2=75-H_c$%. These are equilibrium demands, not the initial PI requests. Keep each architecture and coupling revision explicit. Do not disable a controller merely because Hc changed; it is the declared uncertainty case. Preserve applied/requested values and lifecycle/fault behavior. Do not retune between cases.

## Experiment in Control Studio

Freeze Lesson 64 gains and Lesson 65 map. Use baseline 35/35 °C, 75/75%, dt=.1, $T_t=1$, beta=1 and no filter. Run six target cases (two directions × three Hc values) and six D=10 kW load cases for each architecture: 24 cases total. Requirements remain changed peak ≤37.5 °C, error ≤.2 °C from 30–60 s, unchanged vessel within ±.2 °C, loaded minimum ≥34 °C, and outputs 0–100%.

1. Reset to 35/35 °C, 75/75%, selected frozen gains/map, and Hc=8. Display temperatures, requests, applied outputs, coupling and revision.
2. Run paired PI and decoupling separately for each target direction at Hc=6, 8 and 10. Reset fully between cases.
3. Run D=10 kW on each vessel for each Hc and architecture, also from full reset. Each event starts at **Confirm and start** and lasts 60 s; both setpoints remain 35 °C in the load tests. Keep requirements unchanged.
4. Abort if physical temperature leaves 0–80 °C, applied power leaves 0–100%, data become invalid or signals stop updating. Command zero heat and cancel disturbances, then stop and reset before each comparison.

## Evidence and decision

Create one requirement row per architecture, coupling, direction and load case. Qualify an architecture only if every feasible case passes. If a case is infeasible, identify the capacity calculation separately from controller failure. A passing set establishes performance only at the tested coupling values; it does not prove every value between them.

## Conclusion

Robust multivariable selection compares complete, frozen architectures across declared coupling uncertainty and both target/load directions. Nominal cancellation alone is insufficient.

## Reason for the next lesson

The two-vessel comparison used two actuators and measured both temperatures. Its steady gain map describes settled interaction, but it does not separate internal storage, actuator influence and available measurements for a process with fewer actuators or sensors.

[State-space models](../../02-derivative-action-and-dynamic-response/20-state-equations-from-stores/README.md) extends the familiar gravity tank to two tanks in cascade with one pump. It organizes their storage dynamics explicitly so that we can ask what that pump can control and what a downstream sensor can reveal.
