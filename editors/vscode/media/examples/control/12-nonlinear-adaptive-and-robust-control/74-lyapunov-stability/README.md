# 76. Lyapunov stability

You will use an energy function to determine which initial arm positions and velocities are guaranteed to return toward upright under the ideal PD model. You will check that the required torque stays within the actuator limit throughout that region.

## The problem

The preceding planning exercise checked finite predictions and runs. To
establish behavior for a whole region of starting conditions, we need more
than a collection of successful trajectories. The upright arm makes that
question physical through motion, gravity and available torque.

An upright arm has gravity torque that increases its positive angle away from upright. A controller can settle small disturbances while losing authority at larger angles or after torque saturation. An energy argument gives a regional statement: which initial states are proven to remain bounded and converge under the ideal continuous model.

## What we need

We need a stability argument covering a region of starting conditions,
rather than only the trajectories we happened to simulate. That region must
also remain consistent with the torque the actuator can supply.

## The new idea

**Lyapunov stability analysis** uses a function of the system state to
establish whether motion remains near an equilibrium. A successful run shows
one trajectory; a mathematical argument can cover every initial state in a
specified region. If we also establish that the states approach equilibrium,
we have **asymptotic stability**.

Use angle $\phi$ from upright, positive in the direction of increasing angle, and $\omega=\dot\phi$. The arm has mass $m=2$ kg, center-of-mass lever $l=4/(2\cdot9.81)=.2038736$ m, and $G=mgl=4$ N·m. Use supplied total mechanism inertia $J=1\ \mathrm{kg\,m^2}$ for this illustrative arm model, viscous friction $B=1\ \mathrm{N\,m\,s/rad}$, $K_p=16$, $K_d=7$, $dt=.02$ s and torque limits ±20 N·m.

Weight acts through perpendicular lever arm $l\sin\phi$, giving gravity torque $mgl\sin\phi$. The plant and controller are

$$
J\dot\omega=\tau-B\omega+G\sin\phi,\qquad
\tau=-K_p\phi-K_d\omega.
$$

Positive $\phi$ is destabilized by positive gravity torque near upright. This mechanism is deliberately parameterized with total $J=1$; it includes the arm and drive, rather than adding an unspecified inertia to the earlier shaft.

For the arm, choose the following energy-like quantity in joules:

$$
V=\frac12J\omega^2+\frac12K_p\phi^2+G(\cos\phi-1).
$$

The first term is kinetic energy, the second is virtual spring energy from P control, and the cosine term is gravity potential relative to upright. Differentiating,

$$
\dot V=J\omega\dot\omega+K_p\phi\omega-G\sin\phi\,\omega
=-(B+K_d)\omega^2=-8\omega^2.
$$

Near upright, $|\sin\phi|\le|\phi|$ and $\cos\phi\ge1-\phi^2/2$, the latter following from $1-\cos\phi=\int_0^{|\phi|}\sin t\,dt\le\int_0^{|\phi|}t\,dt=\phi^2/2$. Hence

$$
V\ge\frac12\omega^2+
\left(\frac12K_p-\frac G2\right)\phi^2
=\frac12\omega^2+6\phi^2.
$$

V is zero at upright rest and positive elsewhere. A function with this property that does not increase along motion is called a **Lyapunov function**. For the set of states with $V\le.1$ J, $|\omega|\le\sqrt{.2}=.4472$ rad/s and $|\phi|\le\sqrt{.1/6}=.1291$ rad. The torque is bounded by $16(.1291)+7(.4472)=5.20$ N·m, below ±20, so saturation cannot occur inside this certificate. Since $V$ cannot increase, a valid continuous trajectory starting inside remains inside. The only invariant point with $\dot V=0$ is upright rest ($\phi=0$, $\omega=0$): when $\omega=0$, equilibrium requires $-16\phi+4\sin\phi=0$, which has only $\phi=0$ because $|\sin\phi|\le|\phi|$ and $16>4$. Thus the ideal continuous model is asymptotically stable in this region: states remain near upright and approach it as time passes.

## Prediction

Calculate $V$ for $\phi=.1,\omega=0$: approximately .0600167, inside .1. Calculate it for $\phi=.2,\omega=0$: approximately .240266, outside the proven region. Predict that the first case cannot leave the certificate under the ideal model and that the second is unproved, not classified as unstable. Predict the regional angle, velocity and torque bounds.

## Implementation

Write the scalar energy calculation in ST using `SIN` and `COS`, whose arguments are angles in radians. Calculate $\phi,\omega,\sin\phi,\cos\phi$, P/D torque, applied torque and $V$. Use radians and explicit units. Mark the certificate unproved when $V>.1$, state leaves the regional bounds, torque saturates, or sampled timing/data is invalid. The code computes the energy and command; it does not claim that sampled updates satisfy the exact continuous $\dot V$ identity.

## Experiment in Control Studio

1. Reset $\phi=.1$ rad, $\omega=0$, upright target, no retained state, and at **Confirm and start** begin an 8 s record at $dt=.02$ s. Display energy terms, $V$, $\dot V$ estimate, requested/applied torque and validity.
2. Run the regional case, then reset and run an initial $\phi=.2$ case for comparison. Use $\phi$ bounds ±.3 rad, velocity bounds ±2 rad/s and torque bounds ±20 N·m.
3. If you have completed [sampled stability](../../07-classical-stability-and-frequency-design/50-position-z-plane/README.md), optionally repeat with dt=.1 s as a sampling comparison, keeping the physical plant integration unchanged. Treat an outside-certificate result as unproved; do not call it unstable solely because the exact continuous derivative guarantee is absent.
4. If data is invalid or the arm exits bounds, command zero torque, stop Simulator because gravity can continue moving the arm, and fully reset before repeating.

## Evidence and decision

The certificate supports only valid unsaturated continuous-model reasoning for $V\le.1$. Compare sampled $V$, state errors and torque with that boundary, and report where sampling or saturation invalidates the proof. Outside the region, evaluate the observed behavior separately; the sufficient energy bound has not classified it.

## Conclusion

The upright arm's kinetic, virtual-spring and gravity energies produce $\dot V=-8\omega^2$ in the ideal model. The $V\le.1$ sublevel has explicit angle, velocity and torque bounds and converges to upright; states outside it require other evidence.

## Reason for the next lesson

The regional argument concerns returning to upright. At a nonzero target,
gravity still applies torque at rest, so PD alone needs position error to
oppose it. [Feedback linearization](../75-feedback-linearization/README.md)
uses the known angle-dependent gravity term to calculate that compensating
torque directly, then tests errors in the model and measurement.
