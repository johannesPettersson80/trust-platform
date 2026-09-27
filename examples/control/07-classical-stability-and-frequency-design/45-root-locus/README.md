# 47. Root locus

You will use a root locus to choose the shaft's proportional gain from its
predicted decay and oscillation. You will keep velocity feedback fixed and
test whether the chosen gain meets the position requirements.

## The problem

The previous lesson calculated the poles for a particular controller gain.
That describes one choice, but we still need to choose a gain that makes the
shaft reach its target quickly enough without excessive overshoot.

## What we need

We need **a way to choose the proportional gain so the shaft reaches its
target quickly enough without excessive overshoot**.

The previous lesson calculated the poles for one gain. That tells us about
that particular choice, but not how changing the gain changes the response.

We therefore need to see **how the poles move as $K_p$ changes**, while the
shaft and other controller settings stay fixed. That lets us identify
promising gains and reject unsuitable ones.

**Root locus provides that picture.** We use it to choose a candidate, then
test its actual overshoot and settling in Control Studio.

## The new idea

A **root locus** plots the closed-loop pole locations as one controller gain
changes. We use it to see which response speeds and oscillation patterns are
available, choose a candidate gain, then test its actual response.

Each point is a pole at a particular gain, not the physical shaft position.
The horizontal coordinate is its real part, describing decay or growth;
the vertical coordinate is its imaginary part, describing oscillation rate.
Both use inverse seconds, with imaginary magnitude interpreted in rad/s.
Joining corresponding pole positions as gain changes traces the locus.

Use the same shaft with inertia $J=1$ kg·m² and friction
$B=1$ N·m·s/rad. Let $\theta$ be angle, $r$ its target and $u$ motor torque.
The physical balance is $J\ddot\theta+B\dot\theta=u$.
With P torque $u=K_p(r-\theta)$, the characteristic equation is:

$$
s^2+s+K_p=0.
$$

The zero-initial-state shaft transfer from torque to angle is
$G_0(s)=\Theta(s)/U(s)=1/[s(s+1)]$. It follows by transforming the
same torque balance; $\Theta$ and U are the angle and torque transforms.

Now include the velocity braking already taught in PD:
$u=K_p(r-\theta)-K_d\dot\theta$ with $K_d=7$ N·m·s/rad fixed.
The total damping coefficient is $B+K_d=1+7=8$, giving:

$$
s^2+8s+K_p=0.
$$

The effective transfer with that velocity feedback already connected is
$G_7(s)=1/[s(s+8)]$. The physical shaft has not changed. We will vary
$K_p$ while holding the braking fixed. Keep torque limits ±20 N·m,
$0\le K_p\le20$ in N·m/rad and the 0.02 s controller update.

For these two separate families, the quadratic formula gives:

$$
G_0:\ s=\frac{-1\pm\sqrt{1-4K_p}}{2},\qquad
G_7:\ s=\frac{-8\pm\sqrt{64-4K_p}}{2}.
$$

For G0, the roots are [−1±√(1−4Kp)]/2. At Kp=0 they are 0 and −1; at .25 they meet at −.5; at 4 they are −.5±j1.93649; at 16 they are −.5±j3.96863. Increasing Kp moves the conjugate roots away from the real axis, increasing their oscillation frequency, but cannot move their real parts left of −.5. A screening goal Re(s)≤−2 for a four-second settling requirement is therefore impossible on this P-only locus. It is a design screen, not a guarantee that a recorded response settles in four seconds.

With fixed Kd=7, the shaft equation has total velocity coefficient B+Kd=8. The second locus has roots [−8±√(64−4Kp)]/2. At Kp=12 roots are −2 and −6; at Kp=16 they are a repeated −4 root; at Kp=20 they are −4±j2. The breakaway behavior shows why adding fixed velocity feedback changes the plant family seen by Kp. We vary one scalar on each separate locus, not two gains at once.

## Prediction

Calculate the roots at Kp=0.25,4,16,12,20. Decide which locus can meet the Re(s)≤−2 screen and which selected Kp, 12 or 16, you will test. Predict whether the screen guarantees the peak/error requirement; it does not account for torque saturation or sampling.

