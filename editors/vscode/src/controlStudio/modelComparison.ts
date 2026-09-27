import { decodeIdentificationExperiment } from "./identificationExperiment";
import type { GeneralModel } from "./generalModel";
import type { IdentifiedModelPresentation } from "./identification";

/** Display evidence stays bound to the exact selected model, never to Apply approval. */
export interface ModelComparison {
  modelRevision: string;
  presentation: IdentifiedModelPresentation;
}

export function decodeModelComparison(value: unknown, model: GeneralModel | undefined): ModelComparison | undefined {
  if (!record(value) || !model || model.source !== "fitted" || value.modelRevision !== model.revision) return undefined;
  const p = value.presentation;
  const output = record(p) && typeof p.outputId === "string" ? model.outputs.find(output => output.id === p.outputId) : model.outputs[0];
  if (!record(p) || typeof p.equation !== "string" || !record(p.units) || p.units.time !== "s"
    || p.units.input !== model.inputs[0]?.unit || p.units.output !== output?.unit || typeof p.units.gain !== "string"
    || !Array.isArray(p.parameters) || p.parameters.some(v => !record(v) || typeof v.name !== "string" || !finite(v.value) || typeof v.unit !== "string")
    || !numbers(p.poles) || !numbers(p.zeros) || !finite(p.delaySeconds) || p.delaySeconds < 0
    || !record(p.uncertainty) || typeof p.uncertainty.label !== "string"
    || !metrics(p.estimation) || !metrics(p.validation)
    || !Array.isArray(p.warnings) || p.warnings.some(v => typeof v !== "string")
    || !record(p.validityRange) || !["inputMinimum", "inputMaximum", "outputMinimum", "outputMaximum"].every(k => finite(p.validityRange[k]))
    || !Array.isArray(p.predictedResponse) || p.predictedResponse.length < 2 || p.predictedResponse.length > 20000) return undefined;
  if (p.experiment !== undefined) {
    const experiment = decodeIdentificationExperiment(p.experiment);
    if (!experiment || experiment.inputs.length !== model.inputs.length || experiment.outputs.length !== model.outputs.length
      || experiment.inputs.some((trace, index) => trace.unit !== model.inputs[index].unit)
      || experiment.outputs.some((trace, index) => trace.unit !== model.outputs[index].unit)) return undefined;
  }
  let previous = -Infinity;
  for (const point of p.predictedResponse) {
    if (!record(point) || !["timeSeconds", "measured", "predicted", "residual"].every(k => finite(point[k]))
      || point.timeSeconds < 0 || point.timeSeconds <= previous) return undefined;
    previous = point.timeSeconds;
  }
  if (p.poleZeroPlane !== undefined) {
    const plane = p.poleZeroPlane;
    const roots = (values: unknown) => Array.isArray(values) && values.every(value => record(value) && finite(value.real) && finite(value.imaginary));
    if (!record(plane) || plane.domain !== (model.kind === "discrete-state-space" ? "z" : "s") || typeof plane.scope !== "string"
      || plane.poles !== undefined && !roots(plane.poles) || plane.zeros !== undefined && !roots(plane.zeros)) return undefined;
  }
  if (p.outputFits !== undefined) {
    if (!Array.isArray(p.outputFits) || p.outputFits.length !== model.outputs.length || p.outputFits.some((fit, index) => {
      if (!record(fit) || fit.id !== model.outputs[index].id || fit.unit !== model.outputs[index].unit
        || !Array.isArray(fit.points) || fit.points.length < 2 || fit.points.length > 20000
        || !record(fit.validityRange) || !["inputMinimum", "inputMaximum", "outputMinimum", "outputMaximum"].every(key => finite(fit.validityRange[key]))) return true;
      return fit.points.some((point, index) => !record(point) || !["timeSeconds", "measured", "predicted", "residual"].every(key => finite(point[key]))
        || point.timeSeconds < 0 || index > 0 && point.timeSeconds <= fit.points[index - 1].timeSeconds);
    })) return undefined;
  }
  return JSON.parse(JSON.stringify(value)) as ModelComparison;
}

function record(v: unknown): v is Record<string, any> { return !!v && typeof v === "object" && !Array.isArray(v); }
function finite(v: unknown): v is number { return typeof v === "number" && Number.isFinite(v); }
function numbers(v: unknown): v is number[] { return Array.isArray(v) && v.every(finite); }
function metrics(v: unknown): boolean {
  return record(v) && (v.normalizedError === undefined || finite(v.normalizedError) && v.normalizedError >= 0)
    && (v.residualLagOneCorrelation === undefined || finite(v.residualLagOneCorrelation));
}
