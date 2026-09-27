import type { GeneralModel } from "./generalModel";
import type { RunRequirement } from "./runValidation";
import type { RuntimeVerificationResult } from "./verificationClient";
import type { EngineerScenarioEffect } from "./engineerScenarioEffects";

export interface EngineerScenario {
  id: string;
  model?: GeneralModel;
  operatingOutputs?: readonly number[];
  effects?: readonly EngineerScenarioEffect[];
  initialValues: Readonly<Record<string, number>>;
  events: readonly { atMs: number; symbol: string; value: number }[];
  requirements: readonly RunRequirement[];
}
export interface EngineerScenarioResult { id: string; result: RuntimeVerificationResult }

/** Browser-safe structural contract; native effect and requirement compatibility is checked by the campaign owner. */
export function validEngineerScenarios(value: unknown): value is readonly EngineerScenario[] {
  if (!Array.isArray(value) || value.length > 24) return false;
  const record = (candidate: unknown): candidate is Record<string, unknown> => !!candidate && typeof candidate === "object" && !Array.isArray(candidate);
  const number = (candidate: unknown): candidate is number => typeof candidate === "number" && Number.isFinite(candidate);
  return value.every(scenario => record(scenario) && typeof scenario.id === "string" && !!scenario.id && scenario.id !== "target-response"
    && record(scenario.initialValues) && Object.entries(scenario.initialValues).every(([symbol, item]) => symbol.length > 0 && number(item))
    && Array.isArray(scenario.events) && scenario.events.length <= 12000 && scenario.events.every(event => record(event)
      && number(event.atMs) && event.atMs >= 0 && typeof event.symbol === "string" && !!event.symbol && number(event.value))
    && Array.isArray(scenario.requirements) && scenario.requirements.length > 0 && scenario.requirements.length <= 64)
    && new Set(value.map(scenario => scenario.id)).size === value.length;
}
