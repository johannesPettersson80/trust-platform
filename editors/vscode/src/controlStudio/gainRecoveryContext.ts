import { sameParameterValues, type ParameterStatus } from "./parameterApplication";

export interface GainRecoveryContext {
  origin: Pick<ParameterStatus, "program_revision" | "binding_revision" | "active_revision" | "values">;
  candidate: Readonly<Record<string, number>>;
}

export function decodeGainRecoveryContext(value: unknown): GainRecoveryContext | undefined {
  if (!record(value) || Object.keys(value).length !== 2 || !record(value.origin) || Object.keys(value.origin).length !== 4
    || !["program_revision", "binding_revision", "active_revision"].every(key => text(value.origin[key]))
    || !numbers(value.origin.values) || !numbers(value.candidate)
    || Object.keys(value.origin.values).length !== Object.keys(value.candidate).length
    || !Object.keys(value.origin.values).every(key => Object.prototype.hasOwnProperty.call(value.candidate, key))) return undefined;
  return structuredClone(value) as unknown as GainRecoveryContext;
}

export function matchesGainRecovery(current: ParameterStatus, context: GainRecoveryContext | undefined): boolean {
  return Boolean(context && current.program_revision === context.origin.program_revision
    && current.binding_revision === context.origin.binding_revision && current.previous_revision === context.origin.active_revision
    && current.previous_values && sameParameterValues(current.previous_values, context.origin.values)
    && sameParameterValues(current.values, context.candidate));
}
function record(value: unknown): value is Record<string, any> { return !!value && typeof value === "object" && !Array.isArray(value); }
function text(value: unknown): boolean { return typeof value === "string" && value.length > 0 && value.length <= 256; }
function numbers(value: unknown): value is Record<string, number> {
  return record(value) && Object.keys(value).length > 0 && Object.keys(value).length <= 16
    && Object.keys(value).every(text) && Object.values(value).every(item => typeof item === "number" && Number.isFinite(item));
}
