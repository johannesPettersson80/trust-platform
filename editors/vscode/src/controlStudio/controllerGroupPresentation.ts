import type { PidControllerGroup } from "./generated/PidControllerGroup";

export function controllerInstanceLabel(controller: { instance: string; group?: PidControllerGroup }): string {
  return controller.group?.members.map(member => member.instance).join(" + ") ?? controller.instance;
}
