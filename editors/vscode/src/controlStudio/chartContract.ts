import { displayEngineeringUnit } from "./unitDisplay";

export { displayEngineeringUnit };

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
  additional?: readonly ControlChartAuxiliarySignal[];
}

export interface ControlChartAuxiliarySignal {
  id: string;
  channel: string;
  role: string;
  unit: string;
  value: ControlChartValue;
  quality: SampleQuality | "forced";
  displayName?: string;
}

export function controlChartSampleValue(sample: ControlChartSample, id: string): ControlChartValue {
  if (id === "sp" || id === "pv" || id === "mv") return sample[id];
  return sample.additional?.find(signal => signal.id === id)?.value ?? null;
}

export interface ControlChartEventRow {
  timeSeconds: number;
  kind: ControlChartEvent["kind"] | "quality";
  label: string;
}

export type ControlSaturationState = "minimum" | "maximum" | "none";
export type ControlWorkflowStage = "Monitor" | "Test" | "Identify" | "Tune" | "Validate";

export interface ControlPlotTab {
  id: string;
  available: boolean;
  reason?: string;
}

export interface ControlPlotEvidence {
  structureComparison?: boolean;
  manualParameters?: boolean;
  recordedResponses?: boolean;
  rootLocus?: boolean;
  samples?: boolean;
  recording?: boolean;
  model?: boolean;
  candidate?: boolean;
  constraints?: boolean;
  mimo?: boolean;
}

export interface MimoPlotEvidence {
  schemaVersion: 1;
  modelRevision: string;
  evidenceRevision: string;
  inputs: readonly { id: string; name: string; unit: string }[];
  outputs: readonly { id: string; name: string; unit: string }[];
  channelResponses: readonly {
    inputId: string;
    outputId: string;
    points: readonly (readonly [timeSeconds: number, value: number])[];
  }[];
  relativeGainArray: readonly (readonly number[])[];
  singularValues: readonly {
    frequencyRadPerSecond: number;
    values: readonly number[];
  }[];
}

/** Rejects partial or ambiguously ordered multivariable evidence before rendering it. */
export function decodeMimoPlotEvidence(value: unknown): MimoPlotEvidence | undefined {
  if (!plainRecord(value) || value.schemaVersion !== 1 || !nonempty(value.modelRevision) ||
      !nonempty(value.evidenceRevision) || !Array.isArray(value.inputs) || !Array.isArray(value.outputs) ||
      value.inputs.length < 2 || value.outputs.length < 2 || value.inputs.length !== value.outputs.length ||
      !value.inputs.every(namedChannel) || !value.outputs.every(namedChannel) ||
      !uniqueIds(value.inputs) || !uniqueIds(value.outputs) || !Array.isArray(value.channelResponses) ||
      !Array.isArray(value.relativeGainArray) || !Array.isArray(value.singularValues)) return undefined;
  const inputs = value.inputs as Array<{ id: string }>;
  const outputs = value.outputs as Array<{ id: string }>;
  if (value.channelResponses.length !== inputs.length * outputs.length) return undefined;
  const pairs = new Set<string>();
  for (const response of value.channelResponses) {
    if (!plainRecord(response) || !inputs.some((input) => input.id === response.inputId) ||
        !outputs.some((output) => output.id === response.outputId) || !Array.isArray(response.points) ||
        response.points.length === 0 || !response.points.every(finitePoint)) return undefined;
    pairs.add(`${response.outputId}\u0000${response.inputId}`);
  }
  if (pairs.size !== inputs.length * outputs.length ||
      !value.relativeGainArray.every((row) => Array.isArray(row) && row.length === inputs.length &&
        row.every((item) => typeof item === "number" && Number.isFinite(item))) ||
      value.relativeGainArray.length !== outputs.length ||
      !value.singularValues.every((point) => plainRecord(point) && positiveFinite(point.frequencyRadPerSecond) &&
        Array.isArray(point.values) && point.values.length === inputs.length &&
        point.values.every((item) => typeof item === "number" && Number.isFinite(item) && item >= 0))) return undefined;
  return copy(value) as unknown as MimoPlotEvidence;
}

const MODEL_REQUIRED = "Identify and accept a dynamic model first";
const CANDIDATE_REQUIRED = "Generate or select a controller candidate first";

/** Keeps every relevant plot discoverable while explaining missing evidence. */
export function controlStagePlots(stage: string, evidence: ControlPlotEvidence): ControlPlotTab[] {
  const plots = baseStagePlots(stage, evidence);
  if (evidence.recordedResponses && ["Monitor", "Tune", "Validate"].includes(stage)) {
    plots.push({ id: "recorded-responses", available: true });
  }
  return plots;
}

