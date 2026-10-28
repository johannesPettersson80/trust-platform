// The current values of the variables the pickers show (specification 39, §3.3 item 8): each tree
// says which variables it shows, the host reads them from the run target, and each row shows its
// value; none while no run target answers.

import { createContext } from "react";

import type { LiveUpdate } from "../messages";

export interface PickerValuesContext {
  /** The run target's last answer, or none. */
  update?: LiveUpdate;
  /** The variables a tree shows, by the tree's key; none when it closes. */
  show(tree: string, paths: string[]): void;
}

export const PickerValues = createContext<PickerValuesContext>({ show: () => undefined });

/** A variable's current value as the picker writes it: TRUE or FALSE, a number, a text in quotes
 * (so an empty one shows), a time or date as the runtime writes it, and "?" when the running
 * program does not have it or its value is unknown. */
export function currentText(update: LiveUpdate | undefined, path: string, type = ""): string | undefined {
  if (!update) return undefined;
  if (update.missing?.includes(path)) return "?";
  const wire = update.values[path];
  if (wire === undefined) return undefined;
  if (wire === null) return "?";
  const value = wire.value;
  if (typeof value === "boolean") return value ? "TRUE" : "FALSE";
  if (typeof value === "number") return Number.isInteger(value) ? String(value) : String(Number(value.toPrecision(6)));
  if (typeof value === "string") return /^W?STRING\b/i.test(type) ? `'${value}'` : value;
  return JSON.stringify(value);
}
