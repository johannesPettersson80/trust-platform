export const PLANT_UNCERTAINTY_TARGETS = [
  "command", "process_input", "state", "process_output", "measurement", "parameter",
] as const;
export const PLANT_UNCERTAINTY_MODES = ["add", "multiply", "replace", "hold"] as const;
export const PLANT_UNCERTAINTY_GENERATORS = [
  "constant",
  "uniform_white",
  "pulse",
  "sine",
  "normal_approx_white",
  "colored_noise",
  "random_walk",
  "quantise",
  "clip",
  "backlash",
  "dropout",
] as const;

export type PlantUncertaintyTarget = typeof PLANT_UNCERTAINTY_TARGETS[number];
export type PlantUncertaintyMode = typeof PLANT_UNCERTAINTY_MODES[number];
export type PlantUncertaintyGenerator = typeof PLANT_UNCERTAINTY_GENERATORS[number];

export interface PlantUncertaintyDraft {
  enabled: boolean;
  id: string;
  target: PlantUncertaintyTarget;
  mode: PlantUncertaintyMode;
  generator: PlantUncertaintyGenerator;
  channel?: number;
  parameter?: string;
  start_s?: number;
  end_s?: number;
  minimum?: number;
  maximum?: number;
  value?: number;
  quantum?: number;
  mean?: number;
  standard_deviation?: number;
  correlation_time_s?: number;
  diffusion_per_sqrt_s?: number;
  rate_per_s?: number;
  duration_s?: number;
  duration_min_s?: number;
  duration_max_s?: number;
  dropout_policy?: "hold_last" | "replace" | "fault";
  replacement_value?: number;
  amplitude?: number;
  frequency_hz?: number;
  phase_rad?: number;
  bias?: number;
  width?: number;
}

export interface PlantUncertaintyIssue {
  row: number;
  field: string;
  message: string;
}

const GENERATOR_FIELDS: Record<PlantUncertaintyGenerator, readonly string[]> = {
  constant: ["value"],
  uniform_white: ["minimum", "maximum"],
  pulse: ["value", "duration_s"],
  sine: ["amplitude", "frequency_hz", "phase_rad", "bias"],
  normal_approx_white: ["mean", "standard_deviation"],
  colored_noise: ["standard_deviation", "correlation_time_s"],
  random_walk: ["diffusion_per_sqrt_s", "minimum", "maximum"],
  quantise: ["quantum"],
  clip: ["minimum", "maximum"],
  backlash: ["width"],
  dropout: ["rate_per_s", "duration_s", "dropout_policy", "replacement_value"],
};

export function plantUncertaintyGeneratorFields(generator: PlantUncertaintyGenerator): readonly string[] {
  return GENERATOR_FIELDS[generator];
}

const FIELD_LABELS: Readonly<Record<string, string>> = {
  id: "Effect ID",
  target: "Target",
  mode: "Mode",
  generator: "Generator",
  parameter: "Parameter",
  channel: "Channel",
  start_s: "Start (s)",
  end_s: "End (s)",
  value: "Value",
  minimum: "Minimum",
  maximum: "Maximum",
  duration_s: "Duration (s)",
  amplitude: "Amplitude",
  frequency_hz: "Frequency (Hz)",
  phase_rad: "Phase (rad)",
  bias: "Bias",
  mean: "Mean",
  standard_deviation: "Standard deviation",
  correlation_time_s: "Correlation time (s)",
  diffusion_per_sqrt_s: "Diffusion per √s",
  quantum: "Quantum",
  rate_per_s: "Dropout rate (/s)",
  dropout_policy: "Dropout action",
  replacement_value: "Replacement value",
  width: "Backlash width",
};

export function plantUncertaintyFieldLabel(field: string): string {
  return FIELD_LABELS[field] ?? field.replace(/_/g, " ");
}

export function createPlantUncertainty(sequence: number): PlantUncertaintyDraft {
  return {
    enabled: true,
    id: `effect_${sequence}`,
    target: "measurement",
    mode: "add",
    generator: "constant",
    channel: 0,
    start_s: 0,
    value: 0,
  };
}

export function movePlantUncertainty(
  rows: readonly PlantUncertaintyDraft[],
  index: number,
  delta: -1 | 1
): PlantUncertaintyDraft[] {
  const target = index + delta;
  if (index < 0 || index >= rows.length || target < 0 || target >= rows.length) return [...rows];
  const next = [...rows];
  [next[index], next[target]] = [next[target], next[index]];
  return next;
}

