# Qualify a digital PID realization on the positioning table

Course position: Lesson 25 of 84. Previous lesson: [Diagnose mechanical nonidealities before retuning a servo](../24-servo-mechanical-nonidealities/README.md). Next lesson: [Positional, incremental, and combined PID realization](../../03-production-quality-digital-pid/26-pid-forms/README.md).

**Lesson objective:** Calculate and execute forward-Euler, backward-Euler, and
Tustin realizations at several controller intervals, then decide which cases
remain credible from actual sampling, held torque, tracking, quantization,
aliasing, precision, and computation-delay evidence.

**Evidence class:** Simulation evidence, not physical timing, drive
latency, or encoder qualification.

## The problem

The equipment is the encoder-controlled industrial positioning table since
Lesson 19. Lesson 24 separated command shaping from drive and mechanical
effects; this lesson looks inside the controller: a continuous PID law does
not say which samples it uses, how it approximates an integral or
derivative, or when torque reaches the drive.

The plant advances every 10 ms; the controller updates every one, two, or
ten scans, holding the last torque between updates. Gains remain 8 N m/rad,
2 N m/(rad s), 3 N m s/rad, and a 0.05 s derivative filter — only
realization, interval, resolution, and delay change; derive the actual
recurrences and qualify method and interval from that evidence.

## What we need

We need a sampled controller whose update interval, held output, coefficient
calculation, and aliasing limit are explicit, not hidden by continuous-time
notation.

## The new idea

A **continuous signal** exists at every instant; a **sampled signal** has
values only at updates. The **controller sample interval ($\Delta t$)** is
the elapsed time in seconds between controller updates.

The **sampling frequency ($f_{s}$)** is updates per second, in hertz (Hz):

$f_s=\frac{1}{\Delta t},$

`f_s = 1 / dt`: one update occupies $\Delta t$ seconds, so one second
contains $1/\Delta t$ updates.

The **Nyquist frequency ($f_{N}$)** is half the sampling frequency;
frequencies above it cannot be distinguished uniquely:

$f_N=\frac{f_s}{2},$ i.e. `f_N = f_s / 2`.

Two samples per cycle are the minimum to distinguish high and low values;
above this limit, a component aliases under a false lower frequency.

A **zero-order hold** retains the last motor torque between updates; faster
plant simulation creates no new controller decisions during that hold.

The **setpoint ($SP$)** is desired angle, **process value ($PV$)** is
sampled angle, and **error ($e$)** is SP minus PV. Index **k** is the
current update, `k-1` the previous.

The **integral torque** remembers the error area:

$T_I(t_k)=T_I(t_{k-1})+K_i\int_{t_{k-1}}^{t_k}e(t)\,dt.$

The three methods differ in how they approximate that area.
**Forward Euler** uses a rectangle with the previous error,

$T_{I,k}=T_{I,k-1}+K_i\Delta t\,e_{k-1}.$

**Backward Euler** uses a rectangle with the current error,

$T_{I,k}=T_{I,k-1}+K_i\Delta t\,e_k.$

**Tustin**, or the trapezoidal rule, uses the average of the previous and
current error as the rectangle's height,

$T_{I,k}=T_{I,k-1} +K_i\frac{\Delta t}{2}(e_k+e_{k-1}).$

All three multiply error by elapsed time (error area), then by $K_i$
(N·m).

The **filtered derivative torque ($T_D$)**, written $D$ below, estimates
$K_d$ times the measurement-rate of change $dPV/dt$, subtracted from torque
for damping. Its continuous equation:

$\tau_D\frac{dD}{dt}+D=K_d\frac{dPV}{dt}.$

Forward Euler evaluates the right side from previous rate $r_{k-1}$:

$D_k=D_{k-1} +\frac{\Delta t}{\tau_D}(K_dr_{k-1}-D_{k-1}).$

Backward Euler replaces both derivatives with backward differences, solved
for $D_k$:

$D_k=\frac{D_{k-1}+(K_d/\tau_D)(PV_k-PV_{k-1})} {1+\Delta t/\tau_D}.$

Tustin integrates over one interval, trapezoid-approximating $D$'s area,
solved for $D_k$:

$D_k= \frac{(2\tau_D-\Delta t)D_{k-1} +2K_d(PV_k-PV_{k-1})} {2\tau_D+\Delta t}.$

Three different state updates, not three labels for one calculation.

The **quantization step ($q$)** is the 0.001 rad spacing between encoder
values.

The **alias frequency ($f_{\mathrm{alias}}$)** is the false lower frequency
a sampled disturbance above $f_N$ produces:

$f_{\mathrm{alias}}=|f_{\mathrm{signal}}-m f_s|,$

with integer $m$ so the result lies in $[0,f_N]$: 8 Hz sampled at 10 Hz
gives $m=1$, $f_{\mathrm{alias}}=|8-10|=2\,\mathrm{Hz}.$

The **computation delay ($T_{\mathrm{delay}}$)** is one update: torque is
applied at the next update.

`LREAL` improves coefficient and state precision; it cannot prevent aliasing,
quantization, stale samples, or a poor interval.

## Prediction

