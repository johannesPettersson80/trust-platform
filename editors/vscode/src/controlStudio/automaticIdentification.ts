import { identificationLoop } from "./identificationPolicy";
import { fitRecordedStateModel } from "./recordedStateIdentification";
import { fitRecordedModel } from "./recordedModelIdentification";
import type { GeneralIdentificationFit } from "./generalModelFitPresentation";
import { acquireMultichannelModel, fitMultichannelRecordings, multichannelAcquisitionBounds } from "./multichannelIdentification";
import { relayPiFromMeasuredRecord } from "./relayTuning";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { AcceptedIdentificationModel, IdentificationSample } from "./identification";
import { BoundedAnalysisJob } from "./boundedAnalysisJob";
import type { RuntimeVerificationClient, RuntimeVerificationResult } from "./verificationClient";
import type { GeneralModel } from "./generalModel";
import { estimateFrequencyResponse, fitFirstOrderFrequencyResponse, generateMultisine, generatePrbs } from "./numerics";
import * as path from "path";
import { qualifyIdentificationBaseline } from "./identificationBaseline";
import { evaluateRecordedRun } from "./runValidation";
import { createHash } from "crypto";

export interface IdentificationCampaignRecord {
  status: "running" | "completed" | "failed" | "cancelled";
  reason?: string;
  runs: readonly RuntimeVerificationResult[];
}

export type AutomaticIdentificationResult =
  | { accepted: AcceptedIdentificationModel; runs: readonly RuntimeVerificationResult[] }
  | { model: GeneralModel; runs: readonly RuntimeVerificationResult[]; fit?: GeneralIdentificationFit };