## Implementation

The root locus is calculated and displayed by **Control Studio**, not by the
ST program. ST applies the gain selected from that plot to the shaft:

$$
u_r=K_p(r-\theta)-7\omega.
$$

Here $u_r$ is requested torque, $r-\theta$ the measured position error, and
$\omega$ measured shaft velocity. Keep the existing ±20 N·m output limiter
and the 0.02 s update. The student changes only the selected $K_p$; derivative
gain remains 7. There is no root-calculation or plotting task to implement in ST.

The prepared project supplies the shaft model and Kp sweep. Use the calculated
poles to choose a gain, then set its startup value in `src/Main.st` while
Simulator is stopped. Restart before the separate experiment. Keep `Kd=7`
fixed in both the calculation and program. Calculating the locus does not
change the live controller.

## Experiment in Control Studio

1. In the prepared project, open **Control Studio → Tune**, then choose **Root locus** from **Tuning method**.
   Select **Calculate root locus**. Confirm the physical model is
   $G_0(s)=1/[s(s+1)]$, the varying parameter is `Kp`, and `Kd=7` is fixed.
   The declared numerator `Kd s + Kp` represents the feedback terms in the
   characteristic equation, giving the same locus as $K_pG_7(s)$. It does
   not mean differentiating the target in the ST controller. Control Studio calculates
   the pole paths; no ST root calculations supply this plot.
2. Inspect the plotted pole pairs for gains 12, 16 and 20. Relate their
   horizontal positions to decay and their imaginary parts to oscillation.
   Check them against the short calculations above to understand the graph.
3. Before testing, choose a gain from the locus and explain the expected
   benefit and cost. The requirements are a 0→1 rad target step over 8 s,
   peak≤1.1 rad, and absolute error≤.02 rad from 4–8 s, with torque±20 N·m.
4. Stop Simulator. In `src/Main.st`, set the `Kp` initializer to your selected gain, leaving `Kd=7` unchanged. Save and restart to zero angle and velocity, integral OFF, derivative filtering bypassed and update .02 s. The ST program applies the chosen gain; the root-locus calculation itself does not write the runtime.
5. Open **Identify → Method → Project experiment**, select **Review project test**, then **Confirm and start**. Inspect the first 8 s of angle, velocity and requested/applied torque in the recorded response. The declared 8.1 s envelope includes restoration. Record the peak and 4–8 s error against the requirements fixed above.
6. Select an alternative gain from the same locus and repeat from the same reset, keeping the plant and $K_d$ unchanged. Record both gains and their measured results in your comparison notes before another run. Explain whether the measurements support the chosen pole locations. To repeat the earlier candidate, stop Simulator, restore that `Kp` initializer and restart; online parameter transfer is a separate commissioning topic.
7. Abort if angle leaves ±2 rad, velocity leaves ±10 rad/s, applied torque
   leaves ±20 N·m, or measurement/clock becomes invalid. Command zero torque,
   stop the simulator if necessary and fully reset. Reaching a torque limit
   is recorded saturation, not an out-of-range applied command.

## Evidence and decision

Retain the **Control Studio root-locus plot**, its model, inspected gain/pole
pairs, the selected candidate and the reason for choosing it. Pair that design
evidence with the matched response records. A scalar calculation in ST or a
response trace alone does not establish that the student used root locus.

Check that the locus varies only physical $K_p$ with $K_d=7$ fixed. A multiplier
on the whole PD controller varies both gains and is a different locus. Compare
the selected gain's predicted decay/oscillation with measured peak and settling;
sampling or torque saturation can cause a candidate to miss the requirements.

## Conclusion

Root locus makes the effect of one scalar Kp visible. The P-only shaft is limited by real-part geometry; fixed Kd changes the same physical shaft's damping and creates a different Kp family.

## Reason for the next lesson

The pole paths show how proportional gain changes the shaft’s response. Checking selected gains has not yet established the complete stable range or its boundary cases.

[Routh–Hurwitz stability](../44-routh-hurwitz/README.md) derives a complete interval from polynomial coefficients. It starts with the same physical shaft under proportional control, with velocity feedback removed, so the table construction is easy to follow.