function baseStagePlots(
  stage: string,
  evidence: ControlPlotEvidence
): ControlPlotTab[] {
  const available = (id: string): ControlPlotTab => ({ id, available: true });
  const requires = (id: string, ready: boolean, reason: string): ControlPlotTab =>
    ready ? available(id) : { id, available: false, reason };
  switch (stage as ControlWorkflowStage) {
    case "Monitor":
      return [
        // Response remains the stable landing plot while its own empty state
        // explains that samples have not arrived yet.
        available("response"),
        available("signals"),
        requires("events", Boolean(evidence.samples || evidence.recording),
          "Start the runtime or open a retained recording first"),
      ];
    case "Test":
      return [
        requires("response", Boolean(evidence.samples), "Start the runtime to receive current samples"),
        requires("experiment", Boolean(evidence.recording), "Record or run a bounded experiment first"),
        requires("limits", Boolean(evidence.samples), "Start the runtime to evaluate configured limits"),
      ];
    case "Identify": {
      const recording = requires("recorded-response", Boolean(evidence.recording), "Record a test or import a saved recording first");
      const fitRequired = "Check a recording, then fit a model";
      if (!evidence.model) return [recording, requires("fit", false, fitRequired)];
      if (evidence.structureComparison) return [available("fit"), available("residuals"), ...(evidence.recording ? [recording] : [])];
      const plots = [
        requires("fit", Boolean(evidence.model), fitRequired),
        requires("residuals", Boolean(evidence.model), fitRequired),
        requires("validation-data", Boolean(evidence.model), fitRequired),
        requires("poles-zeros", Boolean(evidence.model), fitRequired),
        ...(evidence.recording ? [recording] : []),
      ];
      if (evidence.mimo) {
        plots.push(available("channel-responses"), available("rga"));
      }
      return plots;
    }
    case "Tune": {
      if (evidence.manualParameters && !evidence.model && !evidence.candidate) {
        return [available(evidence.recording ? "recorded-response" : "live-response")];
      }
      const plots = [
        requires("response", Boolean(evidence.candidate), CANDIDATE_REQUIRED),
        requires("bode", Boolean(evidence.model), MODEL_REQUIRED),
        requires("root-locus", Boolean(evidence.model || evidence.rootLocus), MODEL_REQUIRED),
        requires("nyquist", Boolean(evidence.model), MODEL_REQUIRED),
        requires("nichols", Boolean(evidence.candidate), CANDIDATE_REQUIRED),
        requires("sensitivity", Boolean(evidence.candidate), CANDIDATE_REQUIRED),
      ];
      if (evidence.mimo) {
        plots.push(requires("singular-values", Boolean(evidence.model), MODEL_REQUIRED));
        plots.push(requires("interaction", Boolean(evidence.model), MODEL_REQUIRED));
      }
      return plots;
    }
    case "Validate":
      if (!evidence.candidate && evidence.recording) return [available("recorded-response"), available("signals")];
      if (!evidence.candidate && evidence.samples) return [available("live-response")];
      return [
        requires("requirements", Boolean(evidence.candidate), CANDIDATE_REQUIRED),
        requires("response", Boolean(evidence.candidate), CANDIDATE_REQUIRED),
        requires("robustness", Boolean(evidence.candidate), CANDIDATE_REQUIRED),
        requires("constraints", Boolean(evidence.candidate && evidence.constraints),
          evidence.candidate ? "Declare validation constraints first" : CANDIDATE_REQUIRED),
        ...(evidence.recording ? [available("recorded-response")] : []),
      ];
    default:
      throw new Error(`Unknown Control Studio workflow stage: ${stage}`);
  }
}

export function selectControlPlot(
  tabs: readonly ControlPlotTab[],
  selected?: string
): string | undefined {
  const retained = tabs.find((tab) => tab.id === selected && tab.available);
  return retained?.id ?? tabs.find((tab) => tab.available)?.id ?? tabs[0]?.id;
}

/** Routes direct tabs onto the smallest shared plot surface that preserves their evidence. */
export function controlPlotView(stage: string, plot: string): string {
  if (["Identify", "Validate", "Tune"].includes(stage) && ["recorded-response", "live-response"].includes(plot)) return "response";
  if (stage === "Test" && (plot === "experiment" || plot === "limits")) return "response";
  if ((stage === "Tune" || stage === "Validate") && plot === "response") return "engineering-response";
  if (stage === "Validate" && (plot === "robustness" || plot === "constraints")) return "requirements";
  return plot;
}

