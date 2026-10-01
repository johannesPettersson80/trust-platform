import type { GeneralModel } from "./generalModel";
import type { ControlLoopDefinition } from "./loopDefinition";
import { connectedModelChannels } from "./loopChannels";

/** Supplied and fitted models share the same channel and evidence boundary. */
export function assertEngineerModelCompatibility(loop: ControlLoopDefinition, model: GeneralModel): void {
  assertModelEvidence(model);
  const channels = connectedModelChannels(loop, model);
  assertChannelUnitsAndRanges(model, channels);
  if (!channels.outputs.some(output => output.reference)) throw new Error("Declare at least one controlled output and its setpoint binding.");
  if (model.kind === "discrete-state-space" && Math.abs(model.sampleTimeSeconds! * 1000 - loop.controller.taskPeriodMs) > 1e-9) {
    throw new Error("Selected discrete model sample time must match the controller task period.");
  }
}

/** Model qualification is required even when controller channel mapping is still missing. */
export function assertQualifiedModelResult(loop: ControlLoopDefinition, model: GeneralModel): void {
  assertModelEvidence(model);
  const configured = loop.channels;
  const mapped = configured
    ? configured.inputs.length === model.inputs.length && configured.outputs.length === model.outputs.length
      && model.inputs.every(channel => configured.inputs.some(input => input.modelChannelId === channel.id))
      && model.outputs.every(channel => configured.outputs.some(output => output.modelChannelId === channel.id))
    : model.inputs.length === 1 && model.outputs.length === 1;
  if (mapped) assertChannelUnitsAndRanges(model, connectedModelChannels(loop, model));
}

function assertModelEvidence(model: GeneralModel): void {
  if (model.evidence.length === 0 || model.validation !== "validated" && !(model.source === "fitted" && model.validation === "unvalidated")) {
    throw new Error("The model needs measurement evidence or independent validation before tuning.");
  }
}

function assertChannelUnitsAndRanges(model: GeneralModel, channels: ReturnType<typeof connectedModelChannels>): void {
  for (const [index, input] of channels.inputs.entries()) {
    const range = model.inputs[index].operatingRange;
    if (range.maximum < input.actuator.minimum || range.minimum > input.actuator.maximum) throw new Error(`Selected model input ${model.inputs[index].id} has no usable actuator range.`);
    if (model.inputs[index].unit !== input.signal.unit) throw new Error(`Selected model input ${model.inputs[index].id} unit does not match ${input.signal.symbol}.`);
  }
  for (const [index, output] of channels.outputs.entries()) {
    const range = model.outputs[index].operatingRange;
    if (range.maximum < output.signal.range.minimum || range.minimum > output.signal.range.maximum) throw new Error(`Selected model output ${model.outputs[index].id} has no usable measurement range.`);
    if (model.outputs[index].unit !== output.signal.unit) throw new Error(`Selected model output ${model.outputs[index].id} unit does not match ${output.signal.symbol}.`);
    if (output.reference && output.reference.unit !== output.signal.unit) throw new Error(`Setpoint unit does not match ${output.signal.symbol}.`);
  }
}
