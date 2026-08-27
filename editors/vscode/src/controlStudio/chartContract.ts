export type ControlChartValue = number | null;
export type SampleQuality = "good" | "uncertain" | "stale" | "bad";
export type PlotProvenance = "measured" | "estimated" | "analytical" | "simulated";

export interface ControlChartEvent {
  kind: "event" | "mode-change" | "test-start" | "test-end" | "limit";
  label: string;
}

export interface ControlChartSample {
  elapsedSeconds: number;
  sp: ControlChartValue;
  pv: ControlChartValue;
  mv: ControlChartValue;
  quality?: SampleQuality;
  events?: readonly ControlChartEvent[];
}

export type ControlSaturationState = "minimum" | "maximum" | "none";

/** Units implied by the parallel-form Control.PID equations in Specification 32. */
export function pidEngineeringUnits(processUnit: string, outputUnit: string): {
  proportional: string;
  integral: string;
  derivative: string;
  output: string;
} {
  const process = processUnit.trim();
  const output = outputUnit.trim();
  if (!process || !output) {
    return {
      proportional: "normalized output/process",
      integral: "normalized output/(process·s)",
      derivative: "normalized output·s/process",
      output: "normalized output",
    };
  }
  return {
    proportional: `${output}/${process}`,
    integral: `${output}/(${process}·s)`,
    derivative: `${output}·s/${process}`,
    output,
  };
}

export function controlChartAxisLabels(processUnit: string, outputUnit: string): {
  process: string;
  output: string;
  description: string;
} {
  const process = processUnit.trim() || "engineering unit";
  const output = outputUnit.trim() || "engineering unit";
  return {
    process: `SP / PV (${process})`,
    output: `MV (${output})`,
    description: `SP and PV in ${process}; MV in ${output}`,
  };
}

export function controlChartAccessibleSummary(
  source: string,
  sample: ControlChartSample | undefined,
  processUnit: string,
  outputUnit: string
): string {
  const origin = source.trim() || "Control loop";
  if (!sample) return `${origin} control-loop time plot. No current samples.`;
  return `${origin} control-loop time plot. Current at ${formatEngineeringNumber(sample.elapsedSeconds)} s: ` +
    `SP ${formatEngineeringNumber(sample.sp)} ${processUnit}, ` +
    `PV ${formatEngineeringNumber(sample.pv)} ${processUnit}, ` +
    `MV ${formatEngineeringNumber(sample.mv)} ${outputUnit}.`;
}

/** Emits one marker when limiting starts or ends, never one per scan. */
export function controlChartSaturationEvent(
  previous: ControlSaturationState | undefined,
  current: ControlSaturationState
): ControlChartEvent | undefined {
  if (previous === current || (previous === undefined && current === "none")) return undefined;
  if (current === "none") {
    return previous === undefined ? undefined : {
      kind: "limit", label: `Output left ${previous} saturation`,
    };
  }
  return { kind: "limit", label: `Output entered ${current} saturation` };
}

export const CONTROL_CHART_SIGNALS = [
  { id: "sp", name: "SP", axis: 0, line: "solid" as const, symbol: "diamond" as const },
  { id: "pv", name: "PV", axis: 0, line: "solid" as const, symbol: "circle" as const },
  { id: "mv", name: "MV", axis: 1, line: "solid" as const, symbol: "rect" as const },
  { id: "disturbance", name: "Disturbance", axis: 1, line: "dotted" as const, symbol: "triangle" as const },
  { id: "current", name: "Current", axis: 0, line: "solid" as const, symbol: "circle" as const },
  { id: "candidate", name: "Candidate", axis: 0, line: "solid" as const, symbol: "diamond" as const },
  { id: "baseline", name: "Baseline", axis: 0, line: "dashed" as const, symbol: "rect" as const },
  { id: "limit", name: "Limit", axis: 0, line: [8, 4, 2, 4] as number[], symbol: "none" as const },
] as const;

export function controlChartLayout(width: number): {
  compact: boolean;
  grid: { left: number; right: number; top: number; bottom: number };
  showLegend: boolean;
  showAxisNames: boolean;
} {
  const compact = Number.isFinite(width) && width > 0 && width < 480;
  return compact
    ? { compact, grid: { left: 36, right: 36, top: 18, bottom: 34 }, showLegend: false, showAxisNames: false }
    : { compact, grid: { left: 58, right: 58, top: 38, bottom: 42 }, showLegend: true, showAxisNames: true };
}

export function normalizeChartWindow(start: number, end: number): [number, number] {
  const width = Math.min(100, Math.max(1, end - start));
  const normalizedStart = Math.max(0, Math.min(100 - width, start));
  return [normalizedStart, normalizedStart + width];
}

export function controlChartCsv(samples: readonly ControlChartSample[]): string {
  const rows = ["elapsed_seconds,sp,pv,mv,quality,events"];
  for (const sample of samples) {
    rows.push([
      sample.elapsedSeconds,
      csvValue(sample.sp),
      csvValue(sample.pv),
      csvValue(sample.mv),
      sample.quality ?? "good",
      csvText(sample.events?.map((event) => event.label).join(" | ") ?? ""),
    ].join(","));
  }
  return rows.join("\n");
}