/** Direct chart tools operate only on a plot backed by current evidence. */
export function directChartActionAvailability(plotAvailable: boolean): Record<string, boolean> {
  return {
    inspect: plotAvailable,
    "zoom-out": plotAvailable,
    "zoom-in": plotAvailable,
    reset: plotAvailable,
    expand: plotAvailable,
    "export-figure": plotAvailable,
  };
}

/** Keeps initial result-history synchronization neutral while preserving real failures. */
export function workflowResultStatusText(phase: string, reason?: unknown): string | undefined {
  if (phase === "history") return undefined;
  if (phase !== "failed") return undefined;
  return typeof reason === "string" && reason.trim()
    ? reason
    : "The workflow result operation failed.";
}

export interface ControlSignalLayoutGroup {
  unit: string;
  signalIds: string[];
  axis: number;
}

export function controlSignalLayout(
  signals: readonly { id: string; unit: string }[]
): { mode: "axes" | "lanes"; groups: ControlSignalLayoutGroup[] } {
  const groups: ControlSignalLayoutGroup[] = [];
  for (const signal of signals) {
    const unit = signal.unit.trim() || "engineering unit";
    const group = groups.find((candidate) => candidate.unit === unit);
    if (group) group.signalIds.push(signal.id);
    else groups.push({ unit, signalIds: [signal.id], axis: 0 });
  }
  const mode = groups.length > 2 ? "lanes" as const : "axes" as const;
  if (mode === "axes") groups.forEach((group, index) => { group.axis = index; });
  return { mode, groups };
}

export interface DesignHistory<T> {
  boundary: string;
  past: T[];
  current: T;
  future: T[];
  capacity: number;
}

export function createDesignHistory<T>(candidate: T, boundary: string, capacity = 50): DesignHistory<T> {
  if (!boundary.trim()) throw new Error("A design history boundary is required.");
  if (!Number.isInteger(capacity) || capacity < 1 || capacity > 200) {
    throw new Error("Design history capacity must be between 1 and 200.");
  }
  return { boundary, past: [], current: copy(candidate), future: [], capacity };
}

export function commitDesignEdit<T>(
  history: DesignHistory<T>,
  candidate: T,
  boundary: string
): DesignHistory<T> {
  assertHistoryBoundary(history, boundary);
  return {
    ...history,
    past: [...history.past, copy(history.current)].slice(-history.capacity),
    current: copy(candidate),
    future: [],
  };
}

export function undoDesignEdit<T>(history: DesignHistory<T>, boundary: string): DesignHistory<T> {
  assertHistoryBoundary(history, boundary);
  if (history.past.length === 0) return copy(history);
  return {
    ...history,
    past: history.past.slice(0, -1).map(copy),
    current: copy(history.past[history.past.length - 1]),
    future: [copy(history.current), ...history.future.map(copy)],
  };
}

export function redoDesignEdit<T>(history: DesignHistory<T>, boundary: string): DesignHistory<T> {
  assertHistoryBoundary(history, boundary);
  if (history.future.length === 0) return copy(history);
  return {
    ...history,
    past: [...history.past.map(copy), copy(history.current)].slice(-history.capacity),
    current: copy(history.future[0]),
    future: history.future.slice(1).map(copy),
  };
}

export interface DesignComparisonContext {
  modelRevision: string;
  evidenceRevision: string;
  method: string;
}

export interface PinnedDesign<T> {
  candidate: T;
  context: DesignComparisonContext;
}

export function pinComparableDesign<T>(
  pins: readonly PinnedDesign<T>[],
  candidate: T,
  context: DesignComparisonContext
): PinnedDesign<T>[] {
  const first = pins[0]?.context;
  if (first && first.modelRevision !== context.modelRevision) {
    throw new Error("Cannot compare a design from an incompatible model revision.");
  }
  if (first && first.evidenceRevision !== context.evidenceRevision) {
    throw new Error("Cannot compare a design from incompatible experiment evidence.");
  }
  if (first && first.method !== context.method) {
    throw new Error("Cannot compare a design produced by an incompatible numerical method.");
  }
  return [...pins.map(copy), { candidate: copy(candidate), context: copy(context) }].slice(-3);
}

