import type { GeneralModel } from "./generalModel";
import type { RuntimeVerificationRequest } from "./verificationClient";

/** Bounded subset of the existing native plant uncertainty contract. */
export interface EngineerScenarioEffect {
  id: string;
  target: "command" | "process_input" | "process_output" | "measurement" | "state";
  channel: number;
  mode: "add" | "multiply" | "replace" | "hold";
  startSeconds: number;
  endSeconds?: number;
  generator: "constant" | "uniform_white" | "pulse";
  value?: number;
  minimum?: number;
  maximum?: number;
  durationSeconds?: number;
}
export function withScenarioEffects(request: RuntimeVerificationRequest, model: GeneralModel,
  effects: readonly EngineerScenarioEffect[]): RuntimeVerificationRequest {
  if (!request.simulation_override || effects.length > 32 || new Set(effects.map(effect => effect.id)).size !== effects.length) throw new Error("Scenario effects require a selected plant and distinct bounded effect identities.");
  const tables = effects.map(effect => {
    const count = (effect.target === "process_input" || effect.target === "command") ? model.inputs.length : effect.target === "state"
      ? model.kind === "transfer-function" ? model.denominator.length - 1 : model.states.length : model.outputs.length;
    if (!/^[A-Za-z][A-Za-z0-9_-]*$/.test(effect.id) || !Number.isSafeInteger(effect.channel) || effect.channel < 0 || effect.channel >= count
      || !["command", "process_input", "process_output", "measurement", "state"].includes(effect.target)
      || !["add", "multiply", "replace", "hold"].includes(effect.mode) || effect.mode === "hold" && (effect.target === "state" || effect.target === "command")
      || !Number.isFinite(effect.startSeconds) || effect.startSeconds < 0 || effect.startSeconds >= request.duration_ms / 1000
      || effect.endSeconds !== undefined && (!Number.isFinite(effect.endSeconds) || effect.endSeconds <= effect.startSeconds || effect.endSeconds > request.duration_ms / 1000)) throw new Error("Scenario effect coordinates or observation interval are invalid.");
    const fields: Record<string, string | number> = { id: effect.id, target: effect.target, channel: effect.channel,
      mode: effect.mode, generator: effect.generator, start_s: effect.startSeconds };
    if (effect.endSeconds !== undefined) fields.end_s = effect.endSeconds;
    if (effect.generator === "constant" || effect.generator === "pulse") {
      if (!Number.isFinite(effect.value)) throw new Error("A constant/pulse scenario effect requires a finite value.");
      fields.value = effect.value!;
      if (effect.generator === "pulse") {
        if (!Number.isFinite(effect.durationSeconds) || effect.durationSeconds! <= 0) throw new Error("A pulse scenario requires a positive duration.");
        fields.duration_s = effect.durationSeconds!;
      }
    } else if (effect.generator === "uniform_white") {
      if (!Number.isFinite(effect.minimum) || !Number.isFinite(effect.maximum) || effect.minimum! >= effect.maximum!) throw new Error("Noise requires ordered finite bounds.");
      fields.minimum = effect.minimum!; fields.maximum = effect.maximum!;
    } else throw new Error("Unsupported native scenario generator.");
    return ["[[plants.uncertainties]]", ...Object.entries(fields).map(([key, value]) => `${key} = ${JSON.stringify(value)}`), ""].join("\n");
  });
  return { ...request, simulation_override: { ...request.simulation_override,
    simulation_toml: `${request.simulation_override.simulation_toml}\n${tables.join("\n")}` } };
}
