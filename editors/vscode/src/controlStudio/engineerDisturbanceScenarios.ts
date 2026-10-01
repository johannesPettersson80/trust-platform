import type { ControlLoopDefinition } from "./loopDefinition";
import type { GeneralModel } from "./generalModel";
import { connectedModelChannels } from "./loopChannels";
import { engineerVerificationPlan, type EngineerRequirements } from "./engineerVerificationPlan";

/**
 * Specification 35 §17.12: from the operating point with the setpoint unchanged, each measured disturbance steps by its
 * identification step at the start; the measurement must be within the error allowance by the settling time.
 */
export function measuredDisturbanceScenarios(loop: ControlLoopDefinition, model: GeneralModel, task: EngineerRequirements,
  values: Readonly<Record<string, number>>) {
  const channels = connectedModelChannels(loop, model);
  const disturbances = channels.inputs.map((input, index) => ({ input, step: loop.channels?.inputs.find(channel => channel.modelChannelId === model.inputs[index].id)?.experimentStep }))
    .filter(entry => entry.input.disturbance);
  if (!disturbances.length) return [];
  const regulationLoop = loop.channels ? { ...loop, channels: { ...loop.channels, outputs: loop.channels.outputs.map(channel => ({
    ...channel, ...(channel.requirements ? { requirements: { ...channel.requirements, referenceChange: 0 } } : {}) })) } } : loop;
  const regulation = engineerVerificationPlan(regulationLoop, model, { ...task, referenceChange: 0 }, values);
  return disturbances.map(({ input, step }) => {
    if (!(step !== undefined && step > 0)) throw new Error(`Declare the identification step of the measured disturbance ${input.signal.symbol}.`);
    const start = regulation.request.initial_values[input.signal.symbol];
    const value = start + step <= input.actuator.maximum ? start + step : start - step;
    if (value < input.actuator.minimum) throw new Error(`The step of ${input.signal.symbol} does not fit its range from the operating point.`);
    return { id: disturbances.length === 1 ? "measured-disturbance" : `measured-disturbance:${input.signal.symbol}`, ...regulation,
      request: { ...regulation.request, events: [{ at_ms: 0, symbol: input.signal.symbol, value }] } };
  });
}
