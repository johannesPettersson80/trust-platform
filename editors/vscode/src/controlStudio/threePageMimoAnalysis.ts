import { loopSignal } from "./loopChannels";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { WorkflowResultSample } from "./workflowResult";
import { identifyMimoStaticGain, relativeGainArray, staticDecoupler } from "./numerics";
import { oneToOnePairing, type MimoPairingDecision } from "./mimoPairing";

export interface MimoWindow { startSeconds: number; endSeconds: number }
export interface MimoSettledTrial { inputId: string; baseline: MimoWindow; settled: MimoWindow }
export interface MimoSettledAnalysisOptions { maximumUnexcitedInputDrift?: number }
export interface MimoChannelEvidence { id: string; symbol: string; unit: string }
export type { MimoPairingDecision } from "./mimoPairing";
export type RecordedMimoAnalysis =
  | { status: "rejected"; reason: string }
  | { status: "qualified"; inputIds: readonly string[]; outputIds: readonly string[]; inputs: readonly MimoChannelEvidence[]; outputs: readonly MimoChannelEvidence[]; trials: readonly MimoSettledTrial[]; sampleCount: number; gainMatrix: readonly (readonly number[])[]; rga: readonly (readonly number[])[]; decoupler: readonly (readonly number[])[]; decouplerConditionNumber: number; pairing: readonly MimoPairingDecision[]; inputRank: number; inputConditionNumber: number; residualNorm: number };

const tolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e12 };

