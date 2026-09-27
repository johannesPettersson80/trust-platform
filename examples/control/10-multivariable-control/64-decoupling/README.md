# 65. Static decoupling

You will coordinate the two heater commands to reduce interaction between vessel temperatures. You will derive a static decoupler and check whether cancelling steady cross effects also improves the transient response.

## The problem

The nominal gain matrix is $K=[[9/65,4/65],[4/65,9/65]]$ °C/point.
Separate PI loops correct cross effects after the other temperature has moved.
We can instead coordinate both heater requests from the known cross gains.
That steady compensation does not generally cancel transient interaction or
errors in the assumed coupling.

## What we need

We need to counter a known cross effect when a heater request changes,
rather than wait for the other vessel's temperature to move first.
The coordinated commands must remain consistent with actual actuator limits.

## The new idea

**Static decoupling** coordinates actuator commands to cancel predicted steady
cross effects. When one virtual controller asks for more heating, the map also
reduces the other heater to compensate for heat crossing the wall. "Static"
means this is an algebraic mapping of current requests, without additional
dynamic states in the map itself.

Choose virtual commands v so each has the compensated gain 1/9 °C per point derived in the pairing lesson. Require temperature deviations $(1/9)v$. The physical deviations u must satisfy

$$
K u=(1/9)I_2 v,
$$

so

$$
u=Dv,\qquad D=K^{-1}(1/9)I_2=\begin{bmatrix}1&-4/9\\-4/9&1\end{bmatrix}.
$$

Here $I_2$ is the two-by-two identity matrix, with ones on the diagonal and zeros elsewhere; multiplying a column by it leaves that column unchanged. A matrix-product entry is a row of the first matrix multiplied term by term with a column of the second and added. The equality $K D=(1/9)I_2$ can be checked directly; explicitly, the diagonal is $9/65-16/585=1/9$ and the cross term is $4/65-4/65=0$. For virtual $v_1=5,v_2=0$, physical $u_1=5,u_2=-2.222222$, and steady x1=.555556 °C, x2=0.

Virtual requests are deviations, initialized at zero. Initialize each virtual integral at zero too. The physical commands add the 75% holding bias after mapping: $u_{physical}=[75,75]^T+Dv$. Do not pass an absolute 75% integral through D. On physical clipping, the tracking error is in physical coordinates. Use $D^{-1}=(81/65)[[1,4/9],[4/9,1]]$ to convert $u_a-u_r$ back before updating the column $I_v$ of two virtual integral contributions:

$$
I_{v,next}=I_{v,trial}+\frac{dt}{T_t}D^{-1}(u_a-u_r).
$$

The inverse follows from the two-by-two rule: the determinant of D is $1-16/81=65/81$. Multiplying $D^{-1}D$ returns $I_2$, so the conversion restores the virtual correction that would produce the physical discrepancy. For a physical tracking error of $[-5,0]^T$ points, the corresponding virtual error is $[-6.230769,-2.769231]^T$ points; with dt=.1 s and Tt=1 s the integral corrections are −.623077 and −.276923 points.

At a mode change, preserve applied physical deviations by converting them with $D^{-1}$, then subtract the new virtual proportional outputs to initialize the virtual integrals. This is the same lifecycle principle as Lessons 25–28.

## Prediction

Calculate physical requests for a virtual +5-point step and predict why transient cancellation can fail even when steady cross error is zero. Predict which virtual requests can clip after the map.

## Implementation

Use the same dt=0.1 s, $T_t=1$ s, $\beta=1$, no filter, gains and requirements from Lesson 64. Start both vessels at 35 °C/75% and reset fully between the four target/load cases.

Write `Request1Pct := 75.0 + Virtual1 - (4.0 / 9.0) * Virtual2;` and the corresponding second row. Calculate virtual P/I/trial requests, physical requests, physical limit, and virtual-coordinate tracking. Keep virtual and physical requested/applied values separate and use the same gains in paired and decoupled comparisons.

## Experiment in Control Studio

1. Reset both vessels, virtual integrals to zero, map parameters to the nominal values, physical temperatures to 35 °C and outputs to 75%. Display temperatures, virtual/physical requests, applied outputs and tracking states.
2. Run Lesson 64’s two target directions and two D=10 kW load cases with paired PI, reset, then static decoupling. Start every 60 s clock at **Confirm and start**.
3. Record changed/unchanged requirements, transient peaks, final errors, clipping and continuity. Stop on bounds or invalid signals; zero heat and restart between cases.

## Evidence and decision

Decoupling qualifies only if it meets every unchanged Lesson 64 requirement with feasible applied commands. Report any transient worsening or clipping even when final cross error is smaller. The static map is nominal; Lesson 66 tests coupling variation.

## Conclusion

Static decoupling cancels nominal steady cross gains by mapping virtual commands into physical deviations. Correct scaling and physical-coordinate tracking are essential; a static inverse is not a dynamic inverse.

## Reason for the next lesson

The map used one nominal heat-transfer value. If actual coupling differs,
the compensation can be too small or too large. [Multivariable design selection](../65-robust-mimo-selection/README.md)
compares the same fixed paired and decoupled controllers across the declared
coupling cases before deciding which arrangement meets the requirements.
