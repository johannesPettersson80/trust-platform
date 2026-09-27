# 69. Observability

You will determine whether a history of the downstream tank’s level can reveal the upstream level. You will calculate that relationship and explain why differentiating a noisy measurement gives an unreliable estimate.

## The problem

The pump can influence both tank levels, but the proposed installation
measures only the downstream level. We do not yet know whether that
measurement history contains enough information about the upstream water.
Even if it does, noise may make direct reconstruction unreliable.

## What we need

We need to determine whether the downstream measurement history contains
enough information to distinguish the hidden upstream level. We must also
separate theoretical information availability from useful noisy estimation.

## The new idea

**Observability** is the ability to determine a model's state from its
measured output over time when the input is known. Here upstream level
affects the downstream measurement's later motion, so its history may reveal
a state that is not directly measured.

Use the output row $C=[0,1]$, so $y=Cx=0x_1+1x_2=x_2$. Multiplying $C$ by $A$ selects the second row: $CA=[.04,-.04]$ s⁻¹. Thus the measurement gives one equation, and its time derivative gives a second. Stack those rows to obtain the observability matrix $O=[[0,1],[.04,-.04]]$. Its determinant is $0(-.04)-1(.04)=-.04$, so the rows give independent information. This property is called **observability**. Keep h .05–.45, q 0–.020, dt=1 s, and the initial deviation x1=.02,x2=0.

The output is $y=x_2$. The downstream volume balance gives:

$$
\dot y=.04x_1-.04y,\qquad x_1=y+25\dot y.
$$

The levels are in metres, $\dot y$ in m/s, and the factor 25 in seconds.
The measurement and its rate supply two independent equations, explaining
why the noiseless local model can distinguish both state components.

Run the diagnostic first with noiseless h2. Then add a deterministic noise sequence alternating −.005 and +.005 m to h2 at every second. A change from −.005 to +.005 m noise adds .01 m/s to a one-second derivative. Multiplying by 25 s produces a .25 m contribution to reconstructed x1 error, in addition to the current measurement’s .005 m error, demonstrating why an observer is preferable.

## Prediction

Calculate CA, determinant, and x1 reconstruction for the initial state: x2=0 and x2'=.0008 m/s gives x1=.02 m. Predict the noise-induced error scale.

## Implementation

In Control Studio's Tune pane, import `provided-model.json`, choose **Analyze model**, and read **State-space structure**. The tool displays C, CA and observability rank2. Check the displayed rows against your hand calculation; rank is not calculated in ST.

Open `src/Main.st`. `UpstreamFromDownstreamHistory` takes only measured downstream deviation and slope. The program primes the previous sample on the first frame and calculates no valid estimate until a one-second interval has been observed. `EstimateReady` and `SampleCount` mark usable estimates. Upstream truth is used only for error evaluation and simulation safety; it never seeds the estimator. `NoiseAmplitudeM` selects0 or.005m. Levels and estimates are never clipped to hide failure.

## Experiment in Control Studio

1. Restart with physical h1=.27 m, h2=.25 m and q=.010 m³/s. Initialize the previous h2 sample from the first reading. Wait for the next one-second sample before forming the derivative; no initial h1 estimate is supplied.
2. Run a noiseless 20 s case from x1=.02,x2=0,v=0, then a full-reset 20 s case with noise alternating −.005 and +.005 m on consecutive samples.
3. Display measured h2, derivative, reconstructed x1, `TruthDeviationM` and `EstimateErrorM`. Compare the estimated deviation with the true deviation, not with absolute tank height. Continue to 60 s only if evaluating recovery; restore q and fully reset.
4. Stop if a physical level leaves .05–.45 m, applied flow leaves 0–.020 m³/s, data become invalid, or time stops. Command zero pump flow, stop the simulator and reset; drainage continues while it runs.

**Current workflow gap.** The h1=.27m,h2=.25m start is not an equilibrium. Native tests execute a synchronized first scan; the fresh isolated-run UI must reset runtime and plant together before the complete hands-on journey is qualified. A running plant prepared for a later Confirm has already drifted, and this program rejects that initial mismatch.

## Evidence and decision

In Validate, use `validation-requirements.json` with each complete recorded run. Control Studio calculates RMS (`statistic: "rms"`) and peak absolute error (`"maximum"`) from the `estimate-error` channel over1–20s, excluding the initial frame without an estimate. The clean run should satisfy the small-error requirements; failure under the noisy case is the expected demonstration. Evaluate the runs separately: changing measurement noise changes the experiment conditions, so this is not a fair controller-candidate ranking.

The rank result supports noiseless observability. For errors $e_k$ across N samples, calculate root-mean-square error $\mathrm{RMSE}=\sqrt{(e_1^2+\cdots+e_N^2)/N}$ in metres and peak absolute error. Report both for clean and noisy records; the .25 m one-step noise scale explains degradation. Do not claim that observability guarantees useful noisy estimation.

## Conclusion

Tank 2 reveals tank 1 through the downstream slope, but direct differentiation amplifies measurement noise.

## Reason for the next lesson

Knowing that state information exists does not choose the control gains.
[State feedback](../68-state-feedback/README.md) first uses both measured levels
to design the pump response. The following [observer](../70-state-observer/README.md)
then estimates the missing upstream level for that controller.
