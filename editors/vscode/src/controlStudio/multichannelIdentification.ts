import * as path from "path";
import type { IdentifiedStateModelResult } from "./identifiedStateModel";
import type { GeneralIdentificationFit } from "./generalModelFitPresentation";
import type { ControlLoopDefinition } from "./loopDefinition";
import { loopSignal } from "./loopChannels";
import type { BoundedAnalysisJob } from "./boundedAnalysisJob";
import type { GeneralModel } from "./generalModel";
import type { RuntimeVerificationClient, RuntimeVerificationResult } from "./verificationClient";
import type { RunRequirement } from "./runValidation";
import { generatePrbs, generateMultisine } from "./numerics";

/** Excites independent channels and records every cross-response in each frozen run. */
export async function acquireMultichannelModel(loop: ControlLoopDefinition, runtime: RuntimeVerificationClient, fitting: BoundedAnalysisJob,
  cancelled: () => boolean, retain: (run: RuntimeVerificationResult) => void, progress: (text: string) => Promise<void>): Promise<{ model: GeneralModel; runs: readonly RuntimeVerificationResult[]; fit: GeneralIdentificationFit }> {
  const channels = loop.channels, mode = loop.parameterBinding?.mode;
  if (!channels || !mode) throw new Error("Multichannel identification needs ordered channel and mode bindings.");
  if (loop.experiment?.kind === "relay") throw new Error("A multichannel relay campaign requires explicit feedback pairing; select independent PRBS or multisine excitation.");
  const inputs = channels.inputs.map(channel => ({ ...channel, signal: loopSignal(loop, channel.signalId) }));
  const outputs = channels.outputs.map(channel => ({ ...channel, signal: loopSignal(loop, channel.signalId),
    reference: channel.setpointSignalId ? loopSignal(loop, channel.setpointSignalId) : undefined }));
  const manualCommands = inputs.map((input, index) => input.manualOutputSignal ?? (index === 0 ? loop.controller.manualOutputSignal : undefined));
  const closedLoop = loop.experiment?.kind === "setpoint_step" || manualCommands.some(command => !command);
  if (closedLoop && ["mv_step", "mv_pulse"].includes(loop.experiment?.kind ?? "")) throw new Error("The selected Manual-input method requires every Manual command binding; it cannot be substituted with reference excitation.");
  const references = outputs.filter(output => output.reference).map(output => output.reference!);
  if (closedLoop && references.length < inputs.length) throw new Error("Independent multichannel acquisition needs one controllable excitation per plant input; declare Manual commands or additional references.");
  const observed = [...new Map([...inputs.map(input => input.signal), ...outputs.map(output => output.signal), ...references]
    .map(signal => [signal.symbol, { symbol: signal.symbol, unit: signal.unit }])).values()];
  const step = loop.controller.taskPeriodMs;
  const duration = Math.ceil(Math.max(2, loop.experiment?.durationSeconds ?? 30) * 1000 / step) * step;
  if (!Number.isInteger(step) || step < 1 || step > 1000 || duration > 600000 || duration / step + 1 > 12000
    || observed.length > 16 || (duration / step + 1) * observed.length > 250000) throw new Error("Multichannel acquisition exceeds the declared native time/channel budget.");
  const bounds = (end: number) => multichannelAcquisitionBounds(loop, end);
  if (cancelled()) throw new Error("Identification cancelled.");
  await progress("Reading all channel operating points in isolated ST before planning independent excitation.");
  const baseline = await runtime.verify({ step_ms: step, duration_ms: step * 3, seed: loop.experiment?.seed ?? 1,
    max_total_cycles: 4, wall_budget_ms: 10000, initial_values: {}, events: [], observations: observed }, bounds(step * 3 / 1000));
  retain(baseline);
  if (baseline.evaluation.status !== "Pass") throw new Error("Multichannel baseline has missing observations or violated bounds.");
  const first = baseline.run.samples[0];
  const initial = (symbol: string) => {
    const value = first?.values[symbol];
    if (value?.quality !== "good" || value.value === null || !Number.isFinite(value.value)) throw new Error(`Missing good baseline value for ${symbol}.`);
    return value.value;
  };
  const runs: RuntimeVerificationResult[] = [];
  for (let trial = 0; trial < 2; trial++) {
    if (cancelled()) throw new Error("Identification cancelled.");
    const initialValues: Record<string, number> = { [mode.symbol]: Number(closedLoop ? !mode.manual_value : mode.manual_value) };
    const events: { at_ms: number; symbol: string; value: number }[] = [];
    const onset = 3 * step, restore = duration - step;
    const count = closedLoop ? references.length : inputs.length;
    const interval = Math.max(step, Math.ceil((restore - onset) / Math.max(16, Math.floor(900 / count)) / step) * step);
    const length = Math.floor((restore - onset) / interval);
    if (length < 15) throw new Error("Lengthen the acquisition to obtain independent multichannel excitation.");
    for (let index = 0; index < count; index++) {
      const signal = closedLoop ? references[index] : inputs[index].signal;
      const symbol = closedLoop ? signal.symbol : manualCommands[index]!;
      const center = initial(signal.symbol);
      const minimum = closedLoop ? signal.range.minimum : inputs[index].actuator.minimum;
      const maximum = closedLoop ? signal.range.maximum : inputs[index].actuator.maximum;
      const binary = !closedLoop && inputs[index].actuator.kind === "on-off";
      const method = loop.experiment?.kind;
      if (binary) {
        if (center !== minimum && center !== maximum) throw new Error(`ON/OFF baseline for ${signal.symbol} must be a declared output level.`);
        if (method !== "mv_step" && method !== "mv_pulse") throw new Error(`Choose binary step or pulse acquisition for ${signal.symbol}.`);
        const dwell = Math.ceil(Math.max(inputs[index].actuator.minimumOnSeconds ?? 0,
          inputs[index].actuator.minimumOffSeconds ?? 0) * 1000 / step) * step;
        const start = Math.max(onset, dwell) + Math.floor((restore - onset) * (index + trial * 0.2) / count / step) * step;
        const end = method === "mv_pulse" ? start + Math.max(step, dwell, Math.floor((restore - start) / (2 * step)) * step) : restore;
        if (start >= end || end > restore || end - start < dwell) throw new Error(`Extend acquisition duration for ${signal.symbol}'s minimum dwell.`);
        initialValues[symbol] = center;
        events.push({ at_ms: start, symbol, value: center === minimum ? maximum : minimum }, { at_ms: end, symbol, value: center });
        continue;
      }
      const amplitude = Math.min(center - minimum, maximum - center) * (trial === 0 ? 0.15 : 0.1);
      if (!Number.isFinite(amplitude) || amplitude <= 0) throw new Error(`No bidirectional excitation headroom on ${signal.symbol}.`);
      const order = Math.max(2, Math.min(16, Math.floor(Math.log2(length + 1))));
      const frequency = 2 * Math.PI * (index + 2) / (length * interval / 1000);
      const sequence = method === "multisine" ? generateMultisine({ sampleTimeSeconds: interval / 1000, sampleCount: length,
        frequenciesRadPerSecond: [frequency], amplitudes: [amplitude], phasesRadians: [trial * 0.7 + index], maximumAbsoluteValue: amplitude })
        : generatePrbs({ order, length, amplitude, seed: 1 + ((loop.experiment?.seed ?? 1) + index * 7919 + trial * 104729) % (2 ** order - 1) });
      if (sequence.status !== "qualified") throw new Error(`Cannot plan excitation for ${signal.symbol}: ${sequence.reason}.`);
      initialValues[symbol] = center;
      if (method === "mv_step" || method === "mv_pulse" || method === "setpoint_step") {
        const start = onset + Math.floor((restore - onset) * index / count / step) * step;
        events.push({ at_ms: start, symbol, value: center + amplitude });
        if (method === "mv_pulse") {
          const end = Math.min(restore - step, start + Math.max(step, Math.floor((restore - onset) / count / (2 * step)) * step));
          events.push({ at_ms: end, symbol, value: center });
        }
      } else sequence.values.forEach((value, sample) => events.push({ at_ms: onset + sample * interval, symbol, value: center + value }));
      events.push({ at_ms: restore, symbol, value: center });
    }
    events.sort((left, right) => left.at_ms - right.at_ms || left.symbol.localeCompare(right.symbol));
    await progress(`${trial === 0 ? "Identification" : "Independent validation"}: ${closedLoop ? "closed-loop reference" : "Manual-input"} multichannel excitation; recording all cross-responses.`);
    const result = await runtime.verify({ step_ms: step, duration_ms: duration, seed: (loop.experiment?.seed ?? 1) + trial,
      max_total_cycles: duration / step + 1, wall_budget_ms: 10000, initial_values: initialValues, events, observations: observed }, bounds(duration / 1000));
    runs.push(result); retain(result);
    if (result.projectRevision !== baseline.projectRevision || result.evaluation.status !== "Pass") throw new Error("Multichannel acquisition changed revision or violated its bounds.");
  }
  const fitted = await fitMultichannelRecordings(loop, runs, fitting, cancelled);
  return { model: fitted.model, runs: [baseline, ...runs], fit: fitted };
}