export interface ControlChartSession {
  schemaVersion: 1;
  boundary: string;
  activeStage: ControlWorkflowStage;
  selectedPlots: Partial<Record<ControlWorkflowStage, string>>;
  selectedSignals: string[];
  window: [number, number];
  expanded: boolean;
  candidateRevision?: number;
  pinnedCandidateRevisions: number[];
  pinnedDesigns?: readonly {
    revision: number;
    candidate: Record<string, unknown>;
    context: DesignComparisonContext;
  }[];
}

export function decodeControlChartSession(value: unknown, boundary: string): ControlChartSession | undefined {
  if (!plainRecord(value) || value.schemaVersion !== 1 || value.boundary !== boundary ||
      !CONTROL_STAGES.includes(value.activeStage as ControlWorkflowStage) || !plainRecord(value.selectedPlots) ||
      !Object.entries(value.selectedPlots).every(([stage, plot]) =>
        CONTROL_STAGES.includes(stage as ControlWorkflowStage) && typeof plot === "string" && plot.length > 0) ||
      !Array.isArray(value.selectedSignals) || value.selectedSignals.length > 256 ||
      !value.selectedSignals.every((signal) => typeof signal === "string" && signal.length > 0) ||
      new Set(value.selectedSignals).size !== value.selectedSignals.length ||
      !Array.isArray(value.window) || value.window.length !== 2 ||
      !value.window.every((item) => typeof item === "number" && Number.isFinite(item)) ||
      typeof value.expanded !== "boolean" ||
      (value.candidateRevision !== undefined && !positiveInteger(value.candidateRevision)) ||
      !Array.isArray(value.pinnedCandidateRevisions) || value.pinnedCandidateRevisions.length > 3 ||
      !value.pinnedCandidateRevisions.every(positiveInteger) ||
      (value.pinnedDesigns !== undefined && (!Array.isArray(value.pinnedDesigns) ||
        value.pinnedDesigns.length > 3 || !value.pinnedDesigns.every((item) =>
          plainRecord(item) && positiveInteger(item.revision) && plainRecord(item.candidate) &&
          plainRecord(item.context) && typeof item.context.modelRevision === "string" &&
          typeof item.context.evidenceRevision === "string" && typeof item.context.method === "string")))) {
    return undefined;
  }
  const normalized = normalizeChartWindow(value.window[0] as number, value.window[1] as number);
  if (normalized[0] !== value.window[0] || normalized[1] !== value.window[1]) return undefined;
  return copy(value) as unknown as ControlChartSession;
}

const CONTROL_STAGES: readonly ControlWorkflowStage[] = ["Monitor", "Test", "Identify", "Tune", "Validate"];