/** Bounded time- or frequency-domain identification for explicitly declared manual-output ownership in isolated simulation. */
export class AutomaticIdentification {
  private readonly fitting = new BoundedAnalysisJob();
  private cancelled = false;
  private retained: IdentificationCampaignRecord = { status: "running", runs: [] };
  constructor(private readonly runtime: RuntimeVerificationClient,
    private readonly readOperatingPoint?: (loop: ControlLoopDefinition) => Promise<{ input: number; output: number }>) {}
  async run(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>, reuse?: {
    projectRevision: string; runs: readonly RuntimeVerificationResult[];
  }): Promise<AutomaticIdentificationResult> {
    try {
      const reused = reuse ? await this.reuseRecordings(loop, reuse, progress) : undefined;
      const result = reused ?? await this.execute(identificationLoop(loop), progress);
      this.retained.status = "completed";
      if ("model" in result && loop.channels) {
        return bindIdentificationChannels(result, loop.channels);
      }
      return result;
    } catch (error) {
      this.retained.status = this.cancelled ? "cancelled" : "failed";
      this.retained.reason = error instanceof Error ? error.message : String(error);
      throw error;
    }
  }
  snapshot(): IdentificationCampaignRecord {
    return JSON.parse(JSON.stringify(this.retained)) as IdentificationCampaignRecord;
  }
  private async reuseRecordings(loop: ControlLoopDefinition, reuse: { projectRevision: string; runs: readonly RuntimeVerificationResult[] },
    progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult | undefined> {
    if ((loop.channels?.inputs.length ?? 1) > 1 || (loop.channels?.outputs.length ?? 1) > 1) {
      const records = [...new Map(reuse.runs.filter(result => result.projectRevision === reuse.projectRevision && result.request.simulation_override === undefined
        && result.run.evidence === "ST-executed" && result.run.samples.length >= 30
        && result.evaluation.status === "Pass").map(result => [result.run.id, result])).values()].slice(-2);
      if (records.length !== 2) return undefined;
      try {
        for (const result of records) {
          const requirements = multichannelAcquisitionBounds(loop, result.run.conditions.durationSeconds);
          if (evaluateRecordedRun(result.run, requirements).status !== "Pass") return undefined;
        }
        await progress("Reusing independent multichannel records and comparing every required output against frozen model predictions…");
        const fitted = await fitMultichannelRecordings(loop, records, this.fitting, () => this.cancelled);
        this.retained.runs = records;
        return { model: fitted.model, runs: records, fit: fitted };
      } catch (error) {
        if (this.cancelled) throw error;
        await progress("Existing multichannel records do not establish an adequate current model; preparing the selected acquisition.");
        return undefined;
      }
    }
    const mv = loop.signals.manipulatedVariable; const pv = loop.signals.processVariable;
    const eligible = reuse.runs.filter(result => result.projectRevision === reuse.projectRevision && result.request.simulation_override === undefined
      && result.run.evidence === "ST-executed"
      && result.evaluation.status === "Pass" && result.run.channels.some(channel => channel.id === mv.symbol && channel.unit === mv.unit)
      && result.run.channels.some(channel => channel.id === pv.symbol && channel.unit === pv.unit));
    // An estimation record and its independent validation record must be distinct.
    const records = [...new Map(eligible.map(result => [result.run.id, result])).values()].slice(-2);
    if (records.length !== 2 || records[0].run.conditions.modelRevision !== records[1].run.conditions.modelRevision) return undefined;
    for (const result of records) {
      const window = { start: 0, end: result.run.conditions.durationSeconds };
      const bounds = evaluateRecordedRun(result.run, [
        { id: "reused-output-bounds", kind: "bounds", channel: mv.symbol, window,
          minimum: loop.limits.outputMinimum, maximum: loop.limits.outputMaximum },
        { id: "reused-process-bounds", kind: "bounds", channel: pv.symbol, window,
          minimum: pv.range.minimum, maximum: pv.range.maximum },
      ]);
      if (bounds.status !== "Pass") return undefined;
    }
    await progress("Reusing two independent recordings from the current ST revision; preserving raw data and refitting frozen candidates.");
    try {
      const fitted = await fitRecordedModel(loop, records, this.fitting, () => this.cancelled);
      this.retained.runs = records;
      return fitted;
    } catch (error) {
      if (this.cancelled) throw error;
      await progress("Existing recordings do not establish an adequate model; selecting a new bounded experiment.");
      return undefined;
    }
  }

  private async execute(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult> {
    if (loop.channels && (loop.channels.inputs.length > 1 || loop.channels.outputs.length > 1)) {
      const retained: RuntimeVerificationResult[] = []; this.retained.runs = retained;
      return acquireMultichannelModel(loop, this.runtime, this.fitting, () => this.cancelled, run => retained.push(run), progress);
    }
    const mode = loop.parameterBinding?.mode;
    const manualOutput = loop.controller.manualOutputSignal;
    if (loop.experiment?.kind === "setpoint_step") return this.executeClosedLoop(loop, progress);
    if (!mode || !manualOutput) throw new Error("Confirm the manual-output and mode bindings before automatic open-loop identification.");
    const requestedMethod = loop.experiment?.kind;
    if (requestedMethod === "relay") {
      return this.executeRelay(loop, progress);
    }
    const step = loop.controller.taskPeriodMs;
    if (!Number.isInteger(step) || step < 1 || step > 1000) throw new Error("Identification requires a supported whole-millisecond scan period.");
    const duration = Math.ceil(Math.max(2, loop.experiment?.durationSeconds ?? 10) * 1000 / step) * step;
    if (duration / step + 1 > 12000 || duration > 600000) throw new Error("The declared identification duration exceeds the bounded runtime adapter.");
    const actuator = loop.channels?.inputs[0]?.actuator;
    const switching = actuator?.kind === "on-off" || !!loop.controller.onOff;
    const minimumDwellMs = switching ? Math.ceil(Math.max(actuator?.minimumOnSeconds ?? 0,
      actuator?.minimumOffSeconds ?? 0) * 1000 / step) * step : 0;
    const onset = Math.max(step * 3, minimumDwellMs, Math.round(duration * 0.1 / step) * step);
    const minimum = loop.limits.outputMinimum; const maximum = loop.limits.outputMaximum;
    if (!this.readOperatingPoint) throw new Error("A measured operating point is required before identification.");
    const point = await this.readOperatingPoint(loop);
    if (!Number.isFinite(point.input) || !Number.isFinite(point.output) || point.input < minimum || point.input > maximum
        || point.output < loop.signals.processVariable.range.minimum || point.output > loop.signals.processVariable.range.maximum) {
      throw new Error("Measured operating point is outside the declared bounds.");
    }
    if (switching && (requestedMethod === "multisine" || requestedMethod === "prbs")) {
      throw new Error("The selected frequency excitation requires intermediate output levels. Choose a binary step or pulse for this ON/OFF actuator.");
    }
    if (requestedMethod === "multisine" || requestedMethod === "prbs") {
      return this.executeFrequency(loop, point, onset, duration, requestedMethod, progress);
    }
    const baseline = point.input;
    const low = loop.controller.onOff?.lowOutput ?? actuator?.minimum ?? minimum;
    const high = loop.controller.onOff?.highOutput ?? actuator?.maximum ?? maximum;
    if (switching && baseline !== low && baseline !== high) throw new Error("ON/OFF identification requires a measured baseline at one of the declared output levels.");
    await progress("Baseline must match the measured MV/PV within 1e-6 × max(1, absolute coordinate); deterministic simulation only.");
    const span = maximum - minimum;
    const requestedAmplitude = loop.experiment?.amplitude ?? 0;
    const delta = requestedAmplitude || (maximum - baseline >= baseline - minimum ? span * 0.1 : -span * 0.1);
    if (!switching && (baseline + delta < minimum || baseline + delta > maximum)) throw new Error("The selected excitation amplitude exceeds the available command headroom.");
    if (switching && requestedAmplitude && requestedAmplitude !== (baseline === low ? high : low) - baseline) throw new Error("Binary excitation requires the full available level change; use zero amplitude for automatic selection.");
    const pv = loop.signals.processVariable; const mv = loop.signals.manipulatedVariable;
    const pulse = loop.experiment?.kind === "mv_pulse";
    const pulseRestoreAt = pulse ? Math.min(duration - step,
      onset + Math.max(step, Math.floor((duration - onset) / (2 * step)) * step)) : duration;
    const runs: RuntimeVerificationResult[] = [];
    this.retained.runs = runs;
    for (const [index, fraction] of [1, 0.75].entries()) {
      if (this.cancelled) throw new Error("Identification cancelled.");
      const input = switching ? (baseline === low ? high : low) : baseline + delta * fraction;
      // Independent binary trials vary timing, never invent an intermediate actuator level.
      const trialOnset = switching ? onset + index * Math.max(step, Math.floor(onset * 0.25 / step) * step) : onset;
      if (trialOnset >= pulseRestoreAt || pulseRestoreAt - trialOnset < minimumDwellMs) {
        throw new Error("Extend identification duration to cover the declared ON/OFF dwell and independent excitation intervals.");
      }
      await progress(`${index === 0 ? "Identification" : "Independent validation"} trial: bounded ${pulse ? "pulse" : "step"} ${baseline} → ${input} ${mv.unit}; ${duration / 1000} s, followed by restoration.`);
      if (this.cancelled) throw new Error("Identification cancelled.");
      const result = await this.runtime.verify({ step_ms: step, duration_ms: duration, seed: loop.experiment?.seed ?? 1,
        max_total_cycles: duration / step + 1, wall_budget_ms: 10000,
        initial_values: { [mode.symbol]: Number(mode.manual_value), [manualOutput]: baseline, [mv.symbol]: baseline },
        events: [{ at_ms: trialOnset, symbol: manualOutput, value: input }, { at_ms: pulseRestoreAt, symbol: manualOutput, value: baseline }],
        observations: [{ symbol: mv.symbol, unit: mv.unit }, { symbol: pv.symbol, unit: pv.unit }],
      }, [
        { id: "identification-output-bounds", kind: "bounds", channel: mv.symbol, minimum, maximum, window: { start: 0, end: duration / 1000 } },
        { id: "identification-process-bounds", kind: "bounds", channel: pv.symbol, minimum: pv.range.minimum, maximum: pv.range.maximum,
          window: { start: 0, end: duration / 1000 } },
      ]);
      const previous = runs[0];
      runs.push(result);
      if (result.evaluation.status !== "Pass") throw new Error("Identification record violated its declared bounds or has missing evidence; review the retained trial before retrying.");
      qualifyIdentificationBaseline(result.run, loop, point, trialOnset / 1000);
      if (previous && previous.projectRevision !== result.projectRevision) throw new Error("The loaded ST or configured plant changed between identification trials.");
    }
    if (this.cancelled) throw new Error("Identification cancelled.");
    const samples = (result: RuntimeVerificationResult): IdentificationSample[] => result.run.samples
      // Keep the complete restoration record above; fit only the declared held-input interval.
      .filter(sample => sample.timeSeconds < duration / 1000)
      .map(sample => ({ timeSeconds: sample.timeSeconds, input: sample.values[mv.symbol].value!, output: sample.values[pv.symbol].value!,
        quality: sample.values[mv.symbol].quality === "good" && sample.values[pv.symbol].quality === "good" ? "good" : "bad",
        inputUnit: mv.unit, outputUnit: pv.unit }));
    await progress("Fitting candidate models and testing them against the independent trial…");
    try {
    const accepted = await this.fitting.run<AcceptedIdentificationModel>(path.join(__dirname, "identificationWorker.js"), {
      estimation: samples(runs[0]), validation: samples(runs[1]),
      estimationRecordingId: runs[0].run.id, validationRecordingId: runs[1].run.id,
      maximumNormalizedError: loop.identificationPolicy?.maximumNormalizedError ?? 0.15,
    });
    if (this.cancelled || !accepted) throw new Error("Identification cancelled.");
    return { accepted, runs };
    } catch (error) {
      if (this.cancelled) throw error;
      return this.fitStateRecords(loop, runs, progress);
    }
  }

  private async executeRelay(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult> {
    const mode = loop.parameterBinding?.mode, command = loop.controller.manualOutputSignal;
    const direction = loop.experiment?.relayDirection;
    if (!mode || !command || !this.readOperatingPoint || (direction !== "positive" && direction !== "negative")) {
      throw new Error("Feedback-relay identification needs an explicit process direction and Manual command ownership.");
    }
    const point = await this.readOperatingPoint(loop);
    const step = loop.controller.taskPeriodMs;
    const duration = Math.ceil(Math.max(2, loop.experiment?.durationSeconds ?? 30) * 1000 / step) * step;
    if (!Number.isInteger(step) || step < 1 || step > 1000 || duration > 600000 || duration / step + 1 > 12000) throw new Error("Relay acquisition exceeds its explicit timing budget.");
    const mv = loop.signals.manipulatedVariable, pv = loop.signals.processVariable;
    const actuator = loop.channels?.inputs[0]?.actuator;
    const switching = actuator?.kind === "on-off" || !!loop.controller.onOff;
    const lowLevel = loop.controller.onOff?.lowOutput ?? actuator?.minimum ?? loop.limits.outputMinimum;
    const highLevel = loop.controller.onOff?.highOutput ?? actuator?.maximum ?? loop.limits.outputMaximum;
    if (switching && point.input !== lowLevel && point.input !== highLevel) throw new Error("Binary relay acquisition needs a measured baseline at a declared output level.");
    const highDwell = Math.ceil((actuator?.minimumOnSeconds ?? 0) * 1000 / step) * step;
    const lowDwell = Math.ceil((actuator?.minimumOffSeconds ?? 0) * 1000 / step) * step;
    const start = Math.max(step * 3, switching ? Math.max(highDwell, lowDwell) : 0);
    if (duration - step - start <= Math.max(highDwell, lowDwell)) throw new Error("Extend relay duration to cover the equipment dwell and restoration.");
    const amplitude = Math.min(Math.abs(loop.experiment?.amplitude || (loop.limits.outputMaximum - loop.limits.outputMinimum) * 0.1),
      point.input - loop.limits.outputMinimum, loop.limits.outputMaximum - point.input);
    const hysteresis = Math.max(1e-9, (pv.range.maximum - pv.range.minimum) * 0.005);
    if (!switching && (!Number.isFinite(amplitude) || amplitude <= 0)) throw new Error("No bounded relay amplitude is available around the measured input.");
    const runs: RuntimeVerificationResult[] = []; this.retained.runs = runs;
    for (let trial = 0; trial < 2; trial++) {
      if (this.cancelled) throw new Error("Identification cancelled.");
      const low = switching ? lowLevel : point.input - amplitude * (trial === 0 ? 1 : 0.75), high = switching ? highLevel : point.input + amplitude * (trial === 0 ? 1 : 0.75);
      const window = { start: 0, end: duration / 1000 };
      await progress(`${trial === 0 ? "Identification" : "Independent validation"}: bounded feedback relay; switching follows measured PV crossings.`);
      const result = await this.runtime.verify({ step_ms: step, duration_ms: duration, seed: (loop.experiment?.seed ?? 1) + trial,
        max_total_cycles: duration / step + 1, wall_budget_ms: 10000,
        initial_values: { [mode.symbol]: Number(mode.manual_value), [command]: point.input }, events: [],
        feedback_relay: { process_variable: pv.symbol, command, center: point.output, hysteresis: switching ? hysteresis * (1 + trial * 0.2) : hysteresis, low, high, restore: point.input,
          process_minimum: pv.range.minimum, process_maximum: pv.range.maximum, positive_gain: direction === "positive", start_ms: start, restore_ms: duration - step, minimum_high_ms: highDwell, minimum_low_ms: lowDwell },
        observations: [...new Map([mv, pv, { symbol: command, unit: mv.unit }].map(signal => [signal.symbol, { symbol: signal.symbol, unit: signal.unit }])).values()],
      }, [{ id: "relay-input-bounds", kind: "bounds", channel: mv.symbol, window, minimum: loop.limits.outputMinimum, maximum: loop.limits.outputMaximum },
        { id: "relay-process-bounds", kind: "bounds", channel: pv.symbol, window, minimum: pv.range.minimum, maximum: pv.range.maximum }]);
      runs.push(result);
      if (result.evaluation.status !== "Pass") throw new Error("Relay acquisition violated a bound or lacks qualified data.");
      const cycles = relayPiFromMeasuredRecord({ lowOutput: low, highOutput: high,
        samples: result.run.samples.filter(sample => sample.timeSeconds >= start / 1000 && sample.timeSeconds < (duration - step) / 1000)
          .map(sample => ({ timeSeconds: sample.timeSeconds, processValue: sample.values[pv.symbol].value!, output: sample.values[mv.symbol].value! })) });
      if (cycles.status !== "qualified") throw new Error(`Relay cycle qualification failed: ${cycles.reason}`);
    }
    try { return await this.fitStateRecords(loop, runs, progress); }
    catch (error) {
      if (this.cancelled) throw error;
      throw new Error("Relay recordings retained, but they do not establish an independently adequate full model. Select supplementary PRBS or multisine identification.");
    }
  }

  private async executeClosedLoop(loop: ControlLoopDefinition, progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult> {
    const mode = loop.parameterBinding?.mode; const reference = loop.signals.setpoint;
    if (!mode || !reference || !this.readOperatingPoint) throw new Error("Closed-loop identification needs bound mode, reference and current measurements.");
    const point = await this.readOperatingPoint(loop);
    const step = loop.controller.taskPeriodMs;
    const duration = Math.ceil(Math.max(2, loop.experiment?.durationSeconds ?? 10) * 1000 / step) * step;
    if (!Number.isInteger(step) || step < 1 || step > 1000 || duration > 600000 || duration / step + 1 > 12000) throw new Error("Closed-loop acquisition exceeds its timing budget.");
    const up = Math.min(reference.range.maximum, loop.signals.processVariable.range.maximum) - point.output;
    const down = point.output - Math.max(reference.range.minimum, loop.signals.processVariable.range.minimum);
    const requested = loop.experiment?.amplitude ?? 0;
    const direction = requested ? Math.sign(requested) : up >= down ? 1 : -1;
    const headroom = direction > 0 ? up : down;
    const magnitude = requested ? Math.abs(requested) : headroom * 0.1;
    if (!Number.isFinite(magnitude) || magnitude <= 0 || magnitude > headroom) throw new Error("The selected reference excitation exceeds the available setpoint headroom.");
    const amplitude = direction * magnitude;
    const runs: RuntimeVerificationResult[] = []; this.retained.runs = runs;
    for (let trial = 0; trial < 2; trial++) {
      if (this.cancelled) throw new Error("Identification cancelled.");
      const onset = Math.max(3 * step, Math.floor(duration * 0.1 / step) * step);
      const restore = duration - step;
      const value = point.output + amplitude * (trial === 0 ? 1 : 0.75);
      await progress(`${trial === 0 ? "Identification" : "Independent validation"}: closed-loop reference step with terminal restoration; recording actual plant input and output.`);
      const mv = loop.signals.manipulatedVariable, pv = loop.signals.processVariable;
      const window = { start: 0, end: duration / 1000 };
      const result = await this.runtime.verify({ step_ms: step, duration_ms: duration, seed: (loop.experiment?.seed ?? 1) + trial,
        max_total_cycles: duration / step + 1, wall_budget_ms: 10000,
        initial_values: { [mode.symbol]: Number(!mode.manual_value), [reference.symbol]: point.output },
        events: [{ at_ms: onset, symbol: reference.symbol, value }, { at_ms: restore, symbol: reference.symbol, value: point.output }],
        observations: [mv, pv, reference].map(signal => ({ symbol: signal.symbol, unit: signal.unit })),
      }, [{ id: "closed-loop-input-bounds", kind: "bounds", channel: mv.symbol, window, minimum: loop.limits.outputMinimum, maximum: loop.limits.outputMaximum },
        { id: "closed-loop-output-bounds", kind: "bounds", channel: pv.symbol, window, minimum: pv.range.minimum, maximum: pv.range.maximum }]);
      runs.push(result);
      if (result.evaluation.status !== "Pass") throw new Error("Closed-loop acquisition violated a bound or lacks qualified measurements.");
    }
    return this.fitStateRecords(loop, runs, progress);
  }

  private async fitStateRecords(loop: ControlLoopDefinition, runs: readonly RuntimeVerificationResult[], progress: (text: string) => Promise<void>): Promise<AutomaticIdentificationResult> {
    await progress("Comparing dynamic state models using independent free-running predictions…");
    const result = await fitRecordedStateModel(loop, runs, this.fitting, () => this.cancelled);
    return { model: result.model, runs, fit: { comparisons: result.comparisons, validationPredictions: result.validationPredictions } };
  }

  /** Runs one supported frequency method without changing it into a time-domain step fit. */
  private async executeFrequency(
    loop: ControlLoopDefinition,
    point: { input: number; output: number },
    onset: number,
    duration: number,
    method: "multisine" | "prbs",
    progress: (text: string) => Promise<void>,
  ): Promise<AutomaticIdentificationResult> {
    const step = loop.controller.taskPeriodMs;
    const sampleTimeSeconds = step / 1000;
    const samples = Math.floor((duration - onset) / step) - 1;
    if (samples < 64) throw new Error(`Automatic ${method} frequency identification needs at least 64 post-onset samples.`);
    const durationSeconds = samples * sampleTimeSeconds;
    const frequencies = [2, 4, 6].map(cycles => 2 * Math.PI * cycles / (durationSeconds / 2));
    const minimum = loop.limits.outputMinimum; const maximum = loop.limits.outputMaximum;
    const headroom = Math.min(maximum - point.input, point.input - minimum) * 0.8;
    if (!Number.isFinite(headroom) || headroom <= 0) throw new Error(`Measured operating point has no bounded ${method} output headroom.`);
    const mode = loop.parameterBinding!.mode; const manualOutput = loop.controller.manualOutputSignal!;
    const mv = loop.signals.manipulatedVariable; const pv = loop.signals.processVariable;
    const runs: RuntimeVerificationResult[] = [];
    this.retained.runs = runs;
    const records: Array<{ input: number[]; output: number[] }> = [];
    for (const [index, fraction] of [1, 0.75].entries()) {
      if (this.cancelled) throw new Error("Identification cancelled.");
      const phase = index === 0 ? [0.2, 1.1, 2.3] : [0.7, 1.9, 3.4];
      const prbsDwell = step * 10;
      const prbsSamples = Math.floor(samples * step / prbsDwell);
      const excitation = method === "multisine"
        ? generateMultisine({ sampleTimeSeconds, sampleCount: samples, frequenciesRadPerSecond: frequencies,
          amplitudes: frequencies.map(() => headroom * fraction / frequencies.length), phasesRadians: phase,
          maximumAbsoluteValue: headroom * fraction })
        : generatePrbs({ order: prbsOrder(prbsSamples), length: prbsSamples, amplitude: headroom * fraction, seed: (loop.experiment?.seed ?? 1) + index });
      if (excitation.status !== "qualified") throw new Error(`Automatic ${method} excitation is invalid: ${excitation.reason}.`);
      const eventStep = method === "prbs" ? prbsDwell : step;
      const events = excitation.values.map((value, sample) => ({ at_ms: onset + sample * eventStep, symbol: manualOutput, value: point.input + value }));
      events.push({ at_ms: duration - step, symbol: manualOutput, value: point.input });
      await progress(`${index === 0 ? "Identification" : "Independent validation"} trial: bounded ${method} around ${point.input} ${mv.unit}; ${duration / 1000} s, followed by restoration.`);
      const result = await this.runtime.verify({ step_ms: step, duration_ms: duration, seed: (loop.experiment?.seed ?? 1) + index,
        max_total_cycles: duration / step + 1, wall_budget_ms: 10000,
        initial_values: { [mode.symbol]: Number(mode.manual_value), [manualOutput]: point.input, [mv.symbol]: point.input },
        events, observations: [{ symbol: mv.symbol, unit: mv.unit }, { symbol: pv.symbol, unit: pv.unit }],
      }, [
        { id: "identification-output-bounds", kind: "bounds", channel: mv.symbol, minimum, maximum, window: { start: 0, end: duration / 1000 } },
        { id: "identification-process-bounds", kind: "bounds", channel: pv.symbol, minimum: pv.range.minimum, maximum: pv.range.maximum,
          window: { start: 0, end: duration / 1000 } },
      ]);
      runs.push(result);
      if (result.evaluation.status !== "Pass") throw new Error(`${method} record violated its declared bounds or has missing evidence; review the retained trial before retrying.`);
      qualifyIdentificationBaseline(result.run, loop, point, onset / 1000);
      if (index > 0 && runs[index - 1].projectRevision !== result.projectRevision) throw new Error("The loaded ST or configured plant changed between multisine trials.");
      const retained = result.run.samples.filter(sample => sample.timeSeconds >= onset / 1000 && sample.timeSeconds < (duration - step) / 1000);
      const input = retained.map(sample => sample.values[mv.symbol].value);
      const output = retained.map(sample => sample.values[pv.symbol].value);
      if (input.length !== samples || output.length !== samples || ![...input, ...output].every(Number.isFinite)) {
        throw new Error(`${method} runtime record lacks finite complete measured input/output samples.`);
      }
      records.push({ input: input as number[], output: output as number[] });
    }
    const estimated = estimateFrequencyResponse(records[0].input, records[0].output, sampleTimeSeconds, frequencies,
      { segmentCount: 2, minimumCoherence: 0.6 });
    const validated = estimateFrequencyResponse(records[1].input, records[1].output, sampleTimeSeconds, frequencies,
      { segmentCount: 2, minimumCoherence: 0.6 });
    if (estimated.status !== "qualified" || validated.status !== "qualified") {
      const reason = estimated.status === "rejected" ? estimated.reason
        : validated.status === "rejected" ? validated.reason : "unknown-frequency-result";
      throw new Error(`${method} frequency response is not qualified: ${reason}.`);
    }
    const fit = fitFirstOrderFrequencyResponse(estimated.points, validated.points, { maximumNormalizedValidationError: loop.identificationPolicy?.maximumNormalizedError ?? 0.15 });
    if (fit.status !== "qualified") {
      await progress(`${method} needs a richer dynamic model; fitting retained time records against the independent trial.`);
      return this.fitStateRecords(loop, runs, progress);
    }
    const evidence = [digest({ run: runs[0].run.id, points: estimated.points }), digest({ run: runs[1].run.id, points: validated.points })];
    return { runs, model: {
      kind: "transfer-function", numerator: [fit.model.gain], denominator: [fit.model.timeConstantSeconds, 1], delaySeconds: 0,
      source: "fitted", validation: "validated", evidence,
      operatingPoint: { inputs: [point.input], outputs: [point.output] },
      revision: digest({ method: `${method}-frequency-v1`, fit: fit.model, evidence }),
      inputs: [{ id: mv.symbol, unit: mv.unit, operatingRange: mv.range }],
      outputs: [{ id: pv.symbol, unit: pv.unit, operatingRange: pv.range }],
    } };
  }
  cancellationOwner(): Pick<RuntimeVerificationClient, "cancel" | "snapshot"> { return this.runtime; }
  dispose(): void { this.cancelled = true; this.fitting.cancel(); this.runtime.dispose(); }
}

/** Renames coordinates without separating the selected fit from its frozen predictions. */
export function bindIdentificationChannels(result: Extract<AutomaticIdentificationResult, { model: GeneralModel }>,
  channels: NonNullable<ControlLoopDefinition["channels"]>): Extract<AutomaticIdentificationResult, { model: GeneralModel }> {
  const original = result.model;
  if (original.inputs.length !== channels.inputs.length || original.outputs.length !== channels.outputs.length) {
    throw new Error("Identified model channels do not match the configured bindings.");
  }
  if (original.inputs.every((channel, index) => channel.id === channels.inputs[index].modelChannelId)
    && original.outputs.every((channel, index) => channel.id === channels.outputs[index].modelChannelId)) return result;
  const renamed = { ...original,
    inputs: original.inputs.map((channel, index) => ({ ...channel, id: channels.inputs[index].modelChannelId })),
    outputs: original.outputs.map((channel, index) => ({ ...channel, id: channels.outputs[index].modelChannelId })),
  };
  const model = { ...renamed, revision: digest(renamed) };
  if (!result.fit) return { ...result, model };
  if (result.fit.validationPredictions.length !== original.outputs.length) throw new Error("Model prediction channels are incomplete.");
  const validationPredictions = result.fit.validationPredictions.map((series, index) => {
    if (series.modelRevision !== original.revision || series.channelId !== original.outputs[index].id
      || series.unit !== original.outputs[index].unit) throw new Error("Model predictions do not belong to the identified model channels.");
    return { ...series, modelRevision: model.revision, channelId: model.outputs[index].id };
  });
  return { ...result, model, fit: { ...result.fit, validationPredictions } };
}

function digest(value: unknown): string { return `sha256:${createHash("sha256").update(JSON.stringify(value)).digest("hex")}`; }
function prbsOrder(samples: number): number { return Math.max(2, Math.min(16, Math.floor(Math.log2(samples + 1)))); }