From $J\dot\omega+B\omega=T$, $1\,\mathrm{kg\,m^2}$ inertia over
$1\,\mathrm{N\,m\,s/rad}$ friction gives a 1 s velocity time constant, so
**samples per time constant** is 1 s divided by $\Delta t$.

| $\Delta t$ | $f_{s}$ | $f_{N}$ | Samples/1 s | $K_i\times\Delta t$ |
| ---: | ---: | ---: | ---: | ---: |
| 0.01 s | 100 Hz | 50 Hz | 100 | 0.02 N m/rad |
| 0.02 s | 50 Hz | 25 Hz | 50 | 0.04 N m/rad |
| 0.10 s | 10 Hz | 5 Hz | 10 | 0.20 N m/rad |

Calculate the 0.05 s derivative-filter coefficients before execution:

| $\Delta t$ | Forward $\Delta t$/τ | Backward 1+$\Delta t$/τ | Tustin retained | Tustin position |
| ---: | ---: | ---: | ---: | ---: |
| 0.01 s | 0.200 | 1.200 | 0.818 | 54.545 N m/rad |
| 0.02 s | 0.400 | 1.400 | 0.667 | 50.000 N m/rad |
| 0.10 s | 2.000 | 3.000 | 0.000 | 30.000 N m/rad |

Dividing the Tustin recurrence by $2\tau_D+\Delta t$ gives the tabulated
retained-state and position-change coefficients above.

Thus $dt = 0.10 s$, $f_{s} = 10 Hz$, $f_{N} = 5 Hz$: predict a different
torque recurrence, longer holds, a 2 Hz alias, and a one-update delay shift.

## Implementation

The lesson executes each alternative:

```st
CASE MethodSelection OF
    0: IntegralTorque := IntegralTorque + LREAL#2.0 * CycleTime * PreviousError;
    1: IntegralTorque := IntegralTorque + LREAL#2.0 * CycleTime * Error;
ELSE
    IntegralTorque := IntegralTorque
        + LREAL#0.5 * LREAL#2.0 * CycleTime * (Error + PreviousError);
END_CASE;

RequestedTorque := Controller.Update(
    Setpoint := Setpoint,
    ProcessValue := MeasuredPosition,
    CycleTime := ControllerCycleTime,
    MethodSelection := MethodSelection,
    DelayEnabled := DelayEnabled
);
```

Between updates `RequestedTorque` stays unchanged: the hold.

## Experiment in Control Studio

Control Studio exposes the most diagnostic case: Tustin at 100 ms, above
the 5 Hz Nyquist limit. Test writes one reviewed Boolean authority,
$\mathtt{P1.ExperimentEnabled}$, for 20.01 s; the project selects
method/interval, enables control, applies SP 0.5 rad, and false restores SP
0, disabled, zero torque.

Fresh native runtimes own the three-method 20 ms, 10/20/100 ms interval, and
delayed 10 ms comparisons; one UI run cannot claim that matrix (seed
20260828, deterministic).

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait for **Simulation**, **Running**, current samples, and good quality. Confirm SP = 0 rad, measured PV = 0 rad, held torque = 0 N·m, and experiment authority false. Add SP, PV, velocity, held torque, and authority to **Signals**; `P1.MethodSelection`, `P1.IntervalSelection`, `P1.DelayEnabled`, and the update-pulse flag are not bound Monitor signals, so the selected Tustin/100 ms case is known from the design above, not read back here.
   - **Decision before action:** Write the 100 ms values $f_s=10$ Hz, $f_N=5$ Hz, integral weight 0.20, Tustin coefficients 0 and 30 N·m/rad, and predicted 2 Hz alias. Require torque to change only at update pulses.
   - **Successful evidence:** The plot advances, values are finite/current, quality is good, and **Start recording** is enabled. Select **Start recording**.
   - **Recovery:** For stale, unavailable, nonfinite, or poor-quality samples, stop the Simulator, correct the displayed runtime or binding fault, restart, and repeat the baseline check.
   - **Record:** Note all initial signals and your calculated timing, coefficient, hold, and alias predictions.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **20.01**. The reviewed authority is **Experiment authority** (`P1.ExperimentEnabled`). The project owns SP, method, interval, delay selection, and control enable.
   - **Decision before action:** Confirm that the plan writes only that Boolean and restores it false. Require PV within ±4 rad, velocity within ±20 rad/s, and torque within ±10 N·m. Expect one torque update followed by nine held 10 ms plant scans. This run is made so that the fastest Tustin run is not evidence that Euler alternatives, slower updates, quantized measurements, aliased interference, or delayed application remain acceptable is measured rather than assumed.
   - **Successful evidence:** Select **Review test**, verify authority, timeout, stop conditions, and false restoration, then select **Confirm and start**. Keep **Abort test** available. Response must show SP rising to 0.5 rad, torque held flat between the 100 ms updates consistent with the selected Tustin/100 ms case, quantized PV showing interference near 2 Hz, and safe restoration.
   - **Recovery:** Select **Abort test** for nonfinite, stale, out-of-limit, or incorrectly timed data; confirm authority false, SP zero, control disabled, and held torque zero before retrying.
   - **Record:** Retain terminal state, restoration, actual update timestamps, maximum interval/jitter, hold
     duration, SP/PV/torque, sampled spectrum, estimated alias, and seed. The deterministic seed is
     **20260828**. The inherited limitation under test is: The fastest Tustin run is not evidence that Euler alternatives, slower updates, quantized measurements, aliased interference, or delayed application remain acceptable. The decision this run supports is: Accept only a method and interval whose actual update timing, held output, tracking, torque, finite-state, quantization, alias, and delay evidence satisfy the declared requirements.

