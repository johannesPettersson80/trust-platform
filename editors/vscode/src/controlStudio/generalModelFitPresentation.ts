import { experimentFromRun } from "./identificationExperiment";
import { eigenvalues } from "./numerics";
import { rootsOfRealPolynomial } from "./numerics/classicalAnalysis";
import { stateSpaceTransferPolynomials } from "./linearTransferModel";
import { lagOneCorrelation, type IdentifiedModelPresentation } from "./identification";
import type { GeneralModel } from "./generalModel";
import type { IdentifiedStateModelResult, IdentificationChannels } from "./identifiedStateModel";
import type { RuntimeVerificationResult } from "./verificationClient";
import type { ModelComparison } from "./modelComparison";
import type { ControlLoopDefinition } from "./loopDefinition";
import { connectedModelChannels } from "./loopChannels";

export type GeneralIdentificationFit = Pick<IdentifiedStateModelResult, "comparisons" | "validationPredictions" | "estimationPredictions">;

/** Present already-qualified predictions; no model refitting or second simulator. */
export function generalModelFitComparison(model: GeneralModel, fit: GeneralIdentificationFit,
  runs: readonly RuntimeVerificationResult[], loop: ControlLoopDefinition, recordingChannels?: IdentificationChannels): ModelComparison {
  const series = model.validation === "unvalidated" ? fit.estimationPredictions ?? [] : fit.validationPredictions;
  const connected = recordingChannels ? {
    inputs: recordingChannels.inputs.map(channel => ({ signal: { symbol: channel.id } })),
    outputs: recordingChannels.outputs.map(channel => ({ signal: { symbol: channel.id } })),
  } : connectedModelChannels(loop, model);
  const selected = recordingChannels ? 0 : connected.outputs.findIndex(output => output.signal.symbol === loop.signals.processVariable.symbol);
  if (selected < 0 || series.length !== model.outputs.length) throw new Error("Model-fit outputs do not match the selected model.");
  const validationErrors = fit.comparisons.find(comparison => comparison.adequate)?.normalizedErrors;
  const outputFits = series.map((series, index) => {
    const channel = model.outputs[index];
    const record = runs.find(run => run.run.id === series.recordingId)?.run;
    if (!record || series.modelRevision !== model.revision || series.channelId !== channel.id || series.unit !== channel.unit
      || series.points.length < 2) throw new Error("Independent model-fit evidence does not match its model, output or recording.");
    const inputs = record.samples.map(sample => sample.values[connected.inputs[0].signal.symbol]?.value)
      .filter((value): value is number => typeof value === "number" && Number.isFinite(value));
    const measured = series.points.map(point => point.measured);
    return { id: channel.id, unit: channel.unit, points: series.points,
      normalizedError: validationErrors?.[index], residualLagOneCorrelation: lagOneCorrelation(series.points.map(point => point.residual)),
      validityRange: { inputMinimum: Math.min(...inputs), inputMaximum: Math.max(...inputs),
        outputMinimum: Math.min(...measured), outputMaximum: Math.max(...measured) } };
  });
  const primary = outputFits[selected];
  const recording = runs.find(run => run.run.id === series[selected].recordingId)!.run;
  const experiment = experimentFromRun(loop, recording, recordingChannels);
  const domain = model.kind === "discrete-state-space" ? "z" : "s";
  let plane: IdentifiedModelPresentation["poleZeroPlane"] = { domain, scope: "Pole and zero analysis is unavailable for this model." };
  if (model.kind !== "transfer-function") {
    const eigen = eigenvalues(model.a, { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e12 });
    if (eigen.status === "qualified") {
      let zeros: ReturnType<typeof rootsOfRealPolynomial>;
      if (model.inputs.length === 1 && model.outputs.length === 1) {
        try { zeros = rootsOfRealPolynomial(stateSpaceTransferPolynomials(model).numerator); } catch { /* Keep qualified poles. */ }
      }
      plane = { domain, poles: eigen.values, ...(zeros ? { zeros } : {}),
        scope: model.physical ? "Poles of the local physical linearization; not global nonlinear stability."
          : "Poles of the identified model; not closed-loop stability." };
    }
  }
  const presentation: IdentifiedModelPresentation = {
    experiment,
    outputId: primary.id, inputChannels: model.inputs.map(({ id, unit }) => ({ id, unit })), outputFits, poleZeroPlane: plane,
    equation: model.physical ? `Identified ${model.physical.kind} balance`
      : model.kind === "discrete-state-space" ? "x[k+1] = A x[k] + B Δu[k]; Δy[k] = C x[k] + D Δu[k]"
        : "dx/dt = A x + B Δu; Δy = C x + D Δu",
    units: { input: model.inputs[0].unit, output: primary.unit, gain: `${primary.unit}/${model.inputs[0].unit}`, time: "s" },
    parameters: [...(model.kind !== "transfer-function" ? [{ name: "States", value: model.states.length, unit: "1" }] : []),
      ...(model.kind === "discrete-state-space" ? [{ name: "Sample period", value: model.sampleTimeSeconds!, unit: "s" }] : [])],
    poles: [], zeros: [], delaySeconds: model.delaySeconds,
    uncertainty: { label: model.validation === "unvalidated" ? "Provisional estimation fit; independent validation is pending." : "Independent prediction validation; statistical parameter intervals were not estimated." },
    estimation: model.validation === "unvalidated" ? { normalizedError: primary.normalizedError, residualLagOneCorrelation: primary.residualLagOneCorrelation } : {},
    validation: model.validation === "unvalidated" ? {} : { normalizedError: primary.normalizedError, residualLagOneCorrelation: primary.residualLagOneCorrelation },
    predictedResponse: primary.points, validityRange: primary.validityRange, warnings: [],
  };
  return { modelRevision: model.revision, presentation };
}
