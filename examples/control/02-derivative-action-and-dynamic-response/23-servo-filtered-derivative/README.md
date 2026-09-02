# Filtered derivative for an encoder servo

Course position: Lesson 23 of 84. Previous lesson:
[Derivative kick and setpoint weighting](../22-servo-derivative-kick/README.md).
Next lesson: [Mechanical actuator nonidealities](../24-servo-mechanical-nonidealities/README.md).

**Lesson objective:** Select a derivative-measurement filter from measured
noise attenuation and added lag, and explain why a second filter stage is not
automatically better.

This lesson uses deterministic simulated encoder noise. It does not qualify a
physical encoder, drive, or positioning axis.

**Equipment:** Encoder-based industrial positioning table.

## Where this lesson begins

Lesson 22 moved derivative action from the complete control error to the
measured position. A setpoint step therefore no longer produces a derivative
kick, while real table motion still produces damping torque. That solves a
command-path problem. It does not solve a measurement-path problem: derivative
action cannot tell real encoder motion from encoder noise.

The plant is still the same rotary table. Its simulated dynamics are

\[
J\ddot\theta=T_{applied}-B\dot\theta-G\sin(\theta),
\]

where \(\theta\) is table angle, \(J=1\ \mathrm{kg\,m^2}\) is inertia,
\(B=1\ \mathrm{N\,m\,s/rad}\) is viscous friction, and \(G=4\ \mathrm{N\,m}\)
is the peak gravity-torque coefficient. Gravity is therefore
\(4\sin(\theta)\), not a constant 4 N·m load. At the initial angle
\(\theta=0\), gravity torque is zero.

The simulated encoder adds zero-mean noise \(n_k\) to the true angle:

\[
PV_{raw,k}=\theta_k+n_k.
\]

The noise standard deviation is \(\sigma=0.002\ \mathrm{rad}\). Standard
deviation describes the typical distance of the samples from their mean; it
does not mean that every sample is exactly 0.002 rad wrong. The seed 20260828
makes the noise sequence repeatable so every filter sees the same experiment.

## Why derivative action amplifies encoder noise

With derivative setpoint weight \(c=0\), the PID differentiates
\(-PV_{control}\). Over one controller cycle, its derivative contribution is

\[
T_{D,k}=-K_d\frac{PV_{control,k}-PV_{control,k-1}}{\Delta t}.
\]

This formula comes directly from the definition of a derivative: rate of
change is change in value divided by elapsed time. Here
\(K_d=3\ \mathrm{N\,m\,s/rad}\) converts the estimated angular velocity into
torque, and \(\Delta t=0.02\ \mathrm{s}\) is the scan time. The minus sign is
important: increasing measured position produces opposing torque, which is
the damping learned in Lesson 20.

Now suppose the table is motionless and only noise changes. Substituting
\(PV_{raw}=\theta+n\) and cancelling the unchanged true angle gives

\[
T_{D,k}=-K_d\frac{n_k-n_{k-1}}{\Delta t}.
\]

The division by \(\Delta t\) explains the amplification. The same encoder
change produces twice the derivative torque if the scan time is halved. If
independent noise samples have standard deviation \(\sigma\), the difference
\(n_k-n_{k-1}\) has standard deviation \(\sqrt{2}\sigma\). The approximate RMS
derivative torque caused by noise alone is therefore

\[
T_{D,noise,rms}\approx
  K_d\frac{\sqrt{2}\sigma}{\Delta t}
  =3\frac{\sqrt{2}(0.002)}{0.02}
  \approx0.424\ \mathrm{N\,m}.
\]

The experiment reports changes in applied torque rather than derivative torque
itself. Consecutive derivative-torque changes contain
\(n_k-2n_{k-1}+n_{k-2}\). The squared coefficients add to
\(1^2+(-2)^2+1^2=6\), so a noise-only estimate is

\[
RMS(\Delta T_D)\approx K_d\frac{\sqrt{6}\sigma}{\Delta t}
\approx0.735\ \mathrm{N\,m}.
\]

The retained bypass result is 0.875 N·m because the complete closed loop also
contains proportional and integral action, table motion, gravity, and the
output limit. The estimate predicts the scale and inverse dependence on scan
time; it is not a replacement for the run.

## Deriving the low-pass filter

A low-pass filter creates a separate controller-facing measurement
\(PV_{control}\). Slow changes are allowed through, while rapid changes are
reduced. The raw encoder remains visible so filtering cannot hide the sensor
evidence.

For one continuous first-order filter, the defining equation is

\[
\tau\frac{dy}{dt}=x-y.
\]

Here \(x\) is the raw input, \(y\) is the filtered output, and the time constant
\(\tau\) is measured in seconds. The equation says that the output changes
faster when the input-output gap is large. A larger \(\tau\) spreads that
response over more time, so it smooths more strongly but follows real motion
more slowly.

The shipped `Control.LowPass` evaluates this equation once per scan. Let
\(y_{k-1}\) be the old output and \(y_k=y_{k-1}+\Delta y\) the new output.
Approximating the rate by \(\Delta y/\Delta t\) and the output during the scan
by its midpoint gives

