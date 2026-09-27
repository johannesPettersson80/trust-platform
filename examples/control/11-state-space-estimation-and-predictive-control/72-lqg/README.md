# 74. LQG control

You will combine the two-tank state-feedback controller with a Kalman filter. You will test how estimating the upstream level changes level regulation and pump demand compared with using both levels directly.

## The problem

State feedback uses both level deviations, but the proposed installation measures only the downstream tank. The Kalman filter can estimate the missing upstream level. The remaining question is whether those estimates are accurate enough for the controller to meet the same requirements.

Complete [state estimation](../71-kalman-filter/README.md) and [LQR design](../69-lqr/README.md) before this lesson. Keep the same two gravity tanks, local model and .25 m operating level.

## What we need

We need to run the state-feedback controller when only the downstream level
is measured. We must connect the estimator and controller correctly and
establish what relying on estimates costs in level regulation and pump demand.

## The new idea

**Linear-quadratic-Gaussian control**, or **LQG**, combines LQR state feedback
with a Kalman state estimator. LQR tells us how to use the state to choose
control action; the filter supplies an estimate when all states are not
directly measured. We now connect those two calculations in the actual update order.

Use the one-second prediction model and uncertainty assumptions:

$$
A_d=\begin{bmatrix}.96&0\\.04&.96\end{bmatrix},\quad B_d=\begin{bmatrix}2\\0\end{bmatrix},\quad C=\begin{bmatrix}0&1\end{bmatrix},
$$

$$
Q_w=\operatorname{diag}(10^{-6},10^{-6})\ \mathrm{m^2},\quad R_n=25\times10^{-6}\ \mathrm{m^2}.
$$

Here $Q_w$ is the covariance added per one-second prediction. Use the 20-step LQR design with level weight $Q_x=400I$, pump weight $R_u=40000$, and gain $K=[.0787402564,.0238743039]$ m²/s. The control weights penalize errors and effort; the covariance values describe assumed estimation uncertainty.

Start with physical levels .27 m and .25 m, so $x=[.02,0]^T$. Initialize estimate $\hat x=[0,0]^T$ and covariance $P=10^{-4}I$. The nominal flow is .010 m³/s. Require both physical level deviations within ±.002 m from 50–60 s, applied flow 0–.020 m³/s, and physical levels .05–.45 m.

Use estimated-state feedback $v=-K\hat x$, then command absolute flow $q=.010+v$. Compare it with a full-state reference $v=-Kx$. The reference has both levels; it lets us measure the cost of relying on one sensor and a model. Simulation truth is not an input to the estimated-state controller.

**LQG** means linear-quadratic-Gaussian: linear dynamics, quadratic control cost, and Gaussian noise assumptions. Gaussian noise has a bell-shaped distribution described by its mean and variance. Under the appropriate unconstrained linear assumptions, state-feedback and estimator designs can be combined using the separation principle: their stable error modes can be designed separately. A finite horizon, bounded pump, nonlinear tank model and deterministic noise record must still be assessed on their actual terms; naming the arrangement LQG does not establish optimality for those conditions.

At time zero, the full-state reference requests $.010-.0787402564(.02)=.008425195$ m³/s. The estimated-state controller initially requests .010 m³/s because its initialized estimate is zero. It cannot immediately respond to an upstream deviation that the downstream measurement has not yet revealed.

From the next sample onward, first predict the state and covariance using the **previous applied** pump deviation. Then correct with the current downstream measurement. Finally calculate the new pump request from the corrected estimate. Using the requested flow in prediction when the actuator applied a different flow would introduce an avoidable model error.

## Prediction

Calculate the two initial commands. Explain why their difference follows from available information rather than different controller gains. Predict how sensor noise can move the estimated pump command, and identify which traces distinguish estimation error from actual level error.

## Implementation

Write the integration of the existing Kalman update and state-feedback calculation. Keep `PreviousAppliedFlowM3PerS`, the two estimate entries, four covariance entries, and the two gains in named state or parameter records. After prediction and correction, calculate

```st
RequestedFlowM3PerS := 0.010
    - Gain1 * StateHat[1]
    - Gain2 * StateHat[2];
```

Limit the complete flow request, apply it, and retain that applied value for the next prediction. Do not reinitialize the estimate at every scan. On invalid or stale downstream data, freeze the estimate and covariance, command zero pump flow, and require deliberate reinitialization before resuming. Zero inflow allows gravity drainage; stop the simulator rather than treating it as a level-holding mode.

## Experiment in Control Studio

1. Reset to the stated physical levels, zero estimate and initial covariance. At **Confirm and start**, begin a 60 s record and apply the initial command calculated from each controller’s available state. The first predict/measurement/correct cycle occurs at 1 s.
2. Run the full-state reference and estimated-state controller in separate full resets. Use the same gains, one-second clock, physical model, requirements and actuator limits.
3. Add the same sequence of sensor errors, $[-.005,+.005,+.005,-.005]$ m repeated once per second, to each run’s own downstream level measurement. Reuse the additive errors, not the complete measured-level record: different pump commands create different physical trajectories. The full-state reference uses ideal measured levels; the Kalman filter uses only noisy tank 2.
4. Display physical levels, estimated levels, level errors, estimation errors, innovation, covariance, requested/applied flow and clipping. Compare the late error requirement, estimation RMSE, and the same unweighted sums of squared pump deviations for both controllers.
5. Abort on physical level or applied-flow violations, invalid data or clock, or failed fallback. Command zero inflow, stop the simulator if needed, and reset plant and estimator before repeating. Assess any planned stale-data test separately from these two valid-data records.

## Evidence and decision

Judge level regulation, estimation accuracy and pump demand separately. The full-state reference is a useful benchmark; it does not demonstrate what one physical sensor can observe instantly. If estimated-state control fails the fixed requirement, identify whether the early hidden-state error, noise weighting, local-model error or actuator limit accounts for the failure.

The repeating bounded noise sequence is a reproducible comparison. It has temporal correlation and is not a sample of independent Gaussian noise, so it does not verify the Gaussian optimality assumptions.

## Conclusion

Estimated-state control connects a model and one measurement to a controller that needs both levels. Correct update order and actual applied-flow feedback preserve that connection; matched experiments establish its practical cost and accuracy.

## Reason for the next lesson

The combined controller calculates a command from estimated state, but it
does not explicitly plan around future shared-resource limits. In the next
application, two heaters must share one power supply.
[Model predictive control](../73-mpc/README.md) predicts candidate command
plans, rejects those that violate constraints and applies the first selected move.
