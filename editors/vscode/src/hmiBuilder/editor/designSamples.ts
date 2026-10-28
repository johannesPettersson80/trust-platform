// Design samples (specification 39, §3.4): without a connected source the canvas draws each bound
// value as a sample, so a page looks finished while it is designed. Samples never leave the editor.

import type { HmiObjectModel, HmiPageModel, LiveValue, StructuredValue } from "../drawing/model";
import { SYMBOLS, SymbolType } from "../drawing/symbols";
import { resolveState } from "../drawing/symbols/reading";
import type { Catalogue } from "./catalogue";

/** A sample for each bound path: a symbol shows its sample state (§5.3 item 6); otherwise the
 * first kind a faceplate's field takes (§8.7 item 6), else the first kind a property accepts,
 * decides it. */
export function designValues(page: HmiPageModel, catalogue: Catalogue): Record<string, LiveValue> {
  const values: Record<string, LiveValue> = {};
  const ranged = new Set<string>();
  for (const object of page.object) {
    const component = catalogue.components.find((candidate) => candidate.type === object.type);
    const symbol = SYMBOLS[object.type as SymbolType];
    const shown = symbol?.states.find((state) => state.id === symbol.sample)?.values ?? {};
    for (const [property, binding] of Object.entries(object.bind)) {
      const accepts = component?.properties?.[property]?.accepts ?? component?.additional?.accepts ?? ["value"];
      const state = component?.category === "equipment" && accepts[0] === "boolean";
      const field = binding.path.startsWith("$") ? page.fields?.[binding.path.slice(1)]?.accepts[0] : undefined;
      const value =
        property in shown
          ? resolveState(shown[property], object)
          : state
            ? EQUIPMENT_NORMAL.includes(property)
            : sample(
                binding.range ? { ...object, min: binding.range[0], max: binding.range[1] } : object,
                binding.path,
                field && field !== "value" ? field : accepts[0],
              );
      const explicitRange = binding.range !== undefined || object.min !== undefined || object.max !== undefined;
      if (typeof value === "number" && explicitRange && !ranged.has(binding.path)) {
        values[binding.path] = { value, state: "confirmed" };
        ranged.add(binding.path);
      } else values[binding.path] ??= { value, state: "confirmed" };
    }
  }
  return values;
}

/** Equipment shows its normal state (§5.3, item 4): running or open, and its other states off. */
const EQUIPMENT_NORMAL = ["running", "open"];

function sample(object: HmiObjectModel, path: string, kind: string | undefined): StructuredValue {
  // A chooser shows one of its own choices (§5.8).
  if (object.type === "selector" || object.type === "mode-selector")
    return object.options?.[0] ?? (object.type === "mode-selector" ? "Auto" : 0);
  // A text shows the name it is bound to: a variable's last member, or a field's name.
  if (object.type === "text" || kind === "enumeration" || kind === "text")
    return path.split(".").pop()?.replace(/^\$/, "") ?? path;
  if (kind === "boolean") return true;
  if (kind === "array" || kind === "structure") return [];
  const min = object.min ?? 0;
  const max = object.max ?? 100;
  return Number((min + (max - min) * 0.6).toFixed(6));
}
