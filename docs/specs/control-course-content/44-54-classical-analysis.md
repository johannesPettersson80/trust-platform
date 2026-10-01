# Lesson explanations: 44–54

Part of [the course specification](../control-course-teaching-specification.md).
These are the explanations to use in the mapped lessons.

## 48. Laplace transforms

A **Laplace transform** converts a time-dependent signal into another
representation in which differential equations can be handled algebraically.

We use it to connect plant and controller equations and calculate responses
without solving each connected differential equation from scratch.

For a signal $x(t)$:

$$
X(s)=\int_0^\infty x(t)e^{-st}\,dt.
$$

$t$ is time, $s$ is the transform variable with units of inverse time,
and $X(s)$ is the transformed signal. Begin with real positive $s$ for which
the integral converges. Complex values are introduced in the next lesson.

Integration by parts gives:

$$
\mathcal L\{\dot x\}=sX(s)-x(0).
$$

A derivative becomes multiplication by $s$, with an extra term for the initial
state. For the heater balance $5\dot x+x=0.2v$:

$$
(5s+1)X=0.2V+5x(0).
$$

At zero initial deviation, the **transfer function** is
$G(s)=X/V=0.2/(5s+1)$. It describes the input-output relationship under
zero initial conditions.

We use this expression to connect feedback, calculate a response and compare
it with the heater trace in Control Studio. Actual delivery delay must be
included when comparing with the delayed plant.

Next we learn how features of these expressions explain decay and oscillation.

## 49. Poles and zeros

**Poles and zeros** are values of the transform variable that reveal important
features of a model's input-output response.

For a rational transfer function:

$$
G(s)=\frac{N(s)}{D(s)},
$$

first cancel common factors. A **pole** is a root of the remaining denominator
$D(s)$; a **zero** is a root of the remaining numerator $N(s)$.
A root is a value that makes the corresponding expression zero.

For the heater $G(s)=0.2/(5s+1)$, the pole is $s=-0.2$ s⁻¹.
It corresponds to the natural response $e^{-0.2t}$: a temperature displacement
decays with a five-second time constant. The constant numerator gives no finite zero.

The shaft can have a pair of complex poles:

$$
s=\sigma\pm j\omega,\qquad j^2=-1.
$$

Their real response combines sine and cosine oscillation with an
$e^{\sigma t}$ envelope. Negative $\sigma$ means decay; positive means growth;
$\omega$ sets oscillation rate.

Zeros influence how an input excites the response and its shape. They are
not times when the measured temperature or position crosses zero.

In Control Studio we compare these predictions with heater decay and shaft
motion. A cancelled transfer-function factor may still correspond to internal
dynamics, so next we derive the complete characteristic equation.

## 50. Characteristic equations

A **characteristic equation** determines the natural modes of a complete
linear dynamic system. Its roots tell us which exponential rates can occur
without a continuing external input.

We derive it from the connected plant and controller equations, including
retained controller state. One input-output transfer can hide a mode through
cancellation, so it is not always enough.

For a shaft with position and velocity feedback:

$$
J\ddot z+(B+K_d)\dot z+K_pz=0,
$$

where $z$ is displacement from a fixed target, $J$ is inertia, $B$ friction,
and $K_p,K_d$ are controller gains.

Try $z=Ae^{st}$. Differentiation contributes factors of $s$, giving:

$$
Js^2+(B+K_d)s+K_p=0.
$$

With $J=B=1$, $K_p=16$ and $K_d=7$, this becomes $(s+4)^2=0$.
The repeated rate −4 s⁻¹ gives decaying motion; a repeated root can also
produce a term $te^{-4t}$, which still decays.

In Control Studio we compare predicted modes with position and velocity after
a small perturbation. The heater example additionally shows why integral state
must remain in the connected equations.

Next we examine how those roots move when a gain changes.

## 51. Root locus

We need **a way to choose the proportional gain so the shaft reaches its
target quickly enough without excessive overshoot**.

The previous lesson calculated the poles for one gain. That tells us about
that particular choice, but not how changing the gain changes the response.

We therefore need to see **how the poles move as $K_p$ changes**, while the
shaft and other controller settings stay fixed. That lets us identify
promising gains and reject unsuitable ones.

**Root locus provides that picture.** We use it to choose a candidate, then
test its actual overshoot and settling in Control Studio.

