# 38. Positional and incremental PID

You will calculate a PID controller’s complete output and its change from one update to the next. You will show when these two forms produce equivalent commands and identify the stored values needed to keep them equivalent.

## The problem

A positional controller rebuilds an unlimited torque each scan from P, I and D. An incremental controller adds a calculated change to its previous unlimited request. These forms agree only when their gains, samples, integral rule, derivative estimate, tracking correction and retained output describe the same controller. Saturation or a mode transition can otherwise create a hidden difference.

## What we need

We need to establish when a complete-output PID calculation and an
incremental calculation represent the same controller. That includes the
stored request, tracking corrections and behavior when parameters change.

## The new idea

A **positional PID** calculates the complete output request each update.
An **incremental PID** calculates its change and adds that to the previous
request. We derive the second form from the first so every retained state
and correction has the same meaning.

Use the shared shaft $J=1,B=1$, $K_p=16,K_d=7$, $K_i=4$ when I is enabled, $dt=.02$ s, and torque limits ±20 N·m. Use scalar previous error, previous filtered velocity, previous unlimited request and integral state. The integral state contains any held-load compensation; no extra bias is required.

Let $u_k$ be requested torque, $e_k$ angle error, $I_k$ integral torque and
$\omega_k$ the velocity signal selected for D. With ordinary integral update
and constant gains,

$$
u_k=K_p e_k+I_k-K_d\omega_k,\qquad
I_k=I_{k-1}+K_i e_k\,dt.
$$

Subtracting the preceding complete request gives

$$
\Delta u_k=K_p\Delta e_k+K_i e_k\,dt-K_d\Delta\omega_k.
$$

Here Δe and Δω mean current minus previous values. This is the incremental formula. It retains the previous **unlimited** request, not the clipped applied torque, unless an explicitly defined instantaneous tracking policy says otherwise. With the after-request anti-windup correction from the holding-torque lesson, the previous scan's tracking correction $\gamma_{k-1}$ is added to the next recurrence because the current integral state already includes the prior correction:

$$
\Delta u_k=K_p\Delta e_k+K_i e_k\,dt-K_d\Delta\omega_k+\gamma_{k-1}.
$$

The current $\gamma_k$ is stored for the next scan. Here $\gamma_k=(0.02/0.2)(u_{applied,k}-u_{requested,k})$, in N·m. The worked bounded example below has zero tracking correction. If gains change, the proportional and derivative differences must include $P_{\mathrm{new}}-P_{\mathrm{old}}$ and $D_{\mathrm{new}}-D_{\mathrm{old}}$, or state must be preloaded; a formula using only $\Delta e$ is not equivalent.

For an isolated calculation with the same supplied velocity signal in both controllers, let $e_{prev}=.10$, $\omega_{prev}=.20$, $I_{prev}=1.0$, and $u_{prev}=1.2$. On the next sample $e=.096$, $\omega=.22$, $dt=.02$. Then $I_{\mathrm{trial}}=1.0+4(.096)(.02)=1.00768$, $P=1.536$, $D=-1.54$, and $u=1.00368$. The increment is

$$
16(.096-.10)+4(.096)(.02)-7(.22-.20)
=-.064+.00768-.14=-.19632,
$$

so $1.2-.19632=1.00368$. A 0.001 N·m tolerance is a suitable arithmetic agreement check when both forms use identical floating inputs; it is not motion accuracy.

If velocity is an unfiltered backward angle difference at fixed dt, its incremental term can also be written

$$
-K_d\Delta\omega_k=-\frac{K_d}{dt}(\theta_{m,k}-2\theta_{m,k-1}+\theta_{m,k-2}).
$$

This follows by subtracting the two consecutive angle slopes. It uses measured angle, so a setpoint step does not enter the derivative term. For filtered velocity, retain the filtered velocity difference instead.

## Prediction

Predict that a previous incremental request of 25 N·m is clipped to 20 N·m while the unlimited positional request remains 25 N·m. With a tracking correction $\gamma=.1(20-25)=-.5$, predict that $-.5$ is carried into the next recurrence. Ignoring it changes the controller. Predict that changing $K_p$ at constant error changes positional P immediately while the naive $\Delta e=0$ formula does not.

## Implementation

Compute P, trial I, filtered D, unlimited positional request and applied limit in named scalar variables. Retain previous unlimited request, previous error, previous filtered velocity, integral state, previous tracking correction and elapsed time. The incremental path calculates the displayed change from those same terms; it does not hide a complete opaque algorithm. Use current tracking correction only for the next scan, and apply the normal limit/tracking order from Lesson 35.

For a gain or filter revision, either preload state so the complete current P and D are preserved or include their new-minus-old difference in the increment. On transfer, set $I=u_{\mathrm{applied}}-P-D$ and explicitly define the first-scan tracking behavior.

## Experiment in Control Studio

Use **Identify → Tune → Validate**. On **Identify**, select the project-owned test and review the one position loop. The project feeds the positional `Control.PID` path and the incremental shadow from the same timestamped position record; it exposes unlimited request, applied torque, previous request and tracking state together. Press **Review project test**, then **Confirm and start**.

Use the compact project parameter editor to run the nominal record, the 25→20 N·m clipping case and the constant-error gain revision as separate resets (`TrialKind` 0–2). Keep the isolated arithmetic example separate from motion evidence. Review the retained response on Identify, then use **Tune** and **Validate** for the applicable result and requirements. Stop on angle outside ±2 rad, velocity outside ±10 rad/s, applied torque outside ±20 N·m, invalid data/time or monitor failure. Command zero torque, clear retained state and reset before another case.

## Evidence and decision

The worked update must agree at $u=1.00368$ N·m within 0.001 N·m when all scalar inputs match. The limit case must show that previous unlimited request and applied torque are distinct and that tracking correction is carried forward once. A gain-change mismatch must include the missing $P$ or $D$ state term, not be hidden by retuning.

Compare positional and incremental outputs over the finite record before claiming equivalence. If validity, saturation, tracking or parameter revisions differ, report the boundary rather than presenting one form as universally identical.

## Conclusion

Incremental PID follows by subtracting consecutive complete positional equations. Its compact update is trustworthy only when previous unlimited output, P/I/D state, tracking correction, gains, samples and limits represent the same controller.

## Reason for the next lesson

Matching two controller calculations does not establish that the complete
servo meets motion and operating-state requirements.
[Servo commissioning](../../14-integrated-capstones/82-servo/README.md)
combines those calculations with damping, load support and lifecycle handling
on the same rigid shaft, then checks the integration.
