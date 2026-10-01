import type { ControlLoopDefinition } from "./loopDefinition";
import type { PidControllerGroup } from "./pidControllerGroup";
import { componentSymbols } from "./projectTuningComponents";

export function controllerInstanceLabel(controller: { instance: string; group?: PidControllerGroup }): string {
  return controller.group?.members.map(member => member.instance).join(" + ") ?? controller.instance;
}

/** Fixed adapter coefficients stay in the retained record, outside the tuning controls. A group designed from its model
 * (Specification 35 §17.9) tunes every member's proportional and integral gain. */
export function groupSettingSymbols(loop: ControlLoopDefinition): Set<string> | undefined {
  if (!loop.controller.group) return undefined;
  const designed = ["decentralized-pid", "decoupled-pid"].includes(loop.controller.project?.seed?.kind ?? "")
    ? loop.controller.group.members.flatMap(member => member.parameterSymbols.filter(symbol => /\.(ProportionalGain|IntegralGain)$/.test(symbol))) : [];
  return new Set([...(loop.controller.project?.axes.map(axis => axis.symbol) ?? []), ...designed,
    ...(loop.controller.project?.components?.flatMap(component => componentSymbols(component)) ?? [])]);
}

/** The names of `Control.PID`'s gains wherever Studio shows them: a single loop, a group member and the manual editor. */
export const PID_GAIN_LABELS = { ProportionalGain: "Proportional gain (Kp)", IntegralGain: "Integral gain (Ki)", DerivativeGain: "Derivative gain (Kd)" } as const;

export function ownedParameterLabel(loop: ControlLoopDefinition, symbol: string, label: string): string {
  const owner = loop.controller.group?.members.find(member => member.parameterSymbols.includes(symbol));
  if (!owner) return label;
  const field = label.replace(/^Control\.PID\s*/, "").replace(/\s+/g, "");
  return `${owner.instance} · ${PID_GAIN_LABELS[field as keyof typeof PID_GAIN_LABELS] ?? label.replace(/^Control\.PID\s*/, "").replace(/([a-z0-9])([A-Z])/g, "$1 $2")}`;
}

export function groupChannelOwner(loop: ControlLoopDefinition, kind: "input" | "output", id: string): string | undefined {
  return loop.controller.group?.members.find(member => (kind === "input" ? member.inputChannelId : member.outputChannelId) === id)?.instance;
}
