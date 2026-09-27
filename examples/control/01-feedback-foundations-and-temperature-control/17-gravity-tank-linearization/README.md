# 42. Linearization

You will approximate the gravity tank’s nonlinear response with a linear model near one operating level. You will calculate the approximation’s error and decide how far from that level it remains useful.

## The problem

The square-root outflow from Lesson 41 is nonlinear. A tangent model can support local prediction and controller calculations, but only around a declared operating level and for a declared step size. We will compare the approximation with exact equilibrium calculations.

## What we need

We need a simpler model for small level changes near an operating point,
with a usable process gain and response time. We must also establish how
far from that point the approximation remains accurate enough.

## The new idea

**Linearization** replaces a nonlinear relationship near an operating point
with a straight line having the same value and slope there. The slope is
the derivative of that relationship. This gives a simpler model for small
changes; accuracy generally worsens farther from the operating point.

Use A=.5 m², c=.020 m^(5/2)/s, h0=.25 m, nominal q0=.010 m³/s, and:

A dh/dt=qin−c√h.

Define level deviation x=h−.25 m and inlet deviation v=qin−.010 m³/s. Absolute levels remain bounded 0–1 m, inlet 0–.020 m³/s, and dt=1 s.

$$
g=\frac{c}{2\sqrt{h_0}}=.020\ \mathrm{m^2/s}.
$$

The tangent predicts $q_{out,linear}=q_0+g(h-h_0)=q_0+gx$. Substitute h=0.25+x and qin=0.010+v into the volume balance. Since the nominal flow cancels nominal outflow:

$$
A\dot x=v-gx,\qquad \dot x=2v-.04x.
$$

Dividing by g gives $(A/g)\dot x+x=v/g$. At a fixed inlet deviation, the final level deviation is xf=v/g. The remaining gap obeys $\dot z=-gz/A$, so it decays as $z=z(0)e^{-t/\tau}$ with time constant τ=A/g=25 s. After one time constant, 36.8% of the gap remains. The process gain is K=1/g=50 s/m²: an inlet change of 0.001 m³/s predicts a final level change of 0.05 m.

For qin=.011, v=.001. The exact nonlinear equilibrium is (.011/.020)²=.3025 m. The tangent equilibrium is h0+v/g=.25+.001/.020=.3000 m, differing by .0025 m. For qin=.015, the exact equilibrium is .5625 m and the tangent predicts .5000 m, differing by .0625 m. The larger step is therefore outside a .005 m equilibrium-accuracy criterion even though the local equations remain useful for explaining direction and time scale.

## Prediction

Calculate g, τ, both exact equilibria, both tangent equilibria, and the two differences. Before viewing records, declare that an equilibrium difference of at most .005 m is acceptable for this local-model exercise. Predict which step can pass and why an endpoint at 200 s must still be distinguished from an analytical equilibrium.

## Implementation

Open src/Main.st. Keep nonlinear LevelM and linear deviation X as separate REAL model states, initialized with nonlinear level 0.25 m and linear deviation X=0. Feed both the same applied inlet flow and dt=1 s. Calculate g from c and h0, then calculate the tangent derivative 2v−.04X and one forward-Euler step. Keep measured level read-only and publish absolute level, deviation, nonlinear prediction, tangent prediction, and residual. Do not silently change operating point or sample interval.

## Experiment in Control Studio

1. Stop and restart Simulator. Confirm measured/model level=.25 m, qin=.010 m³/s, qout=.010 m³/s, X=0, valid timestamp, and dt=1 s.
2. Before results, declare the .005 m equilibrium criterion and the 200 s record. Run two separately reset cases, qin=.011 and qin=.015.
3. At Confirm and start define time zero, apply the selected inlet, and record measured level, nonlinear model, tangent model, applied flow, slope, and residual for 200 s.
4. Stop for absolute level outside 0–1 m, inlet outside 0–.020 m³/s, invalid data, or stopped time. Restore qin=.010 normally and fully reset before the second case. If inlet is zero, gravity outflow continues; it is not a frozen-level condition.
5. Compare analytical equilibrium error and final-10 s drift separately. Do not change the criterion after seeing the large-step mismatch.

## Evidence and decision

For qin=.011, exact .3025 m versus tangent .3000 m differs by .0025 m and can meet .005 m. For qin=.015, exact .5625 m versus tangent .5000 m differs by .0625 m and fails. The fixed tangent has τ=25 s, so 200 s spans eight of its time constants. At the larger nonlinear equilibrium 0.5625 m, the local slope gives a 37.5 s time constant; 200 s is about 5.3 of that late-response scale. Check remaining drift rather than treating either ratio as exact nonlinear settling.

## Conclusion

The tangent slope g=.020 m²/s gives a 25 s local model around .25 m. Its usefulness is a bounded, step-dependent decision rather than a global replacement for the square-root plant.

## Reason for the next lesson

The local model has useful gain and time-scale values near .25 m, but the outlet slope changes with level. A fixed PI setting therefore need not give comparable responses at lower and higher operating levels.

[Gain scheduling](../../09-industrial-control-structures/56-gain-scheduling/README.md) uses that known dependence to calculate settings across three levels and compare them with fixed gains under the same requirements.