There is no separate disabled Validate stage. The native comparison owns the
method, interval, and delay matrix; Monitor and Test expose one declared case.

## Evidence and decision

Plot SP, PV, held torque, and update pulses; compare error, overshoot,
settling, torque travel, interval, jitter, and hold duration.

**Maximum absolute position error**, the maximum over all samples of the
absolute value of SP minus PV, is

$e_{\max}=\max_k|SP_k-PV_k|,$

not the final error nor the largest signed error. **Applied-torque total
variation**, the sum of absolute successive changes, is

$TV(T)=\sum_k|T_k-T_{k-1}|,$

which cannot be cancelled by alternating movement.
Final-window position span is $\max(PV)-\min(PV)$, and torque span is $\max(T)-\min(T)$, each over
the final two seconds, so a small final sample alone cannot hide variation.

The **measurement residual** is sampled PV minus its 5-to-20 s window mean;
its **spectrum**, estimated from 5-to-20 s zero-crossing rate (frequency)
and half the peak-to-peak size (amplitude), should show a 2 Hz alias at
100 ms from the 8 Hz interference.

A case **settles** only if PV stays within 0.020 rad of SP and final 2 s
spans stay within 0.010 rad PV and 0.500 N m torque; otherwise it is **not
settled**, and a growing rather than bounded response is **unstable**.

Evidence measured a 23.69 N m TV gap between forward- and backward-Euler at
20 ms, a 0.0020 rad 2 Hz alias at 100 ms, and delayed first torque at
0.020 s versus 0.010 s undelayed.

![Digital choices change the implemented controller](figures/digital-realization.svg)

[Retained samples](figures/digital-realization-trace.csv) ·
[Decision summary](figures/digital-realization-summary.csv) ·
[Method-comparison samples](figures/digital-method-comparison-trace.csv) ·
[Update-timing samples](figures/digital-update-timing-trace.csv) ·
[Alias-spectrum samples](figures/digital-alias-spectrum-trace.csv)

Plots expose recurrences, update pulses, and holds. The
retained 100 ms samples give an estimated alias frequency of 2.00 Hz.

![Torque and position traces for all six candidate realizations](figures/digital-method-comparison.svg)

Torque steps once per interval and holds; the 100 ms trace's sparser steps
contrast with the 20 ms and 10 ms traces.

![Update-pulse timing showing held torque between scans](figures/digital-update-timing.svg)

Update pulses mark when torque may change; the flat trace between pulses
shows the zero-order hold.

![Sampled residual spectrum showing the 100 ms alias peak](figures/digital-alias-spectrum.svg)

The 100 ms residual spectrum peaks near 2 Hz, the alias of the 8 Hz
interference, not at 8 Hz itself.

Every tested case remained finite and bounded:

| Case | Settling | TV | Longest hold | Interval |
| --- | ---: | ---: | ---: | ---: |
| Forward Euler, 20 ms | 11.73 s | 101.34 N m | 0.02 s | 0.02 s |
| Backward Euler, 20 ms | 11.74 s | 77.65 N m | 0.02 s | 0.02 s |
| Tustin, 20 ms | 11.73 s | 89.11 N m | 0.02 s | 0.02 s |
| Tustin, 10 ms | 11.73 s | 94.46 N m | 0.01 s | 0.01 s |
| Tustin, 100 ms | 11.61 s | 24.33 N m | 0.10 s | 0.10 s |
| Tustin, 10 ms plus delay | 11.73 s | 98.25 N m | 0.01 s | 0.01 s |

Lower movement at 100 ms is not automatically better: the command is held
longer and 8 Hz interference appears as 2 Hz. Judge timing, tracking,
alias, and delay together.

Torque shall change only on update pulses at the selected 10/20/100 ms
spacing; PV shall stay on the 0.001 rad grid; the 100 ms alias shall fall
between 1.9 and 2.1 Hz at ≥0.001 rad amplitude; the delayed case shall shift
first torque by one update; all signals stay finite, torque within ±10 N m.

## Conclusion

Each recurrence approximates the same continuous terms differently.
Increasing $\Delta t$ lengthens holds and delays, and can alias an 8 Hz
component to 2 Hz. Precision cannot reconstruct unsampled measurements;
timing, sampling, and resolution are controller behavior.

## Reason for the next lesson

This choice used timing, noise, and closed-loop evidence available now;
later lessons add tools that refine it, and any timing, filtering, delay,
or hardware change requires requalifying it.

Lesson 26 builds on this result by comparing positional, incremental, and
combined production PID forms without pretending that their retained states
are interchangeable.