A **root locus** plots the closed-loop pole locations as one controller gain
changes. We use it to choose a gain by seeing which response speeds and
oscillation patterns are available.

Each point represents a pole at a particular gain, not the physical position
of the shaft. The horizontal coordinate is the real part, describing decay
or growth. The vertical coordinate is the imaginary part, describing
oscillation rate.

For the shaft with fixed velocity gain $K_d=7$:

$$
s^2+8s+K_p=0,
\qquad
s=\frac{-8\pm\sqrt{64-4K_p}}{2}.
$$

$K_p$ is the gain varied along this locus. The shaft and every other controller
parameter remain fixed.

- $K_p=12$: poles −2 and −6.
- $K_p=16$: repeated pole −4.
- $K_p=20$: poles −4 ± 2j.

The first case contains a slower decaying mode; the last contains oscillatory
modes. The locus helps us select a candidate against response requirements,
but output limiting and other real dynamics still need testing.

The hands-on exercise uses **Control Studio → Tune → Root locus** to
calculate and display the locus. The student must not write a root solver,
quadratic-discriminant loop or plotting routine in ST to obtain that graph.

The prepared design must represent the shaft with velocity feedback already
connected: $G_7(s)=1/[s(s+8)]$, with $K_d=7$ fixed. The varying gain must
mean physical proportional gain $K_p$, not a multiplier on the whole PD
controller. The plot must show real/imaginary axes, the gain associated with
each inspected pole pair, and the poles for the selected candidate.

The student opens that model in Tune, selects Root locus and inspects the
pole pairs for $K_p=12$, 16 and 20. They use decay and oscillation to choose
a candidate before running. They then apply its gain to the existing ST
controller and use Test/Response to compare the same shaft target step at
that gain and an alternative, keeping $K_d$, plant, reset and requirements fixed.

ST implements only the control law
$u_r=K_p(r-\theta)-7\omega$ and the existing torque limits. Control Studio
performs the root-locus calculation. The student uses the plot to choose and
explain a gain, then the measured response to keep or reject it.

**Implementation gap recorded 7 September 2026:** the existing project is a
carriage P/lead comparison at 10 ms, with `controller.design: project` and
Monitor/Test stages. Its program, model and bindings do not match this shaft
exercise. The generic root-locus plot also scales the complete candidate
controller numerator. The project and design workflow must be aligned and
visually verified before this lesson is described as runnable. This gap does
not authorize replacing the intended plot exercise with calculations in ST.

## 52. Routh–Hurwitz stability

**The problem:** root locus checked selected gains. We need to establish the
complete stable range, including boundaries between those selected gains.
**What we need:** a test that derives stability conditions from the characteristic
polynomial instead of repeatedly solving its roots.

**The new idea must begin with this definition:** The Routh–Hurwitz stability
criterion is a mathematical test for whether a linear continuous-time system is
stable. It uses the coefficients of the characteristic polynomial without
solving for every pole. A stable system makes small disturbances die away;
an unstable system can make them grow.

Explain that a natural mode is proportional to $e^{st}$, where $s$ is a complex
rate in inverse seconds. Negative real parts decay; positive real parts grow;
zero real parts do not decay. Asymptotic stability requires every real part
strictly negative.

Use the approved shaft, $J=1$ kg·m² and $B=1$ N·m·s/rad, with position
$\theta$ in rad, fixed target $r$, applied torque $u$ in N·m, and proportional
gain $K_p$ in N·m/rad. Start from $J\ddot\theta+B\dot\theta=u$ and
$u=K_p(r-\theta)$. With deviation $z=\theta-r$, show
$J\ddot z+B\dot z+K_pz=0$ and substitution $z=Ae^{st}$ gives
$Js^2+Bs+K_p=0$, hence $s^2+s+K_p=0$.

Explain table construction before using the answer. For
$a_2s^2+a_1s+a_0=0$, the rows are $[a_2,a_0]$, $[a_1,0]$ and $[a_0,0]$.
The bottom entry is $(a_1a_0-a_2\times0)/a_1=a_0$, when $a_1\ne0$.
More generally, from two preceding rows $A,B$, calculate
$C_j=(B_0A_{j+1}-A_0B_{j+1})/B_0$. Explain each row represents the next
lower power of $s$ and missing entries are padded with zero.

