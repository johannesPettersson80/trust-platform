# 45. Poles and zeros

You will learn what poles and zeros are and how to find them in a transfer function. You will use them to explain the shaft’s decaying oscillation and the different setpoint and disturbance responses left unresolved in the Laplace lesson.

## The problem

The preceding lesson produced the shaft transfer $4/(s^2+s+4)$ and two PI-heater expressions with different remaining factors. The shaft oscillates as its error decays, while the heater's setpoint response appears to lose a thermal factor still present in its disturbance response. We need to connect the algebraic factors to those physical response features.

## What we need

We need to interpret the expressions obtained from the model: which parts
explain decay, growth, oscillation or a different response to another input?
Calculating a response without understanding those features does not yet
guide the next controller decision.

## The new idea

**Poles and zeros** are values of the transform variable that reveal important
features of an input-output response. Poles tell us about its natural decay,
growth or oscillation. Zeros influence how the input excites that response,
including cancellations and competing effects.

Write the transfer as $F(s)=N(s)/Q(s)$, with numerator N and denominator Q.
A polynomial is a sum of powers of s; a **root** makes that polynomial zero.
After cancelling common numerator and denominator factors:

- A **pole** is a root of the remaining denominator.
- A **zero** is a root of the remaining numerator.

These are values of s in inverse seconds, not times when measured output
crosses zero. We will calculate them and connect each result to heater
temperature or shaft motion.

Use the heater from Lessons 13 and 44 with $K=.2$ °C/point, $\tau=5$ s and one-second input delay. Use the shaft from Lessons 33–35 with $J=1$, $B=1$, $dt=.02$ s, torque limits ±20 N·m, $K_p=4$ or 16 for P-only tests, and $K_d=7$ for the PD contrast. The Laplace transform and sampled motion are already familiar; these are the same physical examples.

For the heater $G(s)=.2/(5s+1)$, solving $5s+1=0$ gives a pole at $s=-.2$ s⁻¹. The constant numerator never becomes zero, so this transfer has no finite zero. The following physical calculation explains what that pole means.

Hold the heater at 75% for its entire history, then initialize the water at $T=36$ °C, one degree above its 35 °C equilibrium. With no feedback and no new command, $x=T-35$ obeys

$$
\dot x=-\frac{x}{5}.
$$

Trying $x=Ae^{pt}$ in $5\dot x+x=0$ gives $(5p+1)Ae^{pt}=0$, so a nonzero natural response has $p=-.2$ s⁻¹, the denominator root. Therefore $x(t)=e^{-t/5}$ °C, with $x(5)=e^{-1}=.367879$ °C. The denominator root $-.2\ \mathrm{s^{-1}}$ is the natural pole associated with this exponential. The initial decay begins immediately because the initial heat-loss imbalance already exists; a one-second command delay does not delay this natural response. A command delay shifts a response to a new command, not the decay of an already displaced state.

A negative real pole gives a decaying exponential. A positive real pole would give growth, and a pole at zero preserves a constant natural deviation instead of returning it to zero. The tank with equal inflow and fixed outflow provided an example of a level that remains displaced.

For the unloaded shaft with P-only feedback, the physical equation is

$$
J\ddot\theta+B\dot\theta+K_p\theta=K_pr.
$$

For fixed r, use displacement z=θ−r so the natural-response equation has zero right-hand side. The trial form $z=Ae^{st}$ gives $Js^2+Bs+K_p=0$. For $J=B=1$, $K_p=4$ gives

$$
s^2+s+4=0,\qquad s=-.5\pm j1.9364917.
$$

The quadratic formula gives $s=[-1\pm\sqrt{1-16}]/2$. Define the imaginary unit j by $j^2=-1$, so $\sqrt{-15}=j\sqrt{15}$. The two roots are a complex conjugate pair: they have the same real part and opposite imaginary parts. The identity $e^{j\omega t}=\cos(\omega t)+j\sin(\omega t)$ explains why combining the conjugate exponentials gives real oscillation. A pair $\sigma\pm j\omega$ produces the real response

$$
e^{\sigma t}\left(c_1\cos(\omega t)+c_2\sin(\omega t)\right),
$$

where $\sigma$ is in $\mathrm{s^{-1}}$, $\omega$ is in rad/s, and $c_1,c_2$ set the initial state. The period is $2\pi/\omega$. Thus $K_p=4$ predicts period 3.244623 s; both modes have envelope $e^{-.5t}$, and the envelope ratio after one cycle is about $e^{-.5(3.244623)}=.1974$. For $K_p=16$, roots are $-.5\pm j3.968627$, period 1.583214 s, and envelope ratio about .4531. With $K_d=7,K_p=16$, roots are repeated $-4$, a decaying nonoscillatory contrast; do not ask that case for oscillation peak spacing.

Now use numerator roots to answer the heater cancellation question. Use the familiar no-delay heater $G(s)=.2/(5s+1)$ and a PI controller with Kp=2.5 percentage points/°C and Ki=0.5 percentage points/(°C·s). Write $i=I-75$ for controller-state deviation and $\mathcal I$ for its transform. With $i(0)=0$, transforming $\dot i=K_i e$ gives $s\mathcal I=K_iE$. Thus the controller transfer is $C(s)=K_p+K_i/s=2.5+0.5/s$. The closed-loop target transfer is

$$
H(s)=\frac{C(s)G(s)}{1+C(s)G(s)}
=\frac{.5(s+.2)}{5(s+.1)(s+.2)}
=\frac{.1}{s+.1}.
$$

