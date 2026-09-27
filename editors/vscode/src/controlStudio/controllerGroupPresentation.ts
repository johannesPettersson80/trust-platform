import type { ControlLoopDefinition } from "./loopDefinition";
import type { PidControllerGroup } from "./pidControllerGroup";
import { componentSymbols } from "./projectTuningComponents";

export function controllerInstanceLabel(controller: { instance: string; group?: PidControllerGroup }): string {
  return controller.group?.members.map(member => member.instance).join(" + ") ?? controller.instance;
}

/** Fixed adapter coefficients stay in the retained record, outside the tuning controls. */
export function groupSettingSymbols(loop: ControlLoopDefinition): Set<string> | undefined {
  if (!loop.controller.group) return undefined;
  return new Set([...(loop.controller.project?.axes.map(axis => axis.symbol) ?? []),
    ...(loop.controller.project?.components?.flatMap(component => componentSymbols(component)) ?? [])]);
}

export function ownedParameterLabel(loop: ControlLoopDefinition, symbol: string, label: string): string {
  const owner = loop.controller.group?.members.find(member => member.parameterSymbols.includes(symbol));
  return owner ? `${owner.instance} · ${label.replace(/^Control\.PID\s*/, "").replace(/([a-z0-9])([A-Z])/g, "$1 $2")}` : label;
}

export function groupChannelOwner(loop: ControlLoopDefinition, kind: "input" | "output", id: string): string | undefined {
  return loop.controller.group?.members.find(member => (kind === "input" ? member.inputChannelId : member.outputChannelId) === id)?.instance;
}