\[
\tau\frac{\Delta y}{\Delta t}
=x_k-\left(y_{k-1}+\frac{\Delta y}{2}\right).
\]

Move the half-increment to the left and solve for \(\Delta y\):

\[
\left(\frac{\tau}{\Delta t}+\frac12\right)\Delta y=x_k-y_{k-1},
\]

\[
\Delta y=
\frac{\Delta t}{\tau+\tfrac12\Delta t}(x_k-y_{k-1}).
\]

Defining the dimensionless filter coefficient

\[
a=\frac{\Delta t}{\tau+\tfrac12\Delta t}
\]

gives the implemented update

\[
y_k=y_{k-1}+a(x_k-y_{k-1}).
\]

This form makes the behavior readable: each scan moves the output by the
fraction \(a\) of the remaining input-output gap. With
\(\Delta t=0.02\ \mathrm{s}\) and \(\tau=0.05\ \mathrm{s}\),

\[
a=\frac{0.02}{0.05+0.5(0.02)}=0.3333.
\]

If the raw input is 0.003 rad above the old filtered value, the first update is

\[
\Delta y=0.3333(0.003)=0.001\ \mathrm{rad}.
\]

The filter passes only one third of that sudden gap on the first scan. It does
not discard the remaining two thirds: later scans continue closing the gap if
the input stays there.

## First order, second order, and phase lag

**Filter order** is the number of identical stages in series. A first-order
filter applies the update once. The second-order choice sends the first
stage's output through the same update again:

\[
y_{1,k}=y_{1,k-1}+a(x_k-y_{1,k-1}),
\]

\[
y_{2,k}=y_{2,k-1}+a(y_{1,k}-y_{2,k-1}).
\]

The second stage sees an already smoothed signal, so it removes more rapid
variation. It also waits for the first stage to move before it can follow.
That extra delay is **phase lag**: the controller-facing position occurs later
than the physical motion it represents. The PID then reacts to old information.
Filtering therefore trades noise attenuation for feedback delay.

The characteristic cutoff of one continuous first-order stage is

\[
f_c=\frac{1}{2\pi\tau}.
\]

This follows by setting the stage's frequency ratio \(2\pi f\tau\) to one, the
point where the magnitude has fallen to \(1/\sqrt2\) of its low-frequency
value. For \(\tau=0.05\ \mathrm{s}\), \(f_c\approx3.18\ \mathrm{Hz}\). This
gives physical meaning to the time constant, but the actual selection must use
the sampled implementation and measured closed-loop response.

## What the two measures mean

Noise activity is measured from 15 to 20 seconds, after the large setpoint
transient. For \(N\) samples in that window, define each applied-torque change
as \(\Delta T_k=T_{applied,k}-T_{applied,k-1}\). Then

\[
RMS_{\Delta T}=\sqrt{\frac{1}{N}\sum_{k=1}^{N}(\Delta T_k)^2}.
\]

Squaring prevents positive and negative changes from cancelling, averaging
makes runs comparable, and the square root returns the unit to N·m. A smaller
value means less scan-to-scan drive activity.

Added delay is measured from 0 to 5 seconds with the positive lag area

\[
A_{lag}=\sum_k
\max(PV_{raw,k}-PV_{control,k},0)\Delta t.
\]

The difference has unit rad and multiplication by seconds gives rad·s. The
`max` keeps only periods when the controller-facing position trails the raw
position; otherwise positive and negative differences could cancel and hide
delay. Because the raw signal includes noise, this is an experiment-specific
comparison measure, not a universal pure-delay estimate.

Tracking is checked separately using the mean controller-facing position from
18 to 20 seconds:

\[
e_{final}=\left|0.5-\operatorname{mean}(PV_{control})\right|.
\]

A small final error does not prove that transient lag was acceptable. That is
why the lesson keeps tracking, torque activity, and lag as separate measures.

## Prediction and selection rule

Before running the project, predict this ordering:

1. Bypass filtering will have the greatest torque activity and least added lag.
2. Increasing \(\tau\) from 0.02 to 0.05 to 0.10 seconds will reduce activity
   but increase lag.
3. Two 0.05-second stages will reduce activity more than one stage and add more
   lag.

Freeze the decision rule before looking at the results: require
\(RMS_{\Delta T}\le0.250\ \mathrm{N\,m}\) and
\(A_{lag}\le0.020\ \mathrm{rad\,s}\). If several choices qualify, choose the
simplest order and then the smallest time constant. This prevents choosing a
visually smooth trace after seeing it.

## Implementation

The raw encoder is never overwritten. The selected filter changes only the
copy passed to PID:

```st
IF FilterSelection = 1 THEN
    ControllerPosition := FirstFilter.Update(
        Input := RawPosition, CycleTime := LREAL#0.02);
ELSIF FilterSelection = 2 THEN
    ControllerPosition := SecondFilter.Update(
        Input := RawPosition, CycleTime := LREAL#0.02);
ELSE
    ControllerPosition := RawPosition;
END_IF;

Input.ProcessValue := ControllerPosition;
MotorTorque := Controller.Update(Input := Input);
```