The controller zero at $-.2$ cancels the stable heater pole algebraically in this target-to-temperature expression. After cancellation, the setpoint transfer has one pole at −0.1 s⁻¹ and no finite zero. The thermal state has not vanished. Let d be added heating expressed in command points; extra heat extraction D kW corresponds to d=−D/2. With zero setpoint deviation and $U_d$ the transform of d, $X=G[-CX+U_d]$, so the disturbance transfer is $X/U_d=G/(1+GC)$:

$$
G_S(s)=\frac{.2s}{(5s+1)(s+.1)},
$$

which retains both denominator factors. Initial-state mismatch or parameter error can reveal the thermal mode. An exact stable-pole cancellation can persist when the pure input delay is included, but that delay still changes the remaining closed-loop dynamics. Cancellation is therefore an input-output model property, not proof that a physical state is absent.

For an explicit initial measurement reversal, suppose electrical pickup from the heater command causes a temperature-reading offset. Use an illustrative sensor-error relationship that could be established by holding water temperature constant while changing the heater command: measured deviation $m=x-.1v$ °C, where $v$ is applied-command deviation in percentage points. With no delay,

$$
G_m(s)=\frac{.2}{5s+1}-.1
=\frac{.1-.5s}{5s+1}.
$$

Solving $.1-.5s=0$ gives a zero at $+.2$ s⁻¹, while the pole remains $-.2$ s⁻¹. The positive zero concerns how the input enters the measured response; it is not a growing natural pole. A manual 75→85% step at actual $T=35$ can show measured temperature 34 °C initially even though actual water warms, and measured final temperature 36 °C while actual temperature reaches 37 °C. Here the immediate negative measurement offset and the slower positive heating effect compete. Zeros can change response shape in other ways too; not every zero implies an initial reversal. This is a measurement defect to diagnose before changing feedback; do not run feedback from that defective PV as if it were physical negative heating.

## Prediction

Calculate the heater value at 5 s, both P-only shaft root pairs and periods, and the repeated-root PD contrast. Calculate the PI cancellation and explain why the disturbance path still contains both poles. For the sensor-bench example, calculate the sign of the initial measured step and distinguish it from actual water motion.

Predict why sampled peak times can differ from continuous periods, and why torque clipping at ±20 invalidates an unsaturated root comparison. Do not invent analytic stability guarantees for the limited digital implementation.

## Implementation

Write scalar ST calculations for normalized shaft coefficients $a_1=B/J$ and $a_0=K_p/J$, and discriminant $a_1^2-4a_0$. For a negative discriminant, calculate real part $-a_1/2$, imaginary magnitude $\sqrt{-(a_1^2-4a_0)}/2$, and period $2\pi/\omega$. For a nonnegative discriminant, calculate the two real roots and do not divide by a nonexistent oscillation frequency. Check the Kp=4 and16 values, then calculate the sensor zero from its linear numerator. Do not implement a general root library. Expose requested/applied torque, delay history, true/measured angle or temperature, and saturation status. For the heater natural-decay case, hold applied 75% and initialize $T=36$ °C; for shaft cases, retain scalar angle and velocity state.

## Experiment in Control Studio

1. Heater: hold 75% throughout, initialize true temperature 36 °C and no feedback, and record 30 s from **Confirm and start**. Display true/model temperature, PV, applied command and pole prediction.
2. Shaft: run separate full resets with angle and velocity zero, SP 0→1, $K_p=4$ P-only and $K_p=16$ P-only for 8 s at $dt=.02$. Then run $K_p=16,K_d=7$ separately to observe the nonoscillatory contrast; integral remains off.
3. Record peak spacing only for the P-only oscillatory cases. Compare sampled periods and envelope ratios with continuous predictions while applied torque remains unsaturated.
4. Keep the sensor-bench initial-reversal example as a paper or diagnostic calculation; do not use defective PV for a feedback performance claim. Stop for angle ±2 rad, velocity ±10 rad/s, applied torque outside ±20 N·m, heater temperature outside 0–80 °C, invalid data or an unresponsive monitor. Command zero torque/heat, restore and reset before each case.

## Evidence and decision

The heater record should show immediate natural decay and approximately .367879 °C deviation at 5 s, not a one-second delayed start. Shaft records can support period and envelope comparisons only before saturation. The P-only root predictions are $-.5\pm j1.9364917$ and $-.5\pm j3.968627$; the PD case is repeated $-4$, so its response is not classified by oscillation peaks.

For the PI example, report the algebraic cancellation and the separate disturbance path. For the sensor example, report the calibration relation and sign; an initial measured reversal is evidence of instrumentation behavior, not a negative thermal response. No sampled limited run is analytic proof for continuous hardware.

## Conclusion

Poles are denominator roots associated with natural exponential decay or oscillation. Complex pairs give an envelope and period, while zeros describe numerator cancellation or competing input effects. The heater, shaft and sensor examples show why root calculations must be tied to the physical signal and its measurement path.

## Reason for the next lesson

Poles and zeros explain the factors in a particular input-output response. The PI heater has a remaining question: cancellation makes one mode absent from its setpoint transfer, yet its physical temperature and integral state still exist. Which equation captures all those connected-loop modes, including a response to a changed initial state?

[Characteristic equations](../../07-classical-stability-and-frequency-design/43-characteristic-equation/README.md) derives that equation from the physical balances and retained controller state before any input-output cancellation.