export function validatePlantUncertainties(
  rows: readonly PlantUncertaintyDraft[],
  modelParameters: readonly string[]
): PlantUncertaintyIssue[] {
  const issues: PlantUncertaintyIssue[] = [];
  const ids = new Set<string>();
  const add = (row: number, field: string, message: string) => issues.push({ row, field, message });
  rows.forEach((effect, row) => {
    if (!effect.enabled) return;
    if (!/^[A-Za-z][A-Za-z0-9_-]*$/.test(effect.id)) {
      add(row, "id", "Start with a letter; then use letters, numbers, _ or -.");
    } else if (ids.has(effect.id)) add(row, "id", "Choose an effect ID that is not already used.");
    ids.add(effect.id);
    if (effect.target === "parameter") {
      if (!effect.parameter || !modelParameters.includes(effect.parameter)) {
        add(row, "parameter", "Choose a parameter owned by the selected plant model.");
      }
    } else if (!Number.isSafeInteger(effect.channel) || (effect.channel ?? -1) < 0) {
      add(row, "channel", "Channel must be a nonnegative integer.");
    }
    if (effect.start_s !== undefined && effect.start_s < 0) add(row, "start_s", "Start time must be 0 s or greater.");
    if (effect.end_s !== undefined && effect.end_s < (effect.start_s ?? 0)) add(row, "end_s", "End time must not precede start time.");
    if (effect.mode === "hold" && !["process_input", "process_output", "measurement"].includes(effect.target)) {
      add(row, "mode", "Hold is available only for process input, process output, or measurement.");
    }
    if (["quantise", "clip"].includes(effect.generator) && effect.mode !== "replace") {
      add(row, "mode", "Quantisation and clipping require Replace mode.");
    }
    if (effect.generator === "dropout" && (effect.target !== "measurement" || effect.mode !== "replace")) {
      add(row, "generator", "Dropout requires Measurement and Replace mode.");
    }
    if (effect.generator === "backlash" && (effect.target !== "process_output" || effect.mode !== "replace")) {
      add(row, "generator", "Backlash requires Process output and Replace mode.");
    }
    const required = (field: keyof PlantUncertaintyDraft, label: string, positive = false) => {
      const value = effect[field];
      if (typeof value !== "number" || !Number.isFinite(value)) add(row, String(field), `Enter ${label}.`);
      else if (positive && value <= 0) add(row, String(field), `${label[0].toUpperCase()}${label.slice(1)} must be greater than 0 s.`);
    };
    // Hold captures the last accepted value; its generator is a contract
    // placeholder and must not create invisible numeric requirements.
    if (effect.mode !== "hold") switch (effect.generator) {
      case "constant": required("value", "a value"); break;
      case "uniform_white":
      case "clip":
        required("minimum", "a minimum"); required("maximum", "a maximum");
        if (effect.minimum !== undefined && effect.maximum !== undefined &&
          (effect.generator === "uniform_white" ? effect.minimum >= effect.maximum : effect.minimum > effect.maximum)) {
          add(row, "maximum", effect.generator === "uniform_white" ? "Maximum must be greater than minimum." : "Maximum must be greater than or equal to minimum.");
        }
        break;
      case "pulse": required("value", "a value"); required("duration_s", "duration", true); break;
      case "sine":
        required("amplitude", "an amplitude"); required("frequency_hz", "a frequency"); required("phase_rad", "a phase");
        if (effect.frequency_hz !== undefined && effect.frequency_hz < 0) add(row, "frequency_hz", "Frequency must be 0 Hz or greater.");
        break;
      case "normal_approx_white":
        required("standard_deviation", "a standard deviation");
        if ((effect.standard_deviation ?? 0) < 0) add(row, "standard_deviation", "Standard deviation must be 0 or greater.");
        break;
      case "colored_noise":
        required("standard_deviation", "a standard deviation"); required("correlation_time_s", "correlation time", true); break;
      case "random_walk": required("diffusion_per_sqrt_s", "a diffusion rate"); break;
      case "quantise": required("quantum", "a quantum"); if ((effect.quantum ?? 1) <= 0) add(row, "quantum", "Quantum must be greater than 0."); break;
      case "backlash":
        required("width", "a backlash width");
        if ((effect.width ?? 1) <= 0) add(row, "width", "Backlash width must be greater than 0.");
        break;
      case "dropout":
        required("rate_per_s", "a dropout rate"); required("duration_s", "duration", true);
        if (!effect.dropout_policy) add(row, "dropout_policy", "Choose what happens during dropout.");
        if (effect.dropout_policy === "replace") required("replacement_value", "a replacement value");
        break;
    }
  });
  return issues;
}

export function persistedPlantUncertainties(rows: readonly PlantUncertaintyDraft[]): Record<string, unknown>[] {
  return rows.filter((row) => row.enabled).map((row) => {
    const result: Record<string, unknown> = {
      id: row.id,
      target: row.target,
      mode: row.mode,
      generator: row.generator,
    };
    if (row.target === "parameter") result.parameter = row.parameter;
    else result.channel = row.channel;
    if (row.start_s !== undefined) result.start_s = row.start_s;
    if (row.end_s !== undefined) result.end_s = row.end_s;
    for (const field of plantUncertaintyGeneratorFields(row.generator)) {
      const value = row[field as keyof PlantUncertaintyDraft];
      if (value !== undefined && !(field === "replacement_value" && row.dropout_policy !== "replace")) result[field] = value;
    }
    return result;
  });
}
