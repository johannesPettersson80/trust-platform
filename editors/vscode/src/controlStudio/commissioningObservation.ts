import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import { connectedModelChannels } from "./loopChannels";
import { engineerVerificationPlan, type EngineerRequirements } from "./engineerVerificationPlan";
import { decodeRunRequirement, evaluateObservedRequirement, type RunRequirement } from "./runValidation";
import type { WorkflowResultSample } from "./workflowResult";
import { APPLY_OBSERVATION_LIMIT_MS, recordPeriodExecutions } from "./controlStudioLimits";
import { roundedTransferValue, type TransferBinding } from "./proportionalTransfer";

export interface CommissioningObservation {
  channels: readonly { id: string; source: string; unit: string }[];
  requirements: readonly RunRequirement[];
}
export function commissioningReferenceTargets(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements): Record<string, number> {
  const channels = connectedModelChannels(loop, model);
  return Object.fromEntries(channels.outputs.flatMap((output, index) => {
    if (!output.reference) return [];
    const required = output.requirements ?? (output.signal.symbol === loop.signals.processVariable.symbol ? task : undefined);
    if (!required) throw new Error(`Declare commissioning requirements for ${output.signal.symbol}.`);
    return [[output.reference.symbol, (model.operatingPoint?.outputs[index] ?? 0) + required.referenceChange]];
  }));
}
/**
 * The reviewed Auto target of the loop's setpoint, rounded to its IEC type as the transfer writes it and the runtime records it.
 * Live commissioning and a resumed observation assess against this same number, as the assessment compares it exactly.
 */
export function commissioningTarget(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements, binding: TransferBinding): number {
  return roundedTransferValue(binding.states[0], commissioningReferenceTargets(loop, model, task)[loop.signals.setpoint!.symbol]);
}
export function commissioningObservation(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  values: Readonly<Record<string, number>>): CommissioningObservation {
  const channels = [
    ...(loop.signals.setpoint ? [{ id: loop.signals.setpoint.symbol, source: "setpoint", unit: loop.signals.setpoint.unit }] : []),
    { id: loop.signals.processVariable.symbol, source: "processVariable", unit: loop.signals.processVariable.unit },
    { id: loop.signals.manipulatedVariable.symbol, source: "manipulatedVariable", unit: loop.signals.manipulatedVariable.unit },
    ...(loop.signals.actuator ? [{ id: loop.signals.actuator.symbol, source: "actuator", unit: loop.signals.actuator.unit }] : []),
    ...(loop.signals.additional ?? []).map(signal => ({ id: signal.symbol, source: signal.id, unit: signal.unit })),
  ];
  const observed = new Set(channels.map(channel => channel.id));
  const requirements = engineerVerificationPlan(loop, model, task, values).requirements.filter(requirement => observed.has(requirement.channel));
  return { channels, requirements };
}
export function validCommissioningObservation(value: unknown, durationSeconds: number, stepMs: number): value is CommissioningObservation {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const observed = value as CommissioningObservation;
  if (!Array.isArray(observed.channels) || !observed.channels.length || observed.channels.length > 32
    || !observed.channels.every(channel => channel && typeof channel.id === "string" && !!channel.id
      && typeof channel.source === "string" && !!channel.source && typeof channel.unit === "string" && !!channel.unit)
    || new Set(observed.channels.map(channel => channel.id)).size !== observed.channels.length
    || !Array.isArray(observed.requirements) || !observed.requirements.length || observed.requirements.length > 64) return false;
  const count = Math.ceil(durationSeconds * 1000 / stepMs);
  if (!Number.isFinite(count) || count * stepMs > APPLY_OBSERVATION_LIMIT_MS) return false;
  try {
    // The observation is recorded with a record period (A4.2), so its requirements are checked on that grid and its end.
    const every = recordPeriodExecutions(count * stepMs, stepMs);
    const times = Array.from({ length: Math.floor(count / every) + 1 }, (_, index) => index * every * stepMs / 1000);
    if (times[times.length - 1] !== count * stepMs / 1000) times.push(count * stepMs / 1000);
    const contract = { channels: observed.channels, conditions: { sampleTimesSeconds: times } };
    observed.requirements.forEach(requirement => decodeRunRequirement(requirement, contract));
    return new Set(observed.requirements.map(requirement => requirement.id)).size === observed.requirements.length;
  } catch { return false; }
}
/** The response's span and record period: the requirements' windows lie within the duration, and averaged samples cover their
 * executions (A4.2, E4.4). */
export interface ObservedResponseSpan { readonly durationSeconds: number; readonly recordEvery: number }

export function assessCommissioningObservation(samples: readonly WorkflowResultSample[], specification: CommissioningObservation,
  span?: ObservedResponseSpan): void {
  if (!samples.length) throw new Error("Commissioning channel observations are missing.");
  const start = samples[0].elapsedSeconds;
  const observed = {
    channels: specification.channels,
    conditions: { sampleTimesSeconds: samples.map(sample => Math.round((sample.elapsedSeconds - start) * 1e9) / 1e9),
      ...(span ? { durationSeconds: span.durationSeconds } : {}), ...(span && span.recordEvery > 1 ? { recordEvery: span.recordEvery } : {}) },
    samples: samples.map(sample => ({ timeSeconds: Math.round((sample.elapsedSeconds - start) * 1e9) / 1e9, sequence: sample.scan,
      values: Object.fromEntries(specification.channels.map(channel => {
        const primary = channel.source === "setpoint" ? sample.sp : channel.source === "processVariable" ? sample.pv
          : channel.source === "manipulatedVariable" ? sample.appliedMv : undefined;
        const additional = sample.additional?.find(value => value.id === channel.source && value.unit === channel.unit);
        return [channel.id, primary !== undefined ? { value: primary, quality: sample.quality }
          : { value: additional?.value ?? null, quality: additional?.quality ?? "missing" as const }];
      })),
    })),
  };
  for (const requirement of specification.requirements) {
    const result = evaluateObservedRequirement(observed, decodeRunRequirement(requirement, observed));
    if (result.status !== "Pass") throw new Error(`Commissioning ${result.id}: ${result.reason}`);
  }
}