For our shaft, show rows $[1,K_p]$, $[1,0]$, $[K_p,0]$. Count changes
between positive and negative entries down the first column: each change
counts one right-half-plane pole when ordinary nonzero-pivot rules apply.
With positive leading coefficient, the quadratic is asymptotically stable
exactly when all three first-column entries are positive: $K_p>0$.
Work through $K_p=1$ (no sign changes) and $K_p=-1$ (one). At $K_p=0$,
factor $s(s+1)$: poles 0 and −1; the zero mode does not decay and the
boundary is excluded. Never treat a zero entry as positive or silently divide
by zero. Explain that higher-order zero pivots/all-zero rows need special
handling; the supported Studio implementation explicitly reports unresolved
special cases and numerical uncertainty.

**Prediction:** with a small positive target step, positive gain restores
position, zero gain supplies no restoring torque, and negative gain drives
away from the target. State that stability does not guarantee fast settling.

**Control Studio experiment:** use the actual automatic Routh–Hurwitz control
to calculate the declared polynomial, table, first-column signs, unstable-pole
count and analytically derived interval for $K_p=1,0,-1$. Follow
[the feature contract](../control-studio-routh-hurwitz.md). Do not put Routh
math in ST or replace Studio analysis with paper-only calculations. Then use
existing parameter review/application and bounded Test on the actual shaft ST
controller. The README must name only controls verified in the rendered
extension and match the project's actual reset, step, stop and recording
behavior. Interpret observed angle, velocity and torque against the prediction;
saturation and sampling are outside the continuous linear proof.

**Conclusion:** answer which gains make this model's disturbances decay and why
the boundary is excluded. The next lesson asks how a stable system responds to
continuing periodic commands or disturbances.

## 53. Periodic excitation

**Periodic excitation** repeatedly changes a process input at a known rate.
We use it to investigate how the process responds to changes of different frequencies.

A step test measures a transition toward a new equilibrium. A sine input
instead lets us measure the size and lag of a continuing repeating response.

For the heater:

$$
u(t)=u_0+d\sin(\omega t),
\qquad
\omega=\frac{2\pi}{P}.
$$

- $u_0$: steady operating command.
- $d$: command amplitude, in percentage points.
- $P$: full cycle period, in seconds.
- $\omega$: angular frequency, in rad/s.

With $u_0=75\%$ and $d=5$ points, command varies between 70 and 80%.
At $\omega=0.2$ rad/s, one cycle lasts about 31.416 seconds.

We must allow startup transients to decay before measuring complete cycles.
Clipping the command changes the waveform and invalidates a simple
sinusoidal comparison.

In Control Studio we apply the bounded input at several frequencies,
record actual command and temperature, and select settled cycles.
Next we calculate their amplitude ratio and phase difference.

## 54. Frequency response

**Frequency response** describes how a linear system changes the amplitude
and phase of sinusoidal inputs at different frequencies.

The output repeats at the input frequency after startup effects have decayed,
but it can be smaller and delayed.

For the heater model:

$$
G(j\omega)=\frac{0.2e^{-j\omega}}{1+5j\omega}.
$$

$j^2=-1$, $\omega$ is frequency in rad/s, 5 s is the thermal time constant,
and the exponential represents the one-second delivery delay.

Substituting a repeating exponential into the physical balance gives this
ratio of output to input. Its magnitude and phase are:

$$
M=\frac{0.2}{\sqrt{1+(5\omega)^2}},
\qquad
\phi=-\omega-\arctan(5\omega).
$$

$M$ is temperature amplitude per command point and $\phi$ is phase in radians.
At $\omega=0.2$, $M=0.141421$ °C/point and $\phi=-0.985398$ rad.
A 5-point input therefore predicts temperature amplitude about 0.7071 °C.

A **Bode plot** shows magnitude and phase separately against frequency,
usually on a logarithmic frequency axis.

In Control Studio we measure the previous lesson's cycles and compare them
with this prediction. Next we determine how feedback changes disturbance
and measurement-noise responses.

## 55. Sensitivity functions

**Sensitivity functions** describe how feedback transmits or suppresses
different disturbances and inputs.

A physical heat disturbance enters the plant. Measurement noise instead
changes what the controller thinks the temperature is. Their effects are
therefore different even when both create a similar-looking measured fluctuation.