The Control Studio run owns one selected case: first order with
\(\tau=0.05\ \mathrm{s}\). Fresh native runtimes own the bypass, all three
first-order time constants, and the second-order comparison. A single UI run
must not be presented as if it executed that full matrix.

## Experiment in Control Studio

Test writes one reviewed Boolean authority, `P1.ExperimentEnabled`, for 20.02
seconds. The project owns the setpoint, mode, filter selection, and safe
restoration. The false state is Filter bypass, SP 0 rad, Manual mode, and zero
torque. The rising edge initializes the filters at the measured zero-angle
state, selects first order with \(\tau=0.05\ \mathrm{s}\), enters Auto, and
applies SP 0.5 rad. The falling edge restores the complete false state. The
deterministic Simulation seed is 20260828.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait for **Simulation**, **Running**, current samples, and good quality. Confirm SP = 0 rad, raw PV near 0 rad with visible seeded noise, controller-facing PV equal to raw PV, requested/applied torque = 0 N·m, Manual mode, Filter bypass, and experiment authority false. Add raw PV, controller-facing PV, applied torque, filter selection, time constant, mode, and authority to **Signals**.
   - **Decision before action:** Write the predicted activity/lag ordering and the two numerical limits above. Confirm that filtering only the controller-facing copy preserves the raw evidence.
   - **Successful evidence:** The Response plot advances, all values are finite and current, and **Start recording** is enabled. Select **Start recording**.
   - **Recovery:** If samples are stale, unavailable, nonfinite, or poor quality, stop the Simulator, correct the displayed binding or runtime fault, restart, and repeat the baseline check.
   - **Record:** Note the initial SP, both PV signals, requested/applied torque, mode, filter selection, time constant, authority, and your written prediction.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **20.02**. The reviewed authority is `P1.ExperimentEnabled`; the project, not the generic Test form, owns the 0.5-rad setpoint step and filter selection.
   - **Decision before action:** Confirm that the plan writes only the Boolean authority, restores it false, and keeps raw/controller PV inside -4 to 4 rad and applied torque inside -10 to 10 N·m. Expect less late torque activity than bypass, but a positive separation between raw and controller-facing PV during motion.
   - **Successful evidence:** Select **Review test**, verify the authority, timeout, stop conditions, and false restoration, then select **Confirm and start**. Keep **Abort test** available. Response must show authority true, first-order/0.05 s selected, SP rising to 0.5 rad, Auto response, distinct raw and filtered PV, and the complete safe restoration.
   - **Recovery:** Abort if a value becomes nonfinite, either PV leaves -4 to 4 rad, torque leaves -10 to 10 N·m, or data becomes stale. Confirm authority false, SP 0 rad, Filter bypass, Manual mode, and zero requested/applied torque before retrying.
   - **Record:** Retain terminal state, false restoration, selected-case SP/raw PV/controller PV/torque traces, late torque-increment RMS, early lag area, final mean error, and seed.

There is no separate disabled Validate stage. The runnable project and its
native test own the declared comparison; Monitor and Test expose the selected
case honestly.

## Evidence and decision

![Derivative filtering trades encoder-noise activity for lag](figures/filtered-derivative.svg)

[Retained samples](figures/filtered-derivative-trace.csv) ·
[Decision summary](figures/filtered-derivative-summary.csv)

The retained deterministic simulation produced:

| Filter | \(\tau\) | \(RMS_{\Delta T}\) | \(A_{lag}\) | Decision |
| --- | ---: | ---: | ---: | --- |
| Bypass | — | 0.875 N·m | 0.009053 rad·s | Reject: activity |
| First order | 0.020 s | 0.466 N·m | 0.010424 rad·s | Reject: activity |
| First order | 0.050 s | 0.192 N·m | 0.017473 rad·s | Select |
| First order | 0.100 s | 0.097 N·m | 0.031202 rad·s | Reject: lag |
| Second order | 0.050 s | 0.046 N·m | 0.028451 rad·s | Reject: lag |

For the selected case, the activity reduction relative to bypass is

\[
1-\frac{0.192292}{0.874919}=0.780,
\]

or 78.0%. The 0.10-second and second-order choices look smoother, but both
violate the predeclared lag limit. The 0.05-second first-order filter is the
only tested candidate that meets both limits, so it is selected.

## What you should take away

A derivative term is an engineered dynamic element, not an unfiltered
subtraction of two samples. Filtering the measurement reduces the rapid
changes that derivative action turns into torque, but it also delays real
feedback. Filter order and time constant must therefore be selected from both
attenuation and lag evidence.

Lesson 24 keeps this filtered servo and adds Coulomb friction, deadband,
backlash, stiction, and motion limits. Those effects leave different patterns
in SP, PV, and torque. More filtering or more PID gain cannot repair a
mechanical transmission defect, so the next task is diagnosis rather than
automatic retuning.