/** Fit settled independent trials from declared channels; transient scans never become static gains. */
export function calculateRecordedMimoAnalysis(loop: ControlLoopDefinition, samples: readonly WorkflowResultSample[], trials: readonly MimoSettledTrial[], options: MimoSettledAnalysisOptions = {}): RecordedMimoAnalysis {
  const channels = loop.channels;
  if (!channels || channels.inputs.length < 2 || channels.inputs.length !== channels.outputs.length) return { status: "rejected", reason: "square-multichannel-binding-required" };
  if (channels.inputs.length > 8) return { status: "rejected", reason: "pairing-dimension-exceeds-bounded-analysis" };
  const inputIds = channels.inputs.map(channel => channel.modelChannelId), outputIds = channels.outputs.map(channel => channel.modelChannelId);
  if (trials.length !== inputIds.length || new Set(trials.map(trial => trial.inputId)).size !== trials.length || trials.some(trial => !inputIds.includes(trial.inputId))) return { status: "rejected", reason: "one-settled-trial-per-input-required" };
  const inputSignals = channels.inputs.map(channel => loopSignal(loop, channel.appliedSignalId ?? channel.signalId));
  const outputSignals = channels.outputs.map(channel => loopSignal(loop, channel.signalId));
  const times = samples.map(sample => sample.elapsedSeconds).filter(Number.isFinite);
  if (!times.length) return { status: "rejected", reason: "no-retained-samples" };
  const meanWindow = (window: MimoWindow, signals: readonly { symbol: string }[]) => {
    if (!validWindow(window) || window.startSeconds < Math.min(...times) || window.endSeconds > Math.max(...times)) return undefined;
    const selected = samples.filter(sample => sample.elapsedSeconds >= window.startSeconds && sample.elapsedSeconds <= window.endSeconds);
    if (!selected.length) return undefined;
    const values = signals.map(signal => selected.map(sample => sampleValue(loop, sample, signal.symbol)));
    if (values.some(series => series.length === 0 || series.some(value => value === undefined))) return undefined;
    return values.map(series => series.reduce<number>((sum, value) => sum + (value ?? 0), 0) / series.length);
  };
  // The excited input must hold its level inside each window: a window across the step would average both levels.
  const spanWindow = (window: MimoWindow, signal: { symbol: string }) => {
    const values = samples.filter(sample => sample.elapsedSeconds >= window.startSeconds && sample.elapsedSeconds <= window.endSeconds)
      .map(sample => sampleValue(loop, sample, signal.symbol)).filter((value): value is number => value !== undefined);
    return values.length ? Math.max(...values) - Math.min(...values) : 0;
  };
  const inputs: number[][] = [], outputs: number[][] = [], retainedWindows: MimoSettledTrial[] = [], retained = new Set<number>();
  for (const trial of trials) {
    const baselineInputs = meanWindow(trial.baseline, inputSignals), settledInputs = meanWindow(trial.settled, inputSignals);
    const baselineOutputs = meanWindow(trial.baseline, outputSignals), settledOutputs = meanWindow(trial.settled, outputSignals);
    if (!baselineInputs || !settledInputs || !baselineOutputs || !settledOutputs) return { status: "rejected", reason: "insufficient-quality-valid-mimo-samples" };
    const inputDelta = settledInputs.map((value, index) => value - baselineInputs[index]);
    const excited = inputIds.indexOf(trial.inputId);
    const step = Math.abs(inputDelta[excited]);
    if ([trial.baseline, trial.settled].some(window => spanWindow(window, inputSignals[excited]) > 0.01 * step)) return { status: "rejected", reason: "window-straddles-input-change" };
    if (options.maximumUnexcitedInputDrift !== undefined && (!Number.isFinite(options.maximumUnexcitedInputDrift) || options.maximumUnexcitedInputDrift < 0 || inputDelta.some((value, index) => index !== inputIds.indexOf(trial.inputId) && Math.abs(value) > options.maximumUnexcitedInputDrift!))) return { status: "rejected", reason: "unexcited-input-drift" };
    inputs.push(inputDelta); outputs.push(settledOutputs.map((value, index) => value - baselineOutputs[index])); retainedWindows.push(trial);
    samples.forEach((sample, index) => { if (sample.elapsedSeconds >= trial.baseline.startSeconds && sample.elapsedSeconds <= trial.settled.endSeconds) retained.add(index); });
  }
  const identified = identifyMimoStaticGain(inputs, outputs, tolerance) as { status: string; reason?: string; gainMatrix?: number[][]; inputRank?: number; inputConditionNumber?: number; residualNorm?: number };
  if (identified.status !== "qualified" || !identified.gainMatrix || !Number.isFinite(identified.inputRank) || !Number.isFinite(identified.inputConditionNumber) || !Number.isFinite(identified.residualNorm)) return { status: "rejected", reason: identified.reason ?? "mimo-gain-fit-rejected" };
  const rga = relativeGainArray(identified.gainMatrix, tolerance) as { status: string; reason?: string; matrix?: number[][] };
  if (rga.status !== "qualified" || !rga.matrix) return { status: "rejected", reason: rga.reason ?? "mimo-rga-rejected" };
  const decoupler = staticDecoupler(identified.gainMatrix, tolerance) as { status: string; reason?: string; matrix?: number[][]; conditionNumber?: number };
  if (decoupler.status !== "qualified" || !decoupler.matrix || !Number.isFinite(decoupler.conditionNumber)) return { status: "rejected", reason: decoupler.reason ?? "mimo-decoupler-rejected" };
  return { status: "qualified", inputIds, outputIds,
    inputs: channels.inputs.map(channel => { const signal = loopSignal(loop, channel.appliedSignalId ?? channel.signalId); return { id: channel.modelChannelId, symbol: signal.symbol, unit: signal.unit }; }),
    outputs: channels.outputs.map(channel => { const signal = loopSignal(loop, channel.signalId); return { id: channel.modelChannelId, symbol: signal.symbol, unit: signal.unit }; }),
    trials: retainedWindows, sampleCount: retained.size, gainMatrix: identified.gainMatrix, rga: rga.matrix, decoupler: decoupler.matrix,
    decouplerConditionNumber: decoupler.conditionNumber!, pairing: oneToOnePairing(inputIds, outputIds, rga.matrix), inputRank: identified.inputRank!, inputConditionNumber: identified.inputConditionNumber!, residualNorm: identified.residualNorm! };
}

function validWindow(window: MimoWindow): boolean { return Number.isFinite(window.startSeconds) && Number.isFinite(window.endSeconds) && window.startSeconds < window.endSeconds; }
function sampleValue(loop: ControlLoopDefinition, sample: WorkflowResultSample, symbol: string): number | undefined {
  const primaryId = symbol === loop.signals.processVariable.symbol ? "pv" : symbol === loop.signals.manipulatedVariable.symbol ? "mv" : symbol === loop.signals.setpoint?.symbol ? "sp" : undefined;
  const additional = primaryId ? undefined : sample.additional?.find(channel => loop.signals.additional?.find(candidate => candidate.id === channel.id)?.symbol === symbol || loop.signals.actuator?.symbol === symbol && channel.id === "actuator");
  const value = primaryId === "pv" ? sample.pv : primaryId === "mv" ? sample.appliedMv : primaryId === "sp" ? sample.sp : additional?.value;
  const quality = primaryId ? sample.quality : additional?.quality;
  return quality === "good" && typeof value === "number" && Number.isFinite(value) ? value : undefined;
}
