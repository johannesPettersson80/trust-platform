import { decodeStateDesignDraft, decodeStateDesignPreferences, type StateDesignPreferences, type StateDesignDraft } from "./stateDesignPreferences";
export interface ControllerTuningPreferences {
  method: "automatic" | "manual" | "SIMC" | "AMIGO" | "state-design" | "state-space" | "relay";
  values?: Readonly<Record<string, number>>;
  stateDesign?: StateDesignPreferences;
  stateDesignDraft?: StateDesignDraft;
}

/** Saved editing preferences carry no validation or runtime-write authority. */
export function controllerTuningPreferences(value: unknown): ControllerTuningPreferences | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const input = value as Record<string, unknown>;
  if (!["automatic", "manual", "SIMC", "AMIGO", "state-design", "state-space", "relay"].includes(String(input.method)) || Object.keys(input).some(key => !["method", "values", "stateDesign", "stateDesignDraft"].includes(key))) return undefined;
  if (input.values !== undefined && (!input.values || typeof input.values !== "object" || Array.isArray(input.values)
    || Object.keys(input.values).length > 64 || Object.entries(input.values).some(([key, number]) => !key || typeof number !== "number" || !Number.isFinite(number)))) return undefined;
  const stateDesign = input.stateDesign === undefined ? undefined : decodeStateDesignPreferences(input.stateDesign);
  if (input.stateDesign !== undefined && !stateDesign) return undefined;
  if (input.stateDesignDraft !== undefined && !decodeStateDesignDraft(input.stateDesignDraft)) return undefined;
  return structuredClone({ ...value, ...(stateDesign ? { stateDesign } : {}) }) as ControllerTuningPreferences;
}
