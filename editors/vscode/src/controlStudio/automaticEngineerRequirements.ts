import { analyzeGeneralModelFrequency, type GeneralModel } from "./generalModel";
import { connectedModelChannels, type ChannelRequirements } from "./loopChannels";
import { engineerRequirements, type EngineerRequirements } from "./engineerVerificationPlan";
import { decodeAutomaticTestCriteria } from "./automaticTestCriteria";
import { baselineNoise, recordedValues } from "./automaticRequirementEvidence";
import { recordingFingerprint } from "./independentModelValidation";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { ControllerTuningPreferences } from "./controllerTuningPreferences";
import type { IdentificationCampaignRecord } from "./automaticIdentification";
import { formatEngineeringNumber } from "./chartContract";
import { displayEngineeringUnit } from "./unitDisplay";
import { MAXIMUM_EXPERIMENT_DURATION_MS } from "./controlStudioLimits";

/** Draft test criteria; the existing tuner and independent native validation still decide feasibility. */
export function automaticEngineerRequirements(loop: ControlLoopDefinition, model: GeneralModel,
  current: Partial<EngineerRequirements>, campaigns: readonly IdentificationCampaignRecord[]): EngineerRequirements {
  const previous = decodeAutomaticTestCriteria(current.automaticCriteria);
  if (current.automaticCriteria !== undefined && !previous) throw new Error("Invalid automatic test criteria.");
  const supplied = { ...current };
  delete supplied.automaticCriteria;
  for (const key of previous?.fields ?? []) {
    const separator = key.indexOf(":");
    const name = separator < 0 ? key : key.slice(0, separator), channel = separator < 0 ? undefined : key.slice(separator + 1);
    if (channel) {
      const channels = { ...supplied.channelRequirements };
      const values = { ...channels[channel] } as Partial<ChannelRequirements>;
      delete values[name as keyof ChannelRequirements];
      channels[channel] = values as ChannelRequirements;
      supplied.channelRequirements = channels;
    } else delete supplied[name as keyof EngineerRequirements];
  }
  const channels = connectedModelChannels(loop, model);
  // A complete declared task requires no generated policy, even for models without a useful automatic timescale. The page
  // declares no response choice, so it is filled as for generated criteria: the same requirements give the same design.
  if (!previous?.fields.length && channels.outputs.length === 1) {
    try { return engineerRequirements({ ...supplied, tuningObjective: supplied.tuningObjective ?? "robust" } as Record<string, unknown>); }
    catch { /* Fill only absent fields below. */ }
  }
  const record = campaigns.flatMap(campaign => campaign.runs).find(item => model.evidence.includes(recordingFingerprint(item.run)));
  const inputs = channels.inputs.map((input, i) => model.operatingPoint?.inputs[i] ?? recordedValues(record, input.signal.symbol)[0]);
  const outputs = channels.outputs.map((output, i) => model.operatingPoint?.outputs[i] ?? recordedValues(record, output.signal.symbol)[0]);
  if ([...inputs, ...outputs].some(value => value === undefined || !Number.isFinite(value))) throw new Error("The model needs its operating point before automatic tuning setup.");
  const period = loop.controller.taskPeriodMs / 1000;
  const periodBound = Math.max(period, model.kind === "discrete-state-space" ? model.sampleTimeSeconds! : period);
  const overview = analyzeGeneralModelFrequency(model, [0.01 / periodBound]);
  if (overview.status !== "qualified") throw new Error(`Cannot calculate automatic test settings: ${overview.reason}.`);
  const times = overview.poles.flatMap(pole => {
    const rate = model.kind === "discrete-state-space" ? Math.abs(Math.log(Math.hypot(pole.real, pole.imaginary))) / model.sampleTimeSeconds!
      : Math.abs(pole.real) || Math.abs(pole.imaginary);
    return Number.isFinite(rate) && rate > 1e-8 ? [1 / rate] : [];
  });
  const timescale = Math.max(10 * periodBound, 2 * model.delaySeconds, ...times,
    ...(!times.length ? [(record?.run.samples[record.run.samples.length - 1]?.timeSeconds ?? 10 * periodBound) / 10] : []));
  const response = analyzeGeneralModelFrequency(model, [0.001 / timescale]);
  if (response.status !== "qualified") throw new Error("The model has no usable channel response for automatic test settings.");
  const roundTime = (seconds: number) => Number((Math.ceil(engineeringValue(seconds, 3, true) / period) * period).toPrecision(12));
  const fields: string[] = [];
  const pick = (value: unknown, fallback: number, key: string): number => {
    if (value !== undefined) {
      if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`Review the entered ${key} value.`);
      return value;
    }
    fields.push(key); return key.startsWith("referenceChange") ? engineeringValue(fallback, 3, false)
      : key.startsWith("maximumError") || key.startsWith("settlingBand") ? engineeringValue(fallback, 2, true) : fallback;
  };
  const headroom = channels.inputs.map((input, i) => {
    const values = recordedValues(record, input.applied?.symbol ?? input.signal.symbol);
    const minimum = Math.max(input.actuator.minimum, input.signal.range.minimum, model.inputs[i].operatingRange.minimum,
      ...(values.length ? [Math.min(...values)] : []));
    const maximum = Math.min(input.actuator.maximum, input.signal.range.maximum, model.inputs[i].operatingRange.maximum,
      ...(values.length ? [Math.max(...values)] : []));
    const operating = inputs[i]!;
    if (operating < minimum - 1e-7 || operating > maximum + 1e-7) throw new Error(`Review the operating limits for ${input.signal.symbol}.`);
    return { up: input.actuator.rateUpPerS === 0 ? 0 : Math.max(0, maximum - operating) * .1,
      down: input.actuator.rateDownPerS === 0 ? 0 : Math.max(0, operating - minimum) * .1 };
  });
  const slewTime = Math.max(0, ...channels.inputs.flatMap((input, i) => [
    input.actuator.rateUpPerS ? headroom[i].up / input.actuator.rateUpPerS : 0,
    input.actuator.rateDownPerS ? headroom[i].down / input.actuator.rateDownPerS : 0,
  ]));
  // Requirements belong to controlled outputs, those with a reference; an auxiliary measured output gets none. The
  // process variable, or else the first controlled output, supplies the top-level task.
  const indexed = channels.outputs.map((output, i) => ({ output, i }));
  const referenced = indexed.filter(item => item.output.reference);
  const controlled = referenced.length ? referenced : indexed;
  const primary = controlled.find(item => item.output.signal.symbol === loop.signals.processVariable.symbol) ?? controlled[0];
  const channelRequirements = Object.fromEntries(controlled.map(({ output, i }) => {
    const id = model.outputs[i].id, suffix = controlled.length > 1 ? `:${id}` : "";
    const declared = supplied.channelRequirements?.[id] ?? (i === primary.i ? supplied : {});
    const configured = output.requirements;
    const values = { ...configured, ...declared };
    const ys = recordedValues(record, output.signal.symbol), operating = outputs[i]!;
    const minimum = Math.max(output.signal.range.minimum, model.outputs[i].operatingRange.minimum,
      output.reference?.range.minimum ?? -Infinity, ...(ys.length ? [Math.min(...ys)] : []));
    const maximum = Math.min(output.signal.range.maximum, model.outputs[i].operatingRange.maximum,
      output.reference?.range.maximum ?? Infinity, ...(ys.length ? [Math.max(...ys)] : []));
    const excursionFor = (direction: number) => {
      const capacity = response.channels.filter(channel => channel.output.id === id).reduce((sum, channel) => {
        const index = model.inputs.findIndex(input => input.id === channel.input.id);
        const point = channel.points[0], positiveInput = Math.sign(point.real) === direction;
        return sum + point.magnitude * (positiveInput ? headroom[index].up : headroom[index].down);
      }, 0);
      return Math.min(Math.max(0, direction > 0 ? maximum - operating : operating - minimum) * .1, capacity);
    };
    const upward = excursionFor(1), downward = excursionFor(-1);
    const direction = upward >= downward ? 1 : -1, excursion = Math.max(upward, downward);
    const noise = baselineNoise(record, channels.inputs.map(input => input.applied?.symbol ?? input.signal.symbol), output.signal.symbol);
    if (values.referenceChange === undefined && (!(excursion > 1e-9) || excursion < 20 * noise)) throw new Error(`The recorded range or signal noise prevents an automatic test change for ${output.signal.symbol}. Review its limits or identification recording.`);
    const change = pick(values.referenceChange, direction * excursion, `referenceChange${suffix}`);
    if (!change || operating + change < minimum - 1e-8 || operating + change > maximum + 1e-8) {
      const unit = displayEngineeringUnit(output.signal.unit), room = Math.max(maximum - operating, operating - minimum);
      throw new Error(`The test change of ${formatEngineeringNumber(change)} ${unit} for ${output.signal.symbol} leaves the range the model was identified in (${formatEngineeringNumber(minimum)}–${formatEngineeringNumber(maximum)} ${unit}). `
        + `Identify with a larger step, or choose a change of at most ${formatEngineeringNumber(room)} ${unit}.`);
    }
    const band = pick(values.settlingBand, Math.max(noise, Math.min(Math.abs(change) * .02,
      typeof values.maximumError === "number" ? values.maximumError / 2 : Infinity)), `settlingBand${suffix}`);
    const error = pick(values.maximumError, Math.max(2 * band, Math.abs(change) * .04), `maximumError${suffix}`);
    // A declared overshoot limit is kept, never generated: automatic setup fills no overshoot limit, but must not drop a declared one.
    const overshoot = (values as typeof values & { maximumOvershootPercent?: number }).maximumOvershootPercent;
    return [id, { referenceChange: change, settlingBand: band, maximumError: error,
      settlingTimeSeconds: pick(values.settlingTimeSeconds, roundTime(6 * timescale + model.delaySeconds + slewTime), `settlingTimeSeconds${suffix}`),
      ...(overshoot === undefined ? {} : { maximumOvershootPercent: overshoot }) }];
  })) as Record<string, ChannelRequirements>;
  const slowest = Math.max(...Object.values(channelRequirements).map(item => item.settlingTimeSeconds));
  const duration = pick(supplied.durationSeconds, roundTime(2 * slowest + 2 * model.delaySeconds), "durationSeconds");
  if (duration > MAXIMUM_EXPERIMENT_DURATION_MS / 1_000) throw new Error("The model needs a longer validation window than this runtime supports. Review the model and test duration.");
  const tuningObjective = supplied.tuningObjective ?? "robust";
  if (supplied.tuningObjective === undefined) fields.push("tuningObjective");
  return engineerRequirements({ ...supplied, ...channelRequirements[model.outputs[primary.i].id], durationSeconds: duration, tuningObjective,
    ...(controlled.length > 1 ? { channelRequirements } : {}),
    ...(fields.length ? { automaticCriteria: { modelRevision: model.revision, fields } } : {}) });
}

