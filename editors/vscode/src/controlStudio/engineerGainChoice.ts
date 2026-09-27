export interface EngineerGainPin {
  parameterSymbol: string;
  unit: string;
  loopRevision: string;
  modelRevision: string;
  value: number;
}
export interface EngineerGainChoice {
  mode: "automatic" | "manual";
  pin?: EngineerGainPin;
}

/** Stored input choices are distinct from candidate qualification and write approval. */
export function decodeEngineerGainChoice(value: unknown, modelRevision?: string): EngineerGainChoice | undefined {
  if (!record(value) || !keys(value, ["mode", "pin"]) || !["automatic", "manual"].includes(String(value.mode))) return undefined;
  if (value.pin === undefined) return value.mode === "automatic" ? { mode: "automatic" } : undefined;
  const pin = value.pin;
  if (!record(pin) || !keys(pin, ["parameterSymbol", "unit", "loopRevision", "modelRevision", "value"]) ||
    ![pin.parameterSymbol, pin.unit, pin.loopRevision, pin.modelRevision].every(text => typeof text === "string" && text.length > 0) ||
    pin.modelRevision !== modelRevision || typeof pin.value !== "number" || !Number.isFinite(pin.value)) return undefined;
  return { mode: value.mode as EngineerGainChoice["mode"], pin: { ...pin } as unknown as EngineerGainPin };
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function keys(value: Record<string, unknown>, names: string[]): boolean { return Object.keys(value).every(key => names.includes(key)); }
