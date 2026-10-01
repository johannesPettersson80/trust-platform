export interface RelayCycle {
  peak: number;
  trough: number;
  periodSeconds: number;
}

export interface RelaySample {
  timeSeconds: number;
  processValue: number;
  output: number;
}

/** Derives one period only from consecutive measured high-output transitions. */
export function extractRelayCycles(input: {
  highOutput: number;
  lowOutput: number;
  samples: readonly RelaySample[];
}): RelayCycle[] {
  if (!Number.isFinite(input.highOutput) || !Number.isFinite(input.lowOutput) || input.lowOutput >= input.highOutput) {
    throw new Error("Relay output levels must be finite and ordered.");
  }
  if (input.samples.length < 3 || !input.samples.every((sample, index) => Number.isFinite(sample.timeSeconds)
      && Number.isFinite(sample.processValue) && Number.isFinite(sample.output)
      && (index === 0 || sample.timeSeconds > input.samples[index - 1].timeSeconds))) {
    throw new Error("Relay samples need finite strictly increasing measured times, process values and outputs.");
  }
  const tolerance = Math.min((input.highOutput - input.lowOutput) / 4,
    1e-6 * Math.max(1, Math.abs(input.highOutput), Math.abs(input.lowOutput)));
  const level = (output: number): "high" | "low" | undefined => Math.abs(output - input.highOutput) <= tolerance ? "high"
    : Math.abs(output - input.lowOutput) <= tolerance ? "low" : undefined;
  // Recording may start at the baseline or midway through the initial high leg.
  // A complete cycle starts only at an observed low-to-high transition.
  const highStarts: number[] = [];
  let previous: "high" | "low" | undefined;
  input.samples.forEach((sample, index) => {
    const current = level(sample.output);
    if (current === "high" && previous === "low") highStarts.push(index);
    if (current) previous = current;
  });
  if (highStarts.length < 2) throw new Error("Relay records need at least two measured high-output transitions.");
  return highStarts.slice(0, -1).map((start, cycle) => {
    const end = highStarts[cycle + 1];
    const segment = input.samples.slice(start, end + 1);
    if (segment.some(sample => level(sample.output) === undefined)) throw new Error("A relay cycle contains clipped or intermediate output values.");
    return { peak: Math.max(...segment.map(sample => sample.processValue)),
      trough: Math.min(...segment.map(sample => sample.processValue)),
      periodSeconds: input.samples[end].timeSeconds - input.samples[start].timeSeconds };
  });
}

/** Keeps measured cycle evidence and its unapplied PI arithmetic in one relay-only handoff. */
export function relayPiFromMeasuredRecord(input: {
  highOutput: number;
  lowOutput: number;
  samples: readonly RelaySample[];
}): RelayPiCalculation & { cycles: readonly RelayCycle[] } {
  const cycles = extractRelayCycles(input);
  return { ...calculateRelayPiCandidate({ highOutput: input.highOutput, lowOutput: input.lowOutput, cycles }), cycles };
}

/** Adapts the immutable runtime Test ledger only when its reviewed plan was a feedback relay. */
export function relayPiFromExperimentRecord(input: {
  plan: { kind: string; relayFeedback?: { highOutput: number; lowOutput: number } };
  samples: readonly { timeSeconds: number; input: number; output: number; quality: string }[];
}): RelayPiCalculation & { cycles: readonly RelayCycle[] } {
  if (input.plan.kind !== "relay" || !input.plan.relayFeedback) {
    throw new Error("Relay PI analysis requires a reviewed feedback-relay Test record.");
  }
  if (input.samples.some(sample => sample.quality !== "good")) {
    throw new Error("Relay PI analysis requires good-quality measured input/output samples.");
  }
  return relayPiFromMeasuredRecord({ highOutput: input.plan.relayFeedback.highOutput,
    lowOutput: input.plan.relayFeedback.lowOutput,
    samples: input.samples.map(sample => ({ timeSeconds: sample.timeSeconds, processValue: sample.output, output: sample.input })) });
}

export interface RelayPiCandidate {
  proportionalGain: number;
  integralTimeSeconds: number;
  integralGain: number;
  applied: false;
}

export type RelayPiCalculation =
  | { status: "qualified"; ultimateGain: number; periodSeconds: number; outputAmplitude: number; responseAmplitude: number; candidate: RelayPiCandidate }
  | { status: "not_qualified"; reason: string };

type RelayCycleQualification =
  | { status: "qualified"; outputAmplitude: number; responseAmplitude: number; periodSeconds: number }
  | { status: "not_qualified"; reason: string };

export type RelayTuningPresentation =
  | { status: "ready"; cycles: number; message: string }
  | { status: "not_qualified"; reason: string };

type RelayExperimentPlan = {
  kind: string;
  relayFeedback?: { highOutput: number; lowOutput: number };
};

type RelayExperimentSample = {
  timeSeconds: number;
  input: number;
  output: number;
  quality: string;
};

/** Owns the relay-Test evidence until the engineer explicitly requests its PI calculation. */
export class RelayTuningSession {
  private qualifiedRecord: { plan: RelayExperimentPlan; samples: readonly RelayExperimentSample[] } | undefined;

  reset(): void {
    this.qualifiedRecord = undefined;
  }

  augmentTerminalPayload(plan: RelayExperimentPlan | undefined, payload: unknown): unknown {
    this.reset();
    if (plan?.kind !== "relay" || !record(payload) || !record(payload.identification) ||
        !Array.isArray(payload.identification.samples)) return payload;
    const samples = payload.identification.samples as readonly RelayExperimentSample[];
    const presentation = relayTuningPresentation(plan, samples);
    if (presentation.status === "ready") this.qualifiedRecord = { plan, samples };
    return { ...payload, relayTuning: presentation };
  }

