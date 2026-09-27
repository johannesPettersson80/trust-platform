# 79. Robust control

You will choose a PI controller that must meet temperature requirements despite uncertain heat loss and delivery delay. You will check physical feasibility first, then compare fixed gains across the same uncertainty cases.

## The problem

Adaptation cannot supply a trustworthy parameter estimate from every record.
Suppose we can justify a range of heat loss and delay but not their exact
current values. A controller that works only at the nominal model may fail
elsewhere in that range.

Use $50\dot T=2u(t-L)-H(T-20)-D$, with H in 8, 10, 12 kW/°C, L in .8, 1, 1.2 s, and u 0–100%. The physical uncertainty ranges are $8\le H\le12$ kW/°C and $.8\le L\le1.2$ s. Process gain $K=2/H$ and time constant $\tau=50/H$ change together as H changes; do not vary K and τ independently. H and L are varied independently in this exercise.

## What we need

We need a controller that remains suitable across a justified range of
possible plant dynamics when their exact current values cannot be established.
Nominal success is not enough, and physical capacity must be checked first.

## The new idea

**Robust control** designs for a stated range of possible process models
rather than assuming one nominal model is exact. We use it when uncertainty
can be bounded but cannot be reliably estimated away. A controller stays fixed
while the physical parameters vary; each candidate faces the same range and
requirements.

Steady command is

$$
u_{ss}=\frac{H(T-20)+D}{2}.
$$

For H=8,10,12, baseline SP35 needs 60,75,90%; target SP36 needs 64,80,96%; D=10 at SP35 needs 65,80,95%. These endpoints are feasible. The earlier 38 °C target requires $12(38-20)/2=108$% at H=12. This branch deliberately defines the 36 °C requirement before candidate selection so it can investigate controller behavior within the uncertain heater’s capacity. A real application requiring 38 °C would need a capacity or requirement change.

Choose frozen candidates such as $K_p=2.5,K_i=.5$ and $K_p=2.5,K_i=.4$, or a documented manual/model candidate. Use dt=.1, $T_t=1$, beta=1, filter bypassed, full H-specific reset (baseline I=7.5H), and identical limits. A finite grid qualifies only the tested points; it cannot prove every intermediate value.

Robust **stability** means small disturbances decay for every allowed model. Robust **performance** additionally requires the specified temperature and actuator behavior. The finite grid can reject either claim, but a bound over the whole range can establish more than sampled points.

For a frequency-domain stability check, use the reference heater at H=10 and the shortest allowed delay, .8 s:

$$
G_0(s)=\frac{2e^{-.8s}}{50s+10},\qquad G_{H,L}(s)=\frac{2e^{-Ls}}{50s+H}.
$$

The reference is chosen at the shortest delay so every allowed relative error is causal: it uses present and past inputs. Write $G_{H,L}=G_0(1+\delta)$. Then

$$
\delta=\frac{50s+10}{50s+H}e^{-(L-.8)s}-1.
$$

This error system is stable for H>0. Splitting it into loss-coefficient error and extra delay, and using $|e^{-jz}-1|=2|\sin(z/2)|\le\min(|z|,2)$, gives the envelope

$$
|\delta(j\omega)|\le W(\omega)=\frac{2}{\sqrt{64+2500\omega^2}}+\min(.4\omega,2),\quad\omega\ge0.
$$

Here W bounds fractional complex model error, including phase error. It is an envelope to calculate, not a new filter to implement. For example, W(0)=.25; at .2 rad/s it is approximately .23617.

Let $T_0=C G_0/(1+C G_0)$, the complementary sensitivity taught earlier. Factoring the uncertain feedback denominator gives

$$
1+C G_{H,L}=(1+C G_0)(1+T_0\delta).
$$

The extra feedback loop cannot reach its critical value −1 if $|T_0\delta|<1$ at every frequency. With stable nominal internal dynamics, stable causal uncertainty and a well-defined connection, this is the **small-gain sufficient condition** for robust stability. A failed sufficient bound does not prove instability.

The infinity norm $\|F\|_\infty=\sup_{\omega\ge0}|F(j\omega)|$ means the largest amplitude ratio over all frequencies, including any limiting value. It is a worst-case measure, not an average. For the candidate $C=2.5+.5/s$, cancellation of the reference’s stable thermal pole gives $C G_0=.1e^{-.8s}/s$. Its remaining characteristic equation is $s+.1e^{-Ls}=0$. Starting from the stable zero-delay pole −.1, the first imaginary-axis crossing requires $\omega=.1$ and $\omega L=\pi/2$, or L=15.708 s. The .8 s reference is below that boundary; the canceled thermal mode is also stable.