/** Units implied by the parallel-form Control.PID equations in Specification 32. */
export function pidEngineeringUnits(processUnit: string, outputUnit: string): {
  proportional: string;
  integral: string;
  derivative: string;
  output: string;
} {
  const process = displayEngineeringUnit(processUnit);
  const output = displayEngineeringUnit(outputUnit);
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

export function controlChartAxisLabels(processUnit: string, outputUnit: string, topology: "closed-loop" | "open-loop" = "closed-loop"): {
  process: string;
  output: string;
  description: string;
} {
  const process = displayEngineeringUnit(processUnit) || "engineering unit";
  const output = displayEngineeringUnit(outputUnit) || "engineering unit";
  if (topology === "open-loop") return {
    process: `Measured output (${process})`, output: `Applied input (${output})`,
    description: `Measured output in ${process}; applied input in ${output}`,
  };
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
  outputUnit: string,
  topology: "closed-loop" | "open-loop" = "closed-loop"
): string {
  const origin = source.trim() || "Control loop";
  const plot = topology === "open-loop" ? "open-loop" : "control-loop";
  if (!sample) return `${origin} ${plot} time plot. No current samples.`;
  const process = displayEngineeringUnit(processUnit);
  const output = displayEngineeringUnit(outputUnit);
  if (topology === "open-loop") return `${origin} open-loop time plot. Current at ${formatEngineeringNumber(sample.elapsedSeconds)} s: ` +
    `Measured output ${formatEngineeringNumber(sample.pv)} ${process}, Applied input ${formatEngineeringNumber(sample.mv)} ${output}.`;
  return `${origin} control-loop time plot. Current at ${formatEngineeringNumber(sample.elapsedSeconds)} s: ` +
    `SP ${formatEngineeringNumber(sample.sp)} ${process}, ` +
    `PV ${formatEngineeringNumber(sample.pv)} ${process}, ` +
    `MV ${formatEngineeringNumber(sample.mv)} ${output}.`;
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

/** Converts retained sample annotations into the chronological event log shown beside the plot. */
export function controlChartEventRows(samples: readonly ControlChartSample[]): ControlChartEventRow[] {
  const rows: Array<ControlChartEventRow & { order: number }> = [];
  let previousQuality: SampleQuality | undefined;
  for (const sample of samples) {
    for (const event of sample.events ?? []) {
      rows.push({
        timeSeconds: sample.elapsedSeconds,
        kind: event.kind,
        label: readableEventLabel(event.label),
        order: rows.length,
      });
    }
    if (previousQuality !== undefined && sample.quality !== undefined && sample.quality !== previousQuality) {
      rows.push({
        timeSeconds: sample.elapsedSeconds,
        kind: "quality",
        label: `Data quality changed to ${sample.quality}`,
        order: rows.length,
      });
    }
    previousQuality = sample.quality ?? previousQuality;
  }
  return rows
    .sort((left, right) => left.timeSeconds - right.timeSeconds || left.order - right.order)
    .map(({ order: _order, ...row }) => row);
}

function readableEventLabel(label: string): string {
  if (!/^[a-z0-9]+(?:_[a-z0-9]+)+$/u.test(label)) return label;
  const words = label.split("_").join(" ");
  return `${words.charAt(0).toUpperCase()}${words.slice(1)}`;
}

export function controlOutputLimitLines(minimum: number, maximum: number): Array<{ name: string; value: number }> {
  if (!Number.isFinite(minimum) || !Number.isFinite(maximum) || minimum >= maximum) {
    throw new Error("Output minimum must be finite and lower than output maximum.");
  }
  return [{ name: "Output minimum", value: minimum }, { name: "Output maximum", value: maximum }];
}

/** Keeps MV in the actuator's declared engineering scale even when current data occupy a small span. */
export function controlOutputAxisRange(minimum: number, maximum: number): {
  minimum: number;
  maximum: number;
} {
  controlOutputLimitLines(minimum, maximum);
  return { minimum, maximum };
}

/** Frames model-fit plots around the measured variation without clipping it or forcing zero. */
export function identificationOutputAxisRange(values: readonly number[]): {
  minimum: number;
  maximum: number;
} {
  const finite = values.filter(Number.isFinite);
  if (finite.length === 0) throw new Error("Identification output range requires finite values.");
  const minimum = Math.min(...finite);
  const maximum = Math.max(...finite);
  const span = maximum - minimum;
  const padding = span > 0 ? span * 0.08 : Math.max(Math.abs(minimum) * 0.01, 1e-6);
  return { minimum: minimum - padding, maximum: maximum + padding };
}

/** Keeps the live SP/PV trace off the plot frame edge instead of autoscaling tight to the
 * current data, which reads as a trace "pinned to the ceiling" on the first live plot a
 * student sees (finding V-08). Returns undefined when there is no finite data yet, so a
 * caller can leave the axis on its default range rather than forcing a degenerate one. */
export function liveResponseAxisRange(values: readonly (number | null | undefined)[]): {
  minimum: number;
  maximum: number;
} | undefined {
  const finite = values.filter((value): value is number => typeof value === "number" && Number.isFinite(value));
  if (finite.length === 0) return undefined;
  const minimum = Math.min(...finite);
  const maximum = Math.max(...finite);
  const span = maximum - minimum;
  const magnitude = Math.max(Math.abs(minimum), Math.abs(maximum));
  // Subtracting two sensor values can leave a few ulps around a constant
  // difference. Frame that as a constant; keep every recorded value unchanged.
  const varying = span > 128 * Number.EPSILON * magnitude;
  const padding = varying ? span * 0.08 : magnitude === 0 ? 1 : magnitude * 0.05;
  return { minimum: minimum - padding, maximum: maximum + padding };
}

export const CONTROL_CHART_SIGNALS = [
  // SP is dashed (not solid) so it stays visible drawn on top of a coinciding solid PV
  // trace instead of being hidden underneath it (finding V-08).
  { id: "sp", name: "SP", axis: 0, line: "dashed" as const, symbol: "diamond" as const },
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
  const additional = [...new Map(samples.flatMap(sample =>
    (sample.additional ?? []).map(signal => [signal.id, signal] as const))).values()];
  const rows = [["elapsed_seconds,sp,pv,mv,quality,events", ...additional.flatMap(signal =>
    [csvText(`${signal.id} (${signal.unit})`), csvText(`${signal.id}.quality`)])].join(",")];
  for (const sample of samples) {
    rows.push([
      sample.elapsedSeconds,
      csvValue(sample.sp),
      csvValue(sample.pv),
      csvValue(sample.mv),
      sample.quality ?? "good",
      csvText(sample.events?.map((event) => event.label).join(" | ") ?? ""),
      ...additional.flatMap(signal => {
        const measured = sample.additional?.find(item => item.id === signal.id);
        return [csvValue(measured?.value ?? null), measured?.quality ?? "bad"];
      }),
    ].join(","));
  }
  return rows.join("\n");
}

export function restoredControlChartSamples(samples: readonly {
  elapsedSeconds: number;
  sp: number | null;
  pv: number | null;
  mv?: number | null;
  requestedMv?: number | null;
  appliedMv?: number | null;
  quality: "good" | "forced" | "bad";
  events: readonly string[];
  additional?: readonly ControlChartAuxiliarySignal[];
}[]): ControlChartSample[] {
  return samples.map((sample) => ({
    elapsedSeconds: sample.elapsedSeconds,
    sp: sample.sp,
    pv: sample.pv,
    // The chart represents what the actuator actually received. The optional
    // request remains separate evidence and must not replace applied output.
    mv: sample.appliedMv ?? sample.mv ?? null,
    quality: sample.quality === "good" ? "good" : "bad",
    events: sample.events.map((label) => ({ kind: "event" as const, label })),
    ...(sample.additional ? { additional: sample.additional.map(signal => ({ ...signal })) } : {}),
  }));
}

export function displayedControlSamples(
  rawSamples: readonly ControlChartSample[],
  targetPoints: number
): ControlChartSample[] {
  const samples = rawSamples.map(sanitizeSample);
  if (samples.length <= targetPoints || targetPoints < 3) return samples;

  const mandatory = new Set<number>([0, samples.length - 1]);
  const signalIds = ["sp", "pv", "mv", ...new Set(samples.flatMap(sample =>
    sample.additional?.map(signal => signal.id) ?? []))];
  for (let index = 1; index < samples.length; index += 1) {
    const sample = samples[index];
    const previous = samples[index - 1];
    if (sample.events?.length || sample.quality !== previous.quality) mandatory.add(index);
    if (signalIds.some(id => controlChartSampleValue(sample, id) === null ||
        controlChartSampleValue(previous, id) === null)) mandatory.add(index);
    if ((sample.additional ?? []).some(signal => signal.quality !==
        previous.additional?.find(before => before.id === signal.id)?.quality)) mandatory.add(index);
  }
  for (const signal of signalIds) {
    for (let index = 1; index < samples.length - 1; index += 1) {
      const before = controlChartSampleValue(samples[index - 1], signal);
      const value = controlChartSampleValue(samples[index], signal);
      const after = controlChartSampleValue(samples[index + 1], signal);
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

/** Validates an exact graphical edit without mutating the last valid design. */
export function parseGraphicalGain(
  text: string,
  minimum: number,
  maximum: number
): { ok: true; value: number } | { ok: false; reason: string } {
  const value = Number(text);
  if (!Number.isFinite(value) || value < minimum || value > maximum) {
    return { ok: false, reason: `Enter a finite value from ${minimum} to ${maximum}.` };
  }
  return { ok: true, value };
}

/** The same bucket of decimal places `formatEngineeringNumber` picks automatically for a
 * single value, exposed so an axis formatter can derive one shared digit count from an
 * axis's overall span instead of from each tick's coincidental magnitude. */
export function automaticFractionDigits(magnitude: number): number {
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
    ...(sample.additional ? { additional: sample.additional.map(signal => ({ ...signal,
      value: ["bad", "forced", "stale"].includes(signal.quality) ? null : finiteOrGap(signal.value),
    })) } : {}),
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

function assertHistoryBoundary<T>(history: DesignHistory<T>, boundary: string): void {
  if (history.boundary !== boundary) {
    throw new Error("Design history boundary does not match the active loop and model.");
  }
}

function copy<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function plainRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function namedChannel(value: unknown): boolean {
  return plainRecord(value) && nonempty(value.id) && nonempty(value.name) && nonempty(value.unit);
}

function uniqueIds(values: unknown[]): boolean {
  const ids = values.map((value) => (value as { id: string }).id);
  return new Set(ids).size === ids.length;
}

function finitePoint(value: unknown): boolean {
  return Array.isArray(value) && value.length === 2 && value.every((item) =>
    typeof item === "number" && Number.isFinite(item));
}

function nonempty(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function positiveFinite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

function positiveInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0;
}