  calculateCandidate(): RelayPiCalculation {
    if (!this.qualifiedRecord) {
      return { status: "not_qualified", reason: "No qualified feedback-relay Test record is available. Run and review Relay Test first." };
    }
    return relayPiFromExperimentRecord(this.qualifiedRecord);
  }
}

/** Converts qualified feedback-relay cycles into an unapplied Ziegler–Nichols PI candidate. */
export function calculateRelayPiCandidate(input: {
  highOutput: number;
  lowOutput: number;
  cycles: readonly RelayCycle[];
}): RelayPiCalculation {
  const qualification = qualifyRelayCycles(input);
  if (qualification.status !== "qualified") return qualification;
  const ultimateGain = 4 * qualification.outputAmplitude / (Math.PI * qualification.responseAmplitude);
  const proportionalGain = 0.45 * ultimateGain;
  const integralTimeSeconds = qualification.periodSeconds / 1.2;
  return {
    status: "qualified",
    ultimateGain,
    periodSeconds: qualification.periodSeconds,
    outputAmplitude: qualification.outputAmplitude,
    responseAmplitude: qualification.responseAmplitude,
    candidate: { proportionalGain, integralTimeSeconds, integralGain: proportionalGain / integralTimeSeconds, applied: false },
  };
}

function relayTuningPresentation(plan: RelayExperimentPlan, samples: readonly RelayExperimentSample[]): RelayTuningPresentation {
  try {
    const cycles = relayCyclesFromExperimentRecord({ plan, samples });
    const qualification = qualifyRelayCycles({
      highOutput: plan.relayFeedback!.highOutput,
      lowOutput: plan.relayFeedback!.lowOutput,
      cycles,
    });
    if (qualification.status !== "qualified") return qualification;
    return {
      status: "ready",
      cycles: cycles.length,
      message: `${numberName(cycles.length)} measured feedback-relay cycles are ready. Calculate relay candidate to create an unapplied PI proposal.`,
    };
  } catch (error) {
    return { status: "not_qualified", reason: error instanceof Error ? error.message : String(error) };
  }
}

/** Qualifies measured relay evidence without calculating controller settings. */
function qualifyRelayCycles(input: {
  highOutput: number;
  lowOutput: number;
  cycles: readonly RelayCycle[];
}): RelayCycleQualification {
  if (input.cycles.length < 3) {
    return { status: "not_qualified", reason: "At least three complete feedback relay cycles are required." };
  }
  const outputAmplitude = Math.abs(input.highOutput - input.lowOutput) / 2;
  if (!Number.isFinite(outputAmplitude) || outputAmplitude <= 0) {
    return { status: "not_qualified", reason: "Relay output levels must have a finite non-zero separation." };
  }
  if (!input.cycles.every(cycle => Number.isFinite(cycle.peak) && Number.isFinite(cycle.trough) &&
      Number.isFinite(cycle.periodSeconds) && cycle.peak > cycle.trough && cycle.periodSeconds > 0)) {
    return { status: "not_qualified", reason: "Every relay cycle needs finite peak, trough and positive period evidence." };
  }
  const responseAmplitude = mean(input.cycles.map(cycle => (cycle.peak - cycle.trough) / 2));
  const periodSeconds = mean(input.cycles.map(cycle => cycle.periodSeconds));
  if (!Number.isFinite(responseAmplitude) || responseAmplitude <= 0 || !Number.isFinite(periodSeconds) || periodSeconds <= 0) {
    return { status: "not_qualified", reason: "Relay cycle amplitude or period is not identifiable." };
  }
  for (const [quantity, values, average] of [
    ["amplitude", input.cycles.map(cycle => (cycle.peak - cycle.trough) / 2), responseAmplitude],
    ["period", input.cycles.map(cycle => cycle.periodSeconds), periodSeconds],
  ] as const) {
    const spread = Math.max(...values) - Math.min(...values);
    if (spread / average > 0.1 + 32 * Number.EPSILON) {
      return { status: "not_qualified", reason: `Relay ${quantity} spread exceeds 10% of its mean. Record more settled cycles or use another permitted identification method.` };
    }
  }
  return { status: "qualified", outputAmplitude, responseAmplitude, periodSeconds };
}

function relayCyclesFromExperimentRecord(input: {
  plan: RelayExperimentPlan;
  samples: readonly RelayExperimentSample[];
}): readonly RelayCycle[] {
  if (input.plan.kind !== "relay" || !input.plan.relayFeedback) {
    throw new Error("Relay PI analysis requires a reviewed feedback-relay Test record.");
  }
  if (input.samples.some(sample => !isRelayExperimentSample(sample) || sample.quality !== "good")) {
    throw new Error("Relay PI analysis requires good-quality measured input/output samples.");
  }
  return extractRelayCycles({
    highOutput: input.plan.relayFeedback.highOutput,
    lowOutput: input.plan.relayFeedback.lowOutput,
    samples: input.samples.map(sample => ({
      timeSeconds: sample.timeSeconds,
      processValue: sample.output,
      output: sample.input,
    })),
  });
}

function isRelayExperimentSample(value: RelayExperimentSample): boolean {
  return Number.isFinite(value.timeSeconds) && Number.isFinite(value.input) &&
    Number.isFinite(value.output) && typeof value.quality === "string";
}

function numberName(value: number): string {
  const names = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
  const name = names[value] ?? String(value);
  return name.charAt(0).toUpperCase() + name.slice(1);
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function mean(values: readonly number[]): number {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}