export function restoredControlChartSamples(samples: readonly {
  elapsedSeconds: number;
  sp: number | null;
  pv: number | null;
  mv: number | null;
  quality: "good" | "forced" | "bad";
  events: readonly string[];
}[]): ControlChartSample[] {
  return samples.map((sample) => ({
    elapsedSeconds: sample.elapsedSeconds,
    sp: sample.sp,
    pv: sample.pv,
    mv: sample.mv,
    quality: sample.quality === "good" ? "good" : "bad",
    events: sample.events.map((label) => ({ kind: "event" as const, label })),
  }));
}

export function displayedControlSamples(
  rawSamples: readonly ControlChartSample[],
  targetPoints: number
): ControlChartSample[] {
  const samples = rawSamples.map(sanitizeSample);
  if (samples.length <= targetPoints || targetPoints < 3) return samples;

  const mandatory = new Set<number>([0, samples.length - 1]);
  for (let index = 1; index < samples.length; index += 1) {
    const sample = samples[index];
    const previous = samples[index - 1];
    if (sample.events?.length || sample.quality !== previous.quality) mandatory.add(index);
    if ([sample.sp, sample.pv, sample.mv].some((value) => value === null)) mandatory.add(index);
  }
  for (const signal of ["sp", "pv", "mv"] as const) {
    for (let index = 1; index < samples.length - 1; index += 1) {
      const before = samples[index - 1][signal];
      const value = samples[index][signal];
      const after = samples[index + 1][signal];
      if (before !== null && value !== null && after !== null &&
          ((value > before && value > after) || (value < before && value < after))) {
        mandatory.add(index);
      }
    }
  }

  const selected = new Set(mandatory);
  const available = Math.max(0, targetPoints - selected.size);
  for (let step = 1; step <= available; step += 1) {
    selected.add(Math.round((step * (samples.length - 1)) / (available + 1)));
  }
  return [...selected].sort((left, right) => left - right).map((index) => samples[index]);
}

export function validateLogValues(values: readonly number[]): {
  values: ControlChartValue[];
  invalidIndices: number[];
} {
  const invalidIndices: number[] = [];
  const validated = values.map((value, index) => {
    if (!Number.isFinite(value) || value <= 0) {
      invalidIndices.push(index);
      return null;
    }
    return value;
  });
  return { values: validated, invalidIndices };
}

export function wrapPhaseDegrees(value: number, convention: "-180..180" | "0..360"): number {
  if (!Number.isFinite(value)) throw new Error("Phase must be finite");
  if (convention === "0..360") return ((value % 360) + 360) % 360;
  const wrapped = ((value + 180) % 360 + 360) % 360 - 180;
  return wrapped === -180 && value > 0 ? 180 : wrapped;
}

export function frequencyPlotDescription(
  frequencyUnit: "Hz" | "rad/s",
  amplitudeUnit: "ratio" | "dB",
  provenance: PlotProvenance
): string {
  return `${capitalize(provenance)} frequency response; frequency in ${frequencyUnit}; amplitude in ${amplitudeUnit}; phase in degrees`;
}

/** Keeps process values readable without changing their full-precision source. */
export function formatEngineeringNumber(value: unknown, fractionDigits?: number): string {
  if (value === "unbounded") return "∞";
  const numeric = typeof value === "number"
    ? value
    : typeof value === "string" && value.trim() !== "" ? Number(value) : Number.NaN;
  if (numeric === Number.POSITIVE_INFINITY) return "∞";
  if (numeric === Number.NEGATIVE_INFINITY) return "−∞";
  if (!Number.isFinite(numeric)) return "—";
  if (Object.is(numeric, -0) || numeric === 0) return "0";
  const digits = fractionDigits === undefined
    ? automaticFractionDigits(Math.abs(numeric))
    : Math.max(0, Math.min(6, Math.floor(fractionDigits)));
  if (fractionDigits === undefined && Math.abs(numeric) < 0.001) return numeric.toExponential(2);
  if (Math.abs(numeric) < 10 ** -digits) return numeric.toExponential(2);
  return numeric.toFixed(digits).replace(/(\.\d*?[1-9])0+$|\.0+$/u, "$1");
}

export function compactRevisionLabel(value: string): string {
  const digest = value.startsWith("sha256:") ? value.slice("sha256:".length) : value;
  return digest.length > 12 ? `${digest.slice(0, 8)}…` : digest;
}

function automaticFractionDigits(magnitude: number): number {
  if (magnitude >= 1_000) return 0;
  if (magnitude >= 100) return 1;
  if (magnitude >= 1) return 2;
  if (magnitude >= 0.01) return 4;
  return 6;
}

function sanitizeSample(sample: ControlChartSample): ControlChartSample {
  return {
    ...sample,
    sp: finiteOrGap(sample.sp),
    pv: finiteOrGap(sample.pv),
    mv: finiteOrGap(sample.mv),
    quality: sample.quality ?? "good",
  };
}

function finiteOrGap(value: ControlChartValue): ControlChartValue {
  return value !== null && Number.isFinite(value) ? value : null;
}

function csvValue(value: ControlChartValue): string {
  return value === null || !Number.isFinite(value) ? "" : String(value);
}

function csvText(value: string): string {
  return `"${value.replace(/"/g, '""')}"`;
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}
