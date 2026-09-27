import { decodeGeneralFeedback, type GeneralFeedbackDefinition, type GeneralFeedbackResult } from "./generalFeedback";
export interface SavedGeneralFeedback { definition: GeneralFeedbackDefinition; result: GeneralFeedbackResult }
export function isSavedGeneralFeedback(value: unknown, frequencies: readonly number[]): value is SavedGeneralFeedback {
  if (!record(value)) return false;
  const definition = decodeGeneralFeedback(value.definition); const result = value.result;
  if (!definition || !record(result) || result.status !== "qualified" || JSON.stringify(result.definition) !== JSON.stringify(definition)
    || typeof result.nyquistScope !== "string" || !Array.isArray(result.openLoop) || result.openLoop.length !== frequencies.length
    || !result.openLoop.every((p,i) => record(p) && p.frequencyRadPerSecond === frequencies[i]
      && [p.real,p.imaginary,p.magnitude,p.phaseRadians].every(finite))
    || !record(result.closedLoop) || !record(result.margins)) return false;
  if (result.sensitivity !== undefined && (!Array.isArray(result.sensitivity) || result.sensitivity.length !== frequencies.length
    || !result.sensitivity.every((p, i) => record(p) && p.frequencyRadPerSecond === frequencies[i]
      && [p.sensitivityMagnitudeDb, p.complementarySensitivityMagnitudeDb, p.controlSensitivityMagnitudeDb].every(finite)))) return false;
  if (result.nyquistStability !== undefined && (!record(result.nyquistStability)
    || (result.nyquistStability.status === "qualified"
      && ![result.nyquistStability.encirclementsCounterClockwise, result.nyquistStability.openLoopRightHalfPlanePoles,
        result.nyquistStability.closedLoopRightHalfPlanePoles, result.nyquistStability.minimumDistanceToMinusOne,
        result.nyquistStability.endpointMagnitude].every(finite))
    || (result.nyquistStability.status !== "qualified" && (result.nyquistStability.status !== "rejected" || typeof result.nyquistStability.reason !== "string")))) return false;
  const closed = result.closedLoop; const margins = result.margins;
  if (closed.status === "qualified") {
    if (!Array.isArray(closed.poles) || !closed.poles.every(p=>record(p)&&finite(p.real)&&finite(p.imaginary))
      || typeof closed.stable !== "boolean" || typeof closed.scope !== "string"
      || definition.plant.delaySeconds + definition.controller.delaySeconds > 0) return false;
  } else if (closed.status !== "unavailable" || typeof closed.reason !== "string") return false;
  if (margins.status === "estimated") return finite(margins.gainCrossoverRadPerSecond) && finite(margins.phaseMarginDegrees)
    && (margins.gainMarginDb === undefined || finite(margins.gainMarginDb)) && typeof margins.scope === "string";
  return margins.status === "unavailable" && typeof margins.reason === "string";
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function finite(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value); }