/** New and reused records go through exactly the same multichannel fitting owner. */
export async function fitMultichannelRecordings(loop: ControlLoopDefinition, runs: readonly RuntimeVerificationResult[],
  fitting: BoundedAnalysisJob, cancelled: () => boolean): Promise<IdentifiedStateModelResult> {
  if (!loop.channels || runs.length < 1 || runs.length > 2 || runs[1] && (runs[0].run.id === runs[1].run.id
    || runs[0].projectRevision !== runs[1].projectRevision)) throw new Error("Two independent records with one project revision are required.");
  const inputs = loop.channels.inputs.map(input => ({ ...input, signal: loopSignal(loop, input.signalId) }));
  const outputs = loop.channels.outputs.map(output => ({ ...output, signal: loopSignal(loop, output.signalId) }));
  const channel = (signal: typeof loop.signals.processVariable) => ({ id: signal.symbol, unit: signal.unit, operatingRange: signal.range });
  const fitted = await fitting.run<IdentifiedStateModelResult>(path.join(__dirname, "generalIdentificationWorker.js"), {
    estimation: runs[0].run, validation: runs[1]?.run, channels: { inputs: inputs.map(input => channel(input.signal)), outputs: outputs.map(output => channel(output.signal)) },
    maximumNormalizedError: loop.identificationPolicy?.maximumNormalizedError ?? 0.15, maximumOrder: loop.identificationPolicy?.maximumOrder ?? 4,
  });
  if (!fitted || cancelled()) throw new Error("Identification cancelled.");
  return { ...fitted, model: { ...fitted.model, inputs: fitted.model.inputs.map((channel, index) => ({ ...channel, id: inputs[index].modelChannelId })),
    outputs: fitted.model.outputs.map((channel, index) => ({ ...channel, id: outputs[index].modelChannelId })) },
    ...(fitted.estimationPredictions ? { estimationPredictions: fitted.estimationPredictions.map((series, index) => ({ ...series, channelId: outputs[index].modelChannelId })) } : {}),
    validationPredictions: fitted.validationPredictions.map((series, index) => ({ ...series, channelId: outputs[index].modelChannelId })) };
}

export function multichannelAcquisitionBounds(loop: ControlLoopDefinition, end: number): RunRequirement[] {
  if (!loop.channels) throw new Error("Ordered model channels are required.");
  const window = { start: 0, end };
  return [...loop.channels.inputs.map(input => ({ id: `input:${input.modelChannelId}`, kind: "bounds" as const,
    channel: loopSignal(loop, input.signalId).symbol, window, minimum: input.actuator.minimum, maximum: input.actuator.maximum })),
  ...loop.channels.outputs.map(output => { const signal = loopSignal(loop, output.signalId); return {
    id: `output:${output.modelChannelId}`, kind: "bounds" as const, channel: signal.symbol, window,
    minimum: signal.range.minimum, maximum: signal.range.maximum }; })];
}
