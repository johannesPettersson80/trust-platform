# 63. Relative gain array

You will use the relative gain array to choose which heater should control each vessel temperature. You will explain how the other feedback loop changes the gain seen by each pairing.

## The problem

We measured each path with the other heater command fixed. Once the other
feedback loop acts to hold its temperature, that command changes too.
The original isolated gain may no longer describe the gain seen by the first
controller, so it is not enough to choose the pairing from that gain alone.

Lesson 62’s gain matrix is

$$
K=\begin{bmatrix}9/65&4/65\\4/65&9/65\end{bmatrix}\ \mathrm{°C/point}.
$$

An isolated gain holds the other heater command fixed. A compensated gain holds the other temperature fixed, so the other heater moves. These are different physical experiments.

## What we need

We need to judge how a proposed pairing's steady gain changes when the
other loop holds its output. That comparison must guide pairing before
we assume the two closed loops behave independently.

## The new idea

The **relative gain array**, or **RGA**, is a table of dimensionless ratios
comparing each input-output gain with the other inputs fixed against its gain
with the other outputs held fixed. We use it to judge controller pairings.
A value near one means little gain change; a negative value warns that the
effective direction reverses.

We first calculate that comparison from the physical two-vessel equations,
then collect the ratios in an array. It guides pairing, not transient tuning.

To hold $x_2=0$ while changing $v_1=5$, solve

$$
\frac4{65}v_1+\frac9{65}v_2=0,
$$

so $v_2=-(4/9)5=-2.222222$ points. Then $x_1=(9/65)5+(4/65)(-2.222222)=5/9=0.555556$ °C. Its effective gain is $1/9=0.111111$ °C/point. The isolated gain is $9/65=0.1384615$, so the relative gain is $0.1384615/0.111111=1.2461538$.

For $K=\begin{bmatrix}a&b\\c&d\end{bmatrix}$, eliminate $v_2$ from $x_1=av_1+bv_2$ and $x_2=cv_1+dv_2$. This gives $v_1=(dx_1-bx_2)/(ad-bc)$; eliminating $v_1$ gives $v_2=(-cx_1+ax_2)/(ad-bc)$. These two equations are the inverse matrix

$$
K^{-1}=\frac{1}{ad-bc}\begin{bmatrix}d&-b\\-c&a\end{bmatrix}=\begin{bmatrix}9&-4\\-4&9\end{bmatrix}\ \mathrm{points/°C}.
$$

Its denominator $ad-bc$ is called the determinant. Here it is $1/65$ in squared gain units, so the inverse exists. The superscript T below transposes a matrix by exchanging rows and columns. The relative gain array uses elementwise products $\Lambda=K\circ(K^{-1})^T$; $\circ$ means corresponding-element multiplication, not matrix multiplication. The diagonal products are $(9/65)(9)=81/65=1.2461538$; the crossed products are $(4/65)(-4)=-16/65=-.2461538$. Both are dimensionless. A negative cross value warns that holding the other output reverses the effective direction.

## Prediction

Calculate the compensated command and effective gain above, then repeat by symmetry. Predict why a fixed compensation command cancels only at equilibrium and cannot hold the other temperature during the transient without a controller.

## Implementation

Calculate inverse entries and array elements as scalar values with units. Keep PI off and expose isolated/compensated requests and applied commands. Write one ST calculation for the isolated gain, one for the compensated gain, and their guarded ratio. Choose the diagonal or crossed pairing and explain how the relative-gain signs support your choice.

## Experiment in Control Studio

Start at 35 °C/75%, PI off, and use +5-point steps with 30 s holds and full reset. Declare ±0.02 °C final comparison tolerance and the same 33–37 °C / 0–100% bounds before testing.

1. Reset 35 °C/75% and PI off. Run isolated +5-point heater-1 and heater-2 steps, 30 s each, from full reset.
2. Run matched compensated tests using the calculated fixed other-heater correction. Start each clock at **Confirm and start**.
3. Record applied commands, final temperatures and final 5 s averages. Stop on bounds or invalid signals; restore 75%, zero heat on abort, and restart.

## Evidence and decision

First compare the final measured temperature changes with their predicted values within ±.02 °C. Then calculate gains and relative-gain ratios, carrying that temperature uncertainty through the divisions. Do not compare a dimensionless ratio with a temperature tolerance. State that fixed compensation is a steady calculation, not transient regulation. Use the result to select candidate pairings; do not call it a PI qualification.

## Conclusion

Relative gain compares isolated and compensated physical effects. Solving the equations reveals why a positive isolated path can become negative under a held-output pairing.

## Reason for the next lesson

The steady pairing calculation does not establish how the two active loops
interact during a change. [Decentralized PI](../63-decentralized-pi/README.md)
runs both controllers together and checks the changed temperature as well
as the one intended to remain fixed.
