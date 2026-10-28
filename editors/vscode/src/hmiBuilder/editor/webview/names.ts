// Names shown in the editor.

import { symbolTitle } from "../../drawing/symbols";
import type { ComponentDefinition } from "../catalogue";

/** Names that keep their capitals. */
const ACRONYMS: Record<string, string> = {
  vfd: "Variable-speed drive",
  "flow-sensor": "Flow instrument",
  "pressure-sensor": "Pressure instrument",
  "temperature-sensor": "Temperature instrument",
  "level-sensor": "Level instrument",
  analyzer: "Analyzer instrument",
};

/** A component's name for people: `split-bar` becomes "Split bar", and `vfd` "VFD". */
export function componentName(type: string): string {
  const named = ACRONYMS[type] ?? symbolTitle(type);
  if (named) return named;
  const words = type.split("-").join(" ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/** A group of the Components panel. */
export type PaletteGroup =
  | "equipment"
  | "instruments"
  | "controllers"
  | "sequences"
  | "my-equipment"
  | "shapes"
  | "symbols"
  | "my-symbols"
  | "display"
  | "control"
  | "navigation";

/** The Components panel's groups (specification 39, §21.3 item 1): the plant equipment first,
 * then the general elements. */
export const GROUPS: Array<{ id: PaletteGroup; name: string }> = [
  { id: "equipment", name: "Equipment" },
  { id: "instruments", name: "Instruments" },
  { id: "controllers", name: "Controllers" },
  { id: "sequences", name: "Groups and sequences" },
  { id: "my-equipment", name: "My equipment" },
  { id: "shapes", name: "Basic shapes" },
  { id: "symbols", name: "Symbols" },
  { id: "my-symbols", name: "My Symbols" },
  { id: "display", name: "Displays" },
  { id: "control", name: "Controls" },
  { id: "navigation", name: "Navigation" },
];

/** The group a component is listed under. Equipment with a component definition is listed by
 * what it is: drawn equipment, an instrument, a controller, or a group, sequence or condition;
 * artwork without one is a symbol. */
export function paletteGroup(component: ComponentDefinition): PaletteGroup {
  const definition = component.equipment;
  if (definition) {
    // The project's own blocks, proposed or defined by the project (§21.9).
    if (definition.mine) return "my-equipment";
    if (definition.symbol === "controller") return "controllers";
    if (component.group !== undefined && component.group !== "instruments") return "equipment";
    if (component.group === "instruments" || definition.position === "measurements") return "instruments";
    return "sequences";
  }
  if (component.category === "equipment") return component.group === "my-symbols" ? "my-symbols" : "symbols";
  if (component.category === "layout") return component.type === "page-button" ? "navigation" : "shapes";
  return component.category === "control" ? "control" : "display";
}

/** Whether a component belongs to a group of the Components panel. */
export function inGroup(component: ComponentDefinition, group: (typeof GROUPS)[number]): boolean {
  return paletteGroup(component) === group.id;
}