Define the loop transfer $L=CG$, where $C$ is the controller and $G$ the
plant input-output model. Then:

$$
S=\frac{1}{1+L},\qquad T=\frac{L}{1+L}.
$$

$S$ is **sensitivity** and $T$ **complementary sensitivity**.
They follow by solving the negative-feedback equations.

For target deviation $r$, physical disturbance $D$ and measurement noise $n$:

$$
y=Tr+SG_DD-Tn.
$$

$y$ is physical output deviation and $G_D$ is the disturbance-to-plant-output
path before feedback. Noise alone also produces controller output $u=-CSn$.

Although $S+T=1$, their complex magnitudes do not generally add to one.
Reducing disturbance sensitivity can leave important measurement-noise or
actuator-demand costs.

In Control Studio we inject physical load and measurement noise separately,
then inspect physical temperature, measured temperature and command.
Next we change controller dynamics deliberately to address a chosen tradeoff.

## 56. Loop shaping

**Loop shaping** designs controller dynamics to obtain useful gain and phase
behavior across frequencies.

We have identified frequency regions where disturbance rejection, response
speed or noise-driven command activity is inadequate. We now change the
controller for that specific requirement.

A simple compensation element is:

$$
F(s)=\frac{1+sT_z}{1+sT_p}.
$$

$T_z$ and $T_p$ are time constants in seconds. The zero is at $-1/T_z$
and the pole at $-1/T_p$.

When $T_z>T_p$, this is **lead compensation**: it adds positive phase over
a frequency range, but also raises high-frequency gain. Reversing that order
gives **lag compensation**.

For $T_z=10$ s and $T_p=2$ s, high-frequency gain approaches $10/2=5$.
Extra phase may help response, while that amplification can increase
noise-driven actuator movement.

The complete loop becomes $L=CFG$, so we must recalculate the whole response,
not judge $F$ alone. The frequency where $|L|=1$ can move too.

In Control Studio the student designs the lead with **Lead/lag design** on the
supplied model of the PI loop (`pi-loop-model.json`, $0.2\,e^{-s}/s$): a crossover
of 0.4082 rad/s and 11.54° of phase give $T_z = 3$ s and $T_p = 2$ s, which the
program applies at unit gain. The project test then compares the PI alone and the
3 s and 10 s leads on the same target, disturbance and noise cases. The next lesson checks the complete
loop's stability rather than relying on improvement at one frequency.

## 57. Nyquist stability

The Nyquist stability criterion is a mathematical test for whether a feedback loop is stable. A stable loop makes small disturbances die away; an unstable loop can make them grow into increasing oscillations.

It works by examining gain and phase around the loop:

- Gain: how much the controller and plant amplify or reduce a signal.
- Phase: how far their response lags behind that signal.

Normally, negative feedback corrects a deviation. But enough phase lag can make the correction arrive in the wrong direction and reinforce the deviation instead.

A Nyquist plot shows the loop’s gain and phase as frequency changes. Each frequency produces one point on a complex-plane graph. The point −1 represents gain 1 and phase −180°: the signal returns equally large but reversed. The subtraction in negative feedback then reinforces it.

The criterion counts how the complete curve goes around −1:

$$
\boxed{Z=P+N}
$$

With clockwise encirclements counted positively:

- $P$: number of unstable poles before closing feedback.
- $N$: signed number of encirclements of −1.
- $Z$: number of unstable poles after closing feedback.

For closed-loop stability, we need $Z=0$ and no poles on the stability boundary.

We use it to check whether a proposed controller remains stable with the plant’s dynamics and delay—before trusting that controller on the process.

## 58. Robustness margins

**Robustness margins** measure how far the loop's frequency curve $L(j\omega)$
stays from the critical point −1, each as the size of a model error the loop
tolerates:

- the **gain margin** GM is the factor by which the loop gain can grow before
  the curve passes −1: $GM=1/|L(j\omega_{180})|$ at the frequency where its angle
  is −180°;
- the **phase margin** PM is the angle between the curve and −1 where $|L|=1$,
  at the crossover frequency $\omega_c$;
- the **delay margin** $DM=PM/\omega_c$ (PM in radians) is the extra delay that
  turns the crossover point onto −1;