/** Prefill a draft when sufficient model evidence exists; Tune reports any missing prerequisite. */
export function previewAutomaticRequirements(loop: ControlLoopDefinition, model: GeneralModel,
  current: Partial<EngineerRequirements> | undefined, campaigns: readonly IdentificationCampaignRecord[]): EngineerRequirements | undefined {
  try { return automaticEngineerRequirements(loop, model, current ?? {}, campaigns); }
  catch { return current as EngineerRequirements | undefined; }
}

function engineeringValue(value: number, digits: number, upward: boolean): number {
  if (value === 0 || !Number.isFinite(value)) return value;
  const unit = 10 ** (Math.floor(Math.log10(Math.abs(value))) - digits + 1);
  const scaled = Math.abs(value) / unit;
  return Number((Math.sign(value) * (upward ? Math.ceil(scaled - 1e-9) : Math.floor(scaled + 1e-9)) * unit).toPrecision(digits));
}

/** Resolve the task at the same host boundary for automatic and explicit tuning methods. */
export function resolveEngineerRequirements(loop: ControlLoopDefinition, model: GeneralModel, payload: Record<string, unknown>,
  preferences: ControllerTuningPreferences | undefined, campaigns: readonly IdentificationCampaignRecord[]): EngineerRequirements {
  const input = { ...payload };
  if (preferences) {
    delete input.tuningMethod;
    if (["SIMC", "AMIGO"].includes(preferences.method)) input.tuningMethod = preferences.method;
  }
  return input.automaticRequirements === true ? automaticEngineerRequirements(loop, model, input, campaigns) : engineerRequirements(input);
}
