import type { GeneralModel, GeneralStateSpaceModel } from "./generalModel";
import { designPidToRequirements } from "./automaticPidResponse";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { loopSignal } from "./loopChannels";
import type { ControlLoopDefinition } from "./loopDefinition";
import { firstOrderPathModel, sampledStateSpace, settlingHorizon, singleLoopView, twoPointPath } from "./pathReduction";

/** The gain variables of one `Control.PID` of a cascade. */
export interface CascadeGains { instance: string; proportionalSymbol: string; integralSymbol: string; derivativeSymbol: string }
/** A cascade designed inner loop first, the outer loop through the closed inner loop (Specification 35 §17.11). */
export interface CascadePidSeed { kind: "cascade-pid"; family: "PI"; inner: CascadeGains; outer: CascadeGains }

export function designCascade(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements, seed: CascadePidSeed): Record<string, number> {
  const channels = loop.channels;
  if (!channels || channels.inputs.length !== 1 || channels.outputs.length !== 2 || model.kind === "transfer-function") {
    throw new Error("A cascade is designed on one model from the plant input to both measurements.");
  }
  // The inner measurement has no setpoint of its own: the outer controller sets it.
  const innerChannel = channels.outputs.find(output => !output.setpointSignalId), outerChannel = channels.outputs.find(output => output.setpointSignalId);
  if (!innerChannel || !outerChannel) throw new Error("A cascade needs an inner measurement without and an outer measurement with a setpoint.");
  const innerIndex = model.outputs.findIndex(output => output.id === innerChannel.modelChannelId);
  const outerIndex = model.outputs.findIndex(output => output.id === outerChannel.modelChannelId);
  if (innerIndex < 0 || outerIndex < 0 || model.inputs.length !== 1) throw new Error("The model's channels do not match the cascade's plant input and measurements.");
  const plantInput = loopSignal(loop, channels.inputs[0].signalId), inner = loopSignal(loop, innerChannel.signalId);
  const outer = loopSignal(loop, outerChannel.signalId), setpoint = loopSignal(loop, outerChannel.setpointSignalId!);
  const period = loop.controller.taskPeriodMs / 1000;
  const sampled = sampledStateSpace(model, period, "cascade");
  const horizon = settlingHorizon(model, "cascade");
  const operating = model.operatingPoint;

  // The inner controller first, on the path from the plant input to the inner measurement.
  const open = simulate(sampled, model.delaySeconds, horizon, innerIndex, outerIndex);
  const innerPath = twoPointPath(open.times, open.values, "cascade inner");
  const outerTask = channelTask(task, outerChannel.modelChannelId) ?? task;
  const ratio = (inner.range.maximum - inner.range.minimum) / (outer.range.maximum - outer.range.minimum);
  // Without its own requirements the inner loop settles in a fifth of the outer time, so it follows its setpoint.
  const innerTask = channelTask(task, innerChannel.modelChannelId) ?? { ...outerTask, referenceChange: outerTask.referenceChange * ratio,
    maximumError: outerTask.maximumError * ratio, settlingBand: outerTask.settlingBand * ratio,
    settlingTimeSeconds: outerTask.settlingTimeSeconds / 5, durationSeconds: 2 * outerTask.settlingTimeSeconds / 5, channelRequirements: undefined };
  const innerDesign = designPidToRequirements(singleLoopView(loop, inner, plantInput, inner, loop.limits.outputMinimum, loop.limits.outputMaximum),
    firstOrderPathModel(model, innerPath, plantInput, inner, operating?.inputs[0] ?? 0, operating?.outputs[innerIndex] ?? 0), "PI", innerTask).candidate;

  // Then the outer controller, on the outer measurement's response to an inner setpoint step through the closed inner loop.
  const closed = simulate(sampled, model.delaySeconds, horizon, innerIndex, outerIndex,
    { proportionalGain: innerDesign.proportionalGain, integralGain: innerDesign.integralGain, period });
  const outerPath = twoPointPath(closed.times, closed.values, "cascade outer");
  const outerDesign = designPidToRequirements(singleLoopView(loop, outer, inner, setpoint, inner.range.minimum, inner.range.maximum),
    firstOrderPathModel(model, outerPath, inner, outer, operating?.outputs[innerIndex] ?? 0, operating?.outputs[outerIndex] ?? 0), "PI", outerTask).candidate;
  return {
    [seed.inner.proportionalSymbol]: innerDesign.proportionalGain, [seed.inner.integralSymbol]: innerDesign.integralGain, [seed.inner.derivativeSymbol]: 0,
    [seed.outer.proportionalSymbol]: outerDesign.proportionalGain, [seed.outer.integralSymbol]: outerDesign.integralGain, [seed.outer.derivativeSymbol]: 0,
  };
}

function channelTask(task: EngineerRequirements, channel: string): EngineerRequirements | undefined {
  const own = task.channelRequirements?.[channel];
  return own ? { ...task, ...own, channelRequirements: undefined } : undefined;
}

/**
 * The unit-step response of the sampled model: open loop from the plant input, or, with the inner PI, closed around the
 * inner measurement from a step of its setpoint. Returns the times and the measurement the design reduces.
 */
function simulate(model: GeneralStateSpaceModel, delaySeconds: number, horizon: number, innerIndex: number, outerIndex: number,
  innerPi?: { proportionalGain: number; integralGain: number; period: number }): { times: number[]; values: number[] } {
  const h = model.sampleTimeSeconds!, { a, b, c, d } = model;
  const delaySteps = Math.round(delaySeconds / h), steps = Math.min(200_000, Math.ceil(horizon / h));
  const history: number[] = Array(delaySteps).fill(0);
  let state: number[] = Array(a.length).fill(0), integral = 0;
  const times: number[] = [], values: number[] = [];
  const output = (row: number, applied: number) => c[row].reduce((sum, weight, index) => sum + weight * state[index], 0) + d[row][0] * applied;
  for (let step = 0; step <= steps; step++) {
    const error = innerPi ? 1 - output(innerIndex, history[0] ?? 0) : 0;
    const command = innerPi ? innerPi.proportionalGain * error + integral : 1;
    history.push(command);
    const applied = history.shift()!;
    times.push(step * h); values.push(output(innerPi ? outerIndex : innerIndex, applied));
    if (innerPi) integral += innerPi.integralGain * error * h;
    state = a.map((row, index) => row.reduce((sum, weight, column) => sum + weight * state[column], 0) + b[index][0] * applied);
  }
  return { times, values };
}