- the **peak sensitivity** $M_s$ is one over the curve's shortest distance to −1;
  it bounds the other two: $GM\ge M_s/(M_s-1)$ and $PM\ge 2\arcsin(1/(2M_s))$.

With SIMC's PI on the heater, $T_i=\tau$ and the controller's zero cancels the
heater's lag, so $L(s)=a\,e^{-Ls}/s$ with $a=KK_p/\tau=1/(\lambda+L)$. Then
$GM=\pi/(2aL)$, $PM=\pi/2-aL$ and $DM=\pi/(2a)-L$: a slower response buys margin.
For Lesson 22's balanced PI these are about 6.9 (16.8 dB), 77° and 5.9 s.
Control Studio shows the four margins beside every candidate. Validation on
deliberately mismatched plants confirms their meaning: the heater with three
times the gain and with 4 s more delay stays stable; with eight times the gain
or 7 s more delay, beyond the margins, it oscillates with growing amplitude.

A mechanical resonance can raise the loop gain near crossover and consume the
gain margin that a rigid model promised.

## 59. Mechanical resonance

A load turned through a flexible coupling adds a **resonance** to the drum axis:
the load angle follows the motor angle through
$\omega_r^2/(s^2+2\zeta\omega_r s+\omega_r^2)$, with the **natural frequency**
$\omega_r$ and the **damping ratio** ζ. At $\omega_r$ this factor has size
$1/(2\zeta)$ and angle −90°, and above it the angle approaches −180°. With the
encoder on the load, the resonance multiplies the loop gain there by $1/(2\zeta)$.

The simulated axis resonates at 3 Hz with ζ = 0.05. A multisine torque of seven
1 N·m tones, each a whole number of periods in 2 s, gives its measured frequency
response in one recording: at 3 Hz the gain is 10 times the rigid model
$1/(s(Js+B))$, which identifies ζ = 1/(2·10). Lesson 42's PID has loop gain
0.386 there on the rigid axis; the resonance lifts it to 3.86 where the loop's
angle passes −180°, so the gain margin is below 1 and the loop grows into a 3 Hz
oscillation.

A **notch filter** $N(s)=(s^2+\omega_n^2)/(s^2+(\omega_n/Q)s+\omega_n^2)$ on the
measurement has gain 0 at $\omega_n$ and 1 at steady state; the **quality factor**
Q sets its width $\omega_n/Q$. It must attenuate everywhere the resonance lifts
the loop gain above 1, and a wider notch adds more phase lag at crossover: at the
rigid loop's crossover, 7.33 rad/s, 24.6° with Q = 1 and 8.7° with Q = 3. The
program realizes the notch as a digital biquad whose coefficients follow from the
frequency and Q. Control Studio searches the declared frequencies 0 (off), 2, 3
and 4 Hz with Q = 1 and 3 on the actual ST: only the 3 Hz, Q = 1 notch meets the
step requirements; without it, at the wrong frequency or too narrow, the loop
oscillates. The notched loop is validated and applied.

The continuous design must survive sampling and computation delay.

## 60. Sampled-data stability

**Sampled-data stability** concerns a system whose physical process moves
continuously while its controller measures and updates at separate times.

A controller stable in a continuous-time model may become unstable when
commands are held too long or computation delays their application.

For the shaft, an exact held-torque update has the form:

$$
x_{k+1}=x_k+b\omega_k+cu_k,\qquad
\omega_{k+1}=a\omega_k+du_k.
$$

$x$ is angle deviation, $\omega$ velocity and $u$ the torque held during
the current interval. For the taught $J=B=1$ shaft with period $h$ seconds:

$$
a=e^{-h},\quad b=d=1-e^{-h},\quad c=h-1+e^{-h}.
$$

One update of computation delay means the newly calculated torque becomes
$u_{k+1}=-K_px_k-K_d\omega_k$ while $u_k$ remains applied now.

A discrete natural mode repeats with multiplier $z$:

$$
\text{mode at update }k\ \propto z^k.
$$

If $|z|<1$, it decays; if $|z|>1$, it grows.
The **unit circle** is therefore the discrete stability boundary.

In Control Studio we compare the stated 0.02 s and 0.6 s control periods
while keeping the physical simulation accurate. We inspect held torque and
motion to distinguish true timing-induced instability from numerical
integration error.

Next we return to the heater and use a measured disturbance to act earlier.