For this candidate,

$$
|T_0|=\frac{.1}{\sqrt{.01+\omega^2-.2\omega\sin(.8\omega)}}
\le\frac{.1}{\sqrt{.01+.84\omega^2}}.
$$

Since $W\le.25+.4\omega$, their product is at most $(.025+.04\omega)/\sqrt{.01+.84\omega^2}$. To bound it at every frequency, use $(ap+bq)^2\le(a^2+b^2)(p^2+q^2)$, which follows by expanding the nonnegative square $(aq-bp)^2$. With $a=.25$, $b=.04/\sqrt{.84}$, $p=.1$ and $q=\sqrt{.84}\omega$, the bound is

$$
\sup_\omega |T_0\delta|\le\sqrt{.25^2+.04^2/.84}=.25378<1.
$$

This establishes robust stability for the stated continuous linear model family and this candidate. It does not establish the step-response requirements or sampled, saturated operation.

A performance weight similarly divides a response by its allowed magnitude. For a 10 kW sinusoidal extraction, the physical path is $S G_D$, with $G_D=-1/(50s+H)$; the command path is $-C S G_D$. If the allowed amplitudes at a frequency were 1 °C and 5 command points, their normalized scores would be $10|S G_D|/1$ and $10|C S G_D|/5$. A score at most one means that particular allowance is met. These example frequency allowances are distinct from this lesson’s actual step requirements; a small stability bound cannot substitute for either performance assessment.

## Prediction

Calculate all endpoint commands, predict which delay produces the largest transient error, and predict whether the lower-gain candidate trades speed for robustness. Explain why nominal success cannot guarantee untested H/L values. Calculate W at .2 rad/s and reproduce the .25378 all-frequency stability bound for the first candidate. Explain which performance questions it leaves unanswered.

## Implementation

Write scalar ST calculations for W and the displayed bound at selected frequencies to inspect the arithmetic; the algebraic bound supplies the all-frequency argument. These calculations are analysis values, not controller outputs. Write the steady-command check as `RequiredPowerPct := (LossKwPerC * (SetpointC - 20.0) + ExtractionKw) / 2.0;` and reject an infeasible proposed test before enabling it. Keep H, L, D, delayed history, candidate gains, requested/applied output and validity explicit. Freeze each candidate before its 18-record comparison. Do not adapt or disable a candidate merely because H changes; that change is the declared uncertainty case. Invalid sensor data still follows the fault/recovery contract.

## Experiment in Control Studio

Use D=0 and D=10 only. Requirements are fixed: SP 35→36 °C, peak ≤37 °C and |error|≤0.2 °C from 30–60 s; separately SP=35 with D=10, minimum ≥33 °C and the same error band from 30–60 s. Test all nine combinations H=8,10,12 and L=.8,1,1.2, including nominal H=10,L=1. Each candidate therefore has 18 separately reset records: nine parameter cases times two events.

1. Reset each H case to SP=PV=35 °C, correct baseline output and integral/history, then run SP35→36 for 60 s at each L.
2. Reset and run SP35,D=10 for 60 s at each L. Start every clock at **Confirm and start** and record T, error, request/applied output and clipping.
3. Repeat all eighteen records for each frozen candidate. Abort if true temperature leaves 0–80 °C, applied command leaves 0–100%, data become invalid, or time stops. Command zero heat, cancel the event, stop the simulator if needed and reset. Record clipping at 100% separately.

## Evidence and decision

Use one table per candidate and H/L/event case. A candidate qualifies only if every tested case meets its requirement and output remains feasible. Report finite-grid performance scope explicitly. Separately report the all-frequency robust-stability argument for the 2.5/.5 candidate under its continuous, linear assumptions. No such bound has been supplied for a different candidate; it needs its own analysis. If no candidate qualifies, report the blocker and redesign need.

## Conclusion

Robust heater selection compares fixed controllers across physically coherent heat-loss and delay uncertainty. Feasibility, transient performance and evidence coverage determine the decision.

## Reason for the next lesson

The model range describes heat loss and delivery delay. It does not account
for commands lost, delayed or reordered by a communication link.
[Networked control](../../13-networked-fault-aware-and-diagnostic-control/78-networked-control/README.md)
defines when remote commands may be accepted, how long they may be held and
how the loop returns to local control.
