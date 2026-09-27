import { formatEngineeringNumber } from "../chartContract";
import { modelKindLabel } from "../failureMessages";

interface EvidenceViewPorts {
  setText(selector: string, value: unknown, title?: string): void;
  setHidden(selector: string, hidden: boolean): void;
  replaceEvidenceRows(selector: string, rows: readonly (readonly [string, string])[]): void;
}

/** Reuses the existing Identify evidence rendering outside the workflow event router. */
export function createIdentificationEvidenceView({ setText, setHidden, replaceEvidenceRows }: EvidenceViewPorts) {
function renderIdentificationQualification(value: Record<string, unknown>): void {
  const metrics = value.metrics as Record<string, unknown> | undefined;
  if (!metrics) return;
  setHidden("[data-identify-qualification]", false);
  setHidden("[data-identify-details]", false);
  setText("[data-identify-samples]", `${String(metrics.retainedSampleCount)} / ${String(metrics.rawSampleCount)}`);
  setText("[data-identify-duration]", `${formatEngineeringNumber(metrics.durationSeconds, 3)} s`);
  setText("[data-identify-interval]", `${formatEngineeringNumber(metrics.medianSampleTimeSeconds, 4)} s`);
  setText("[data-identify-snr]", `${formatEngineeringNumber(metrics.signalToNoiseDb, 2)} dB`);
  replaceEvidenceRows("[data-identify-detail-rows]", [
    ["Input / output units", `${String(value.inputUnit)} / ${String(value.outputUnit)}`],
    ["Maximum interval deviation", `${formatEngineeringNumber(metrics.maximumSampleTimeDeviationPercent, 2)} %`],
    ["Timing gaps", String(metrics.gapCount)],
    ["Rejected quality samples", String(metrics.rejectedQualitySampleCount)],
    ["Clipped / saturated", `${String(metrics.clippedSampleCount)} / ${String(metrics.saturationSampleCount)}`],
    ["Baseline drift", `${formatEngineeringNumber(metrics.baselineDriftPerSecond, 5)} ${String(value.outputUnit)}/s`],
    ["Input / output span", `${formatEngineeringNumber(metrics.inputSpan, 5)} ${String(value.inputUnit)} / ${formatEngineeringNumber(metrics.outputSpan, 5)} ${String(value.outputUnit)}`],
    ["Excitation", String(metrics.excitation)],
    ["Coherence", String(metrics.coherence)],
    ["Independent validation", metrics.independentValidationAvailable === true ? "Available" : "Not yet available"],
  ]);
}

function renderIdentificationModel(model: Record<string, unknown>, fit?: Record<string, unknown>): void {
  setHidden("[data-identify-model-details]", false);
  const range = fit?.operatingRange as Record<string, unknown> | undefined;
  const warnings = Array.isArray(fit?.warnings) ? fit.warnings.map(String).join("; ") : "None";
  replaceEvidenceRows("[data-identify-model-rows]", [
    ["Structure", modelKindLabel(String(model.kind))],
    ["Gain", formatEngineeringNumber(model.gain, 6)],
    ["Time constants", [model.timeConstantSeconds, model.secondTimeConstantSeconds]
      .filter((item) => typeof item === "number").map((item) => `${formatEngineeringNumber(item, 5)} s`).join(" / ") || "None"],
    ["Dead time", `${formatEngineeringNumber(model.deadTimeSeconds ?? 0, 5)} s`],
    ["Normalized estimation error", formatEngineeringNumber(model.normalizedEstimationError ?? model.normalizedError, 5)],
    ["Normalized validation error", typeof model.normalizedValidationError === "number"
      ? formatEngineeringNumber(model.normalizedValidationError, 5) : "Not evaluated"],
    ["Validation RMSE", fit ? `${formatEngineeringNumber(fit.rootMeanSquareError, 5)} ${String(fit.outputUnit)}` : "Not evaluated"],
    ["Operating range", range
      ? `MV ${formatEngineeringNumber(range.inputMinimum, 5)}…${formatEngineeringNumber(range.inputMaximum, 5)}; PV ${formatEngineeringNumber(range.outputMinimum, 5)}…${formatEngineeringNumber(range.outputMaximum, 5)}`
      : "Not evaluated"],
    ["Stability", model.stable === true ? "Stable" : "Not stable"],
    ["Warnings", warnings || "None"],
  ]);
}

function renderIdentificationPresentation(presentation?: Record<string, unknown>): void {
  if (!presentation) return;
  const units = presentation.units as Record<string, unknown> | undefined;
  const estimation = presentation.estimation as Record<string, unknown> | undefined;
  const validation = presentation.validation as Record<string, unknown> | undefined;
  const uncertainty = presentation.uncertainty as Record<string, unknown> | undefined;
  const range = presentation.validityRange as Record<string, unknown> | undefined;
  const warnings = Array.isArray(presentation.warnings) ? presentation.warnings.map(String).join("; ") : "None";
  const plane = presentation.poleZeroPlane as { poles?: { real: number; imaginary: number }[]; zeros?: { real: number; imaginary: number }[]; domain: string } | undefined;
  const complex = (values: { real: number; imaginary: number }[] | undefined) => values === undefined ? "Not evaluated"
    : values.map(value => `${formatEngineeringNumber(value.real, 6)}${value.imaginary === 0 ? "" : ` ${value.imaginary < 0 ? "−" : "+"} ${formatEngineeringNumber(Math.abs(value.imaginary), 6)}i`}`).join(", ") || "None";
  const poles = plane ? `${complex(plane.poles)} (${plane.domain})` : Array.isArray(presentation.poles)
    ? presentation.poles.map((value) => formatEngineeringNumber(value, 6)).join(", ") : "Not evaluated";
  const zeros = plane ? complex(plane.zeros) : Array.isArray(presentation.zeros) && presentation.zeros.length > 0
    ? presentation.zeros.map((value) => formatEngineeringNumber(value, 6)).join(", ") : "None";
  const rows: Array<[string, string]> = [
    ["Equation", String(presentation.equation)],
    ["Input / output", `${String(units?.input)} / ${String(units?.output)}`],
    ["Poles", poles || "None"],
    ["Zeros", zeros],
    ["Delay", `${formatEngineeringNumber(presentation.delaySeconds, 5)} s`],
    ["Uncertainty", String(uncertainty?.label ?? "Not evaluated")],
    ["Estimation residual correlation (lag 1)", typeof estimation?.residualLagOneCorrelation === "number"
      ? formatEngineeringNumber(estimation.residualLagOneCorrelation, 4) : "Not evaluated"],
    ["Validation residual correlation (lag 1)", typeof validation?.residualLagOneCorrelation === "number"
      ? formatEngineeringNumber(validation.residualLagOneCorrelation, 4) : "Not evaluated"],
    ["Validity range", range
      ? `MV ${formatEngineeringNumber(range.inputMinimum, 5)}…${formatEngineeringNumber(range.inputMaximum, 5)}; PV ${formatEngineeringNumber(range.outputMinimum, 5)}…${formatEngineeringNumber(range.outputMaximum, 5)}`
      : "Not evaluated"],
    ["Warnings", warnings || "None"],
  ];
  replaceEvidenceRows("[data-identify-model-rows]", rows);
}

  return { renderIdentificationQualification, renderIdentificationModel, renderIdentificationPresentation };
}
