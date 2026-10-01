import type { GeneralModel, GeneralStateSpaceModel } from "./generalModel";
import { designPidToRequirements } from "./automaticPidResponse";
import type { EngineerRequirements } from "./engineerVerificationPlan";
import { connectedModelChannels } from "./loopChannels";
import type { ControlLoopDefinition } from "./loopDefinition";
import { firstOrderPathModel, openLoopStepResponse, sampledStateSpace, singleLoopView, twoPointPath, type ReducedPath } from "./pathReduction";

/**
 * A `Control.PID` with a feedforward from a measured disturbance: the PI's gains and the program's feedforward gain and,
 * optionally, its lead and lag times (Specification 35 §17.12).
 */
export interface FeedforwardPidSeed {
  kind: "feedforward-pid"; family: "PI";
  proportionalSymbol: string; integralSymbol: string; derivativeSymbol: string;
  gainSymbol: string; leadSymbol?: string; lagSymbol?: string;
}

/** What Tune shows for a feedforward: both paths, the three values and the predicted largest deviations. */
export interface FeedforwardDesign {
  outputPath: ReducedPath; disturbancePath: ReducedPath;
  gain: number; leadSeconds?: number; lagSeconds?: number;
  /** The disturbance step the prediction and Validate use: its identification step. */
  disturbanceStep: number;
  /** Largest deviation of the measurement from its operating value after that step, on the model. */
  largestDeviation: { feedback: number; feedforward: number };
}

/** Designs the PI on the output's path and the feedforward from both paths, each reduced to first order with delay. */
export function feedforwardDesign(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  seed: FeedforwardPidSeed): FeedforwardDesign & { values: Record<string, number> } {
  const channels = connectedModelChannels(loop, model);
  const output = channels.inputs.findIndex(input => !input.disturbance), disturbance = channels.inputs.findIndex(input => input.disturbance);
  const measurement = channels.outputs.findIndex(channel => channel.signal.symbol === loop.signals.processVariable.symbol);
  if (channels.inputs.length !== 2 || output < 0 || disturbance < 0 || measurement < 0 || channels.outputs.length !== 1) {
    throw new Error("A feedforward is designed on one model from the controller output and the measured disturbance to the measurement.");
  }
  const disturbanceStep = loop.channels?.inputs.find(input => input.modelChannelId === model.inputs[disturbance].id)?.experimentStep;
  if (!(disturbanceStep !== undefined && disturbanceStep > 0)) throw new Error("Declare the measured disturbance's identification step.");
  const path = (input: number, subject: string) => {
    const response = openLoopStepResponse(model, input, "feedforward");
    return twoPointPath(response.times, response.outputs[measurement], subject);
  };
  const outputPath = path(output, "controller output"), disturbancePath = path(disturbance, "disturbance");
  const operating = model.operatingPoint;
  const mv = channels.inputs[output].signal, pv = channels.outputs[measurement].signal;
  const pi = designPidToRequirements(singleLoopView(loop, pv, mv, loop.signals.setpoint ?? pv, loop.limits.outputMinimum, loop.limits.outputMaximum),
    firstOrderPathModel(model, outputPath, mv, pv, operating?.inputs[output] ?? 0, operating?.outputs[measurement] ?? 0), "PI", task).candidate;
  // K_ff cancels the disturbance in steady state; the lead cancels the output path's lag and the lag reproduces the disturbance's,
  // with the delay the disturbance takes longer than the output as further lag.
  const gain = -disturbancePath.gain / outputPath.gain;
  const leadLag = seed.leadSymbol && seed.lagSymbol ? { leadSeconds: outputPath.timeConstantSeconds,
    lagSeconds: disturbancePath.timeConstantSeconds + Math.max(0, disturbancePath.delaySeconds - outputPath.delaySeconds) } : undefined;
  const period = loop.controller.taskPeriodMs / 1000;
  const sampled = sampledStateSpace(model, period, "feedforward");
  const deviation = (withFeedforward: boolean) => largestDeviation(sampled, model.delaySeconds, { output, disturbance, measurement },
    { proportionalGain: pi.proportionalGain, integralGain: pi.integralGain, period }, withFeedforward ? { gain, ...leadLag } : undefined,
    disturbanceStep, task.durationSeconds);
  return {
    outputPath, disturbancePath, gain, ...leadLag, disturbanceStep,
    largestDeviation: { feedback: deviation(false), feedforward: deviation(true) },
    values: { [seed.proportionalSymbol]: pi.proportionalGain, [seed.integralSymbol]: pi.integralGain, [seed.derivativeSymbol]: 0,
      [seed.gainSymbol]: gain, ...(leadLag ? { [seed.leadSymbol!]: leadLag.leadSeconds, [seed.lagSymbol!]: leadLag.lagSeconds } : {}) },
  };
}

/**
 * The largest deviation of the measurement from its operating value after a disturbance step at the start, with the PI
 * and, when given, the feedforward `K_ff·(z + (T_lead/T_lag)·(d − z))` with `z` the disturbance through the lag.
 */
function largestDeviation(model: GeneralStateSpaceModel, delaySeconds: number, index: { output: number; disturbance: number; measurement: number },
  pi: { proportionalGain: number; integralGain: number; period: number }, feedforward: { gain: number; leadSeconds?: number; lagSeconds?: number } | undefined,
  step: number, durationSeconds: number): number {
  const h = model.sampleTimeSeconds!, { a, b, c, d } = model;
  const delaySteps = Math.round(delaySeconds / h), steps = Math.min(200_000, Math.ceil(durationSeconds / h));
  const history: number[][] = Array.from({ length: delaySteps }, () => Array<number>(b[0].length).fill(0));
  let state: number[] = Array(a.length).fill(0), integral = 0, filtered = 0, largest = 0;
  const measured = (applied: readonly number[]) => c[index.measurement].reduce((sum, weight, column) => sum + weight * state[column], 0)
    + d[index.measurement].reduce((sum, weight, column) => sum + weight * applied[column], 0);
  for (let sample = 0; sample <= steps; sample++) {
    const error = -measured(history[0] ?? Array<number>(b[0].length).fill(0));
    let command = pi.proportionalGain * error + integral;
    if (feedforward) {
      if (feedforward.leadSeconds !== undefined && feedforward.lagSeconds !== undefined) {
        filtered += h / (feedforward.lagSeconds + h) * (step - filtered);
        command += feedforward.gain * (filtered + feedforward.leadSeconds / feedforward.lagSeconds * (step - filtered));
      } else command += feedforward.gain * step;
    }
    const inputs = Array<number>(b[0].length).fill(0);
    inputs[index.output] = command; inputs[index.disturbance] = step;
    history.push(inputs);
    const applied = history.shift()!;
    largest = Math.max(largest, Math.abs(measured(applied)));
    integral += pi.integralGain * error * pi.period;
    state = a.map((row, rowIndex) => row.reduce((sum, weight, column) => sum + weight * state[column], 0)
      + b[rowIndex].reduce((sum, weight, column) => sum + weight * applied[column], 0));
  }
  return largest;
}
