import { isDeepStrictEqual } from "util";
import { assertPidGroupSignalMapping } from "./pidControllerGroup";
import type { ControlLoopDefinition } from "./loopDefinition";
import { loopSignal, validateLoopChannelBindings, validLoopChannels, type LoopChannels } from "./loopChannels";
import type { EngineerRequirements } from "./engineerVerificationPlan";

/** Channel edits change assignments, never the declared actuator/state authority. */
export function controllerChannelMapping(loop: ControlLoopDefinition, value: unknown): LoopChannels {
  const declared = loop.channels;
  if (!declared || !validLoopChannels(value)) throw new Error("Choose a complete declared channel mapping.");
  assertPidGroupSignalMapping(loop, value);
  for (const kind of ["inputs", "outputs"] as const) {
    if (value[kind].length !== declared[kind].length) throw new Error("Map every selected plant channel.");
    for (const [index, channel] of value[kind].entries()) {
      const original = declared[kind][index];
      if (!declared[kind].some(item => item.signalId === channel.signalId)
        || !isDeepStrictEqual({ ...channel, signalId: original.signalId }, original)
        || loopSignal(loop, channel.signalId).unit !== loopSignal(loop, original.signalId).unit) {
        throw new Error(`Review the plant signal for ${original.modelChannelId}; keep its declared limits and channel order.`);
      }
    }
  }
  if (!isDeepStrictEqual(value.initialState, declared.initialState)) throw new Error("Channel mapping cannot replace controller initial states.");
  validateLoopChannelBindings({ ...loop, channels: value });
  return structuredClone(value);
}

export function controllerRequirementLoop(loop: ControlLoopDefinition, mapping: LoopChannels | undefined,
  task: Partial<EngineerRequirements> | undefined): ControlLoopDefinition {
  const channels = mapping ?? loop.channels;
  if (!channels) return loop;
  return { ...loop, channels: { ...channels, outputs: channels.outputs.map(output => ({ ...output,
    ...(task?.channelRequirements?.[output.modelChannelId] ? { requirements: task.channelRequirements[output.modelChannelId] } : {}) })) } };
}

export class ChannelRequirementsRequired extends Error { readonly missing = "requirements"; }

export function assertControllerRequirements(loop: ControlLoopDefinition, task: EngineerRequirements): void {
  const outputs = loop.channels?.outputs ?? [];
  if (Object.keys(task.channelRequirements ?? {}).some(id => !outputs.some(output => output.modelChannelId === id && output.setpointSignalId))) {
    throw new ChannelRequirementsRequired("Review Requirements for the selected controlled outputs.");
  }
  for (const output of outputs) {
    if (output.setpointSignalId && output.signalId !== "processVariable" && !output.requirements && !task.channelRequirements?.[output.modelChannelId]) {
      throw new ChannelRequirementsRequired(`Enter control requirements for ${loopSignal(loop, output.signalId).symbol}.`);
    }
  }
}
