import type { ManualProportionalDesign } from "./manualProportionalDesign";
import type { GeneralTransferModel } from "./generalModel";
import type { SimpleProportionalDesign, SimpleProportionalRequirements } from "./simpleControllerDesign";

/** Analytic error envelope for a delay-free integrator; the ST law has no hidden bias. */
export function designIntegratingProportionalController(
  model: GeneralTransferModel, task: SimpleProportionalRequirements,
): SimpleProportionalDesign {
  const rate = model.numerator[0] / model.denominator[0];
  if (!Number.isFinite(rate) || rate === 0) throw new Error("Integrating P design requires a finite nonzero rate gain.");
  const operatingInput = model.operatingPoint?.inputs[0] ?? 0;
  const load = Math.max(Math.abs(operatingInput), Math.abs(operatingInput - task.constantInputDisturbance));
  const transientBound = Math.abs(task.referenceChange) + task.maximumSteadyError;
  const minimumRate = Math.max(
    Math.abs(rate) * load / task.maximumSteadyError,
    Math.log(transientBound / task.settlingBand) / task.settlingTimeSeconds,
    Number.EPSILON,
  );
  const lowerRate = Math.min(rate * task.proportionalGainBounds.minimum, rate * task.proportionalGainBounds.maximum);
  const upperRate = Math.max(rate * task.proportionalGainBounds.minimum, rate * task.proportionalGainBounds.maximum);
  const decay = Math.max(minimumRate, lowerRate);
  if (!Number.isFinite(decay) || decay <= 0 || decay > upperRate) {
    throw new Error("Proportional gain bounds cannot satisfy the integrating plant error and settling requirements.");
  }
  const proportionalGain = decay / rate;
  const steadyErrorBound = load / Math.abs(proportionalGain);
  const settlingResidualBound = (Math.abs(task.referenceChange) + steadyErrorBound)
    * Math.exp(-decay * task.settlingTimeSeconds);
  if (![proportionalGain, steadyErrorBound, settlingResidualBound].every(Number.isFinite)) {
    throw new Error("Integrating P design is numerically unidentifiable within the declared bounds.");
  }
  return {
    status: "provisional", candidate: { applied: false, structure: "P", proportionalGain },
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` },
    formulas: {
      symbolic: ["G(s)=k/s; lambda=k*Kp>0; closed-loop pole=-lambda.",
        "u=Kp*(r-y); e_inf=(u0-d)/Kp; no controller bias is added.",
        "E_inf=max(|u0|,|u0-d|)/|Kp|; residual(T)<=(|deltaR|+E_inf)*exp(-lambda*T)."],
      substituted: [`k=${rate}; u0=${operatingInput}; Kp=${proportionalGain}; lambda=${decay}.`,
        `Steady error bound=${steadyErrorBound}; transient bound=${settlingResidualBound}.`],
    },
    prediction: { evidence: "model-predicted", steadyErrorBound, settlingResidualBound, closedLoopPole: -decay },
    actualSTVerify: true,
    scope: "Provisional P prediction for the selected delay-free integrating model; actual-ST Verify is required.",
  };
}

/** Keeps the engineer's bounded gain, including zero and unstable exploratory choices. */
export function evaluateManualIntegratingProportionalController(
  model: GeneralTransferModel, proportionalGain: number, task: SimpleProportionalRequirements,
): ManualProportionalDesign {
  const rate = model.numerator[0] / model.denominator[0];
  const decay = rate * proportionalGain;
  if (!Number.isFinite(rate) || rate === 0 || !Number.isFinite(decay)
      || (decay === 0 && proportionalGain !== 0)) throw new Error("Manual integrating prediction has numerical uncertainty.");
  const pole = decay === 0 ? 0 : -decay;
  const input = model.operatingPoint?.inputs[0] ?? 0;
  const load = Math.max(Math.abs(input), Math.abs(input - task.constantInputDisturbance));
  const steadyErrorBound = decay > 0 ? load / Math.abs(proportionalGain) : null;
  const settlingResidualBound = steadyErrorBound === null ? null
    : (Math.abs(task.referenceChange) + steadyErrorBound) * Math.exp(pole * task.settlingTimeSeconds);
  if ((steadyErrorBound !== null && !Number.isFinite(steadyErrorBound))
      || (settlingResidualBound !== null && !Number.isFinite(settlingResidualBound))) {
    throw new Error("Manual integrating prediction has numerical uncertainty.");
  }
  return {
    status: "exploratory", candidate: { applied: false, structure: "P", proportionalGain },
    selectedModel: { revision: model.revision, provenance: `declared-${model.source}-${model.validation}` },
    prediction: { evidence: "model-predicted", closedLoopPole: pole, steadyErrorBound, settlingResidualBound,
      ...(decay <= 0 ? { noAsymptoticBoundReason: "The selected pole is nonnegative; no asymptotic error bound is established." } : {}) },
    formulas: { symbolic: ["G(s)=k/s; pole=-k*Kp.", "u=Kp*(r-y); e_inf=(u0-d)/Kp; no hidden bias."],
      substituted: [`k=${rate}; Kp=${proportionalGain}; pole=${pole}; u0=${input}.`] },
    actualSTVerify: true,
    scope: "Exploratory integrating-model prediction. Actual-ST Verify remains required before application.",
  };
}
