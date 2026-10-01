import type { ControlLoopDefinition } from "./loopDefinition";
import { loopSignal } from "./loopChannels";
import type { TransferBinding, TransferMember } from "./proportionalTransfer";

/** Track measured applied action when command and actuator differ, in explicitly matching coordinates. */
export function withActuatorFeedback(loop: ControlLoopDefinition, binding: TransferBinding): TransferBinding {
  const guards = [...binding.guards], tracking_outputs: Record<string, string> = {};
  const map = (output: TransferMember) => {
    const channel = loop.channels?.inputs.find(input => loopSignal(loop, input.signalId).symbol === output.symbol);
    const signal = channel ? loopSignal(loop, channel.signalId) : loop.signals.manipulatedVariable;
    const applied = channel?.appliedSignalId ? loopSignal(loop, channel.appliedSignalId)
      : output.symbol === loop.signals.manipulatedVariable.symbol ? loop.signals.actuator : undefined;
    if (!applied) return output;
    if (applied.unit !== output.unit || !applied.address?.startsWith("%I") || !["REAL", "LREAL", "BOOL"].includes(applied.iecType)) throw new Error("Actuator tracking needs a separate typed input in command coordinates.");
    const member: TransferMember = { symbol: applied.symbol, iec_type: applied.iecType as TransferMember["iec_type"], unit: applied.unit,
      minimum: output.minimum, maximum: output.maximum };
    const existing = guards.find(guard => guard.symbol === member.symbol);
    if (existing && (existing.unit !== member.unit || existing.iec_type !== member.iec_type)) throw new Error("Applied feedback disagrees with another bound measurement.");
    if (!existing) guards.push(member);
    tracking_outputs[output.symbol] = member.symbol;
    return { ...output, minimum: signal.range.minimum, maximum: signal.range.maximum };
  };
  const output = map(binding.output), additional_outputs = binding.additional_outputs?.map(map);
  if (!Object.keys(tracking_outputs).length) return binding;
  return { ...binding, output, ...(additional_outputs ? { additional_outputs } : {}), guards, tracking_outputs };
}
export function transferOutputBasis(binding: TransferBinding, output: TransferMember): TransferMember {
  const feedback = binding.tracking_outputs?.[output.symbol];
  return feedback ? binding.guards.find(guard => guard.symbol === feedback)! : output;
}
export function transferTargetAllowance(binding: TransferBinding): number {
  const primary = transferOutputBasis(binding, binding.output);
  const span = primary.maximum - primary.minimum;
  return Math.max(...[binding.output, ...(binding.additional_outputs ?? [])].map(output => {
    const basis = transferOutputBasis(binding, output), width = basis.maximum - basis.minimum;
    return width > 0 ? (output.maximum - output.minimum) / width * span : 0;
  }));
}
