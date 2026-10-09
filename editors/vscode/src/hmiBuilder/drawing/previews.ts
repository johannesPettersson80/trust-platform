// The states the editor's switcher shows (specification 39, §5.3 item 6): without a connected
// source, an object is drawn in each state it can show. A symbol names its own states; every
// object with a binding can also be shown stale and unknown, and a non-symbol commanded and
// estimated. A preview changes only the values drawn; it is never saved.

import type { HmiObjectModel, LiveValue } from "./model";
import { symbolFor } from "./symbols";
import { resolveState, StateValue } from "./symbols/reading";

/** A data state that applies to every bound value (§5.3, items 1 and 2). */
export type DataState = "commanded" | "estimated" | "stale" | "unknown";

export interface PreviewState {
  id: string;
  /** Its name on the switcher. */
  name: string;
  /** For a symbol's state: the values of the bindings that show it. */
  values?: Record<string, StateValue>;
  /** For a data state: what it does to every bound value. */
  data?: DataState;
  /** The bindings it needs, each written `a|b` when either will do. */
  needs: string[];
}

/** How old a stale preview's values are. */
const STALE_MS = 12_000;

const DATA_NAMES: Record<DataState, string> = {
  commanded: "Commanded",
  estimated: "Estimated",
  stale: "Stale",
  unknown: "Unknown",
};

/** The states an object of `type` can be previewed in, the sample first; `properties` are the
 * component's bindings, which the data states need one of. */
export function previewStates(type: string, properties: string[] = []): PreviewState[] {
  const sample: PreviewState = { id: "sample", name: "Sample", needs: [] };
  const symbol = symbolFor(type);
  const data = (ids: DataState[], bindings: string[]): PreviewState[] =>
    ids.map((id) => ({ id, name: DATA_NAMES[id], data: id, needs: bindings.length > 0 ? [bindings.join("|")] : [] }));
  if (symbol) return [sample, ...symbol.states, ...data(["stale", "unknown"], symbol.properties)];
  return [sample, ...data(["commanded", "estimated", "stale", "unknown"], properties)];
}

/** The values drawn for an object in a state: its bound values changed, the others kept. */
export function previewValues(
  object: HmiObjectModel,
  state: PreviewState,
  values: Record<string, LiveValue>,
): Record<string, LiveValue> {
  if (!state.values && !state.data) return values;
  const next = { ...values };
  for (const [property, binding] of Object.entries(object.bind)) {
    if (state.values) {
      if (property in state.values)
        next[binding.path] = { value: resolveState(state.values[property], object), state: "confirmed" };
    } else if (state.data) {
      next[binding.path] = withDataState(values[binding.path] ?? { value: null, state: "unknown" }, state.data);
    }
  }
  return next;
}

/** A value in a data state: stale with an age, unknown, commanded towards a target, or estimated. */
export function withDataState(value: LiveValue, data: DataState): LiveValue {
  switch (data) {
    case "stale":
      return { ...value, staleMs: STALE_MS };
    case "unknown":
      return { value: null, state: "unknown" };
    case "commanded":
      return { ...value, state: "commanded", target: typeof value.value === "boolean" ? !value.value : 70 };
    case "estimated":
      return { ...value, state: "estimated" };
  }
}

/** The bindings a state needs that the object lacks; of alternatives, the first is named. */
export function missingBindings(object: HmiObjectModel, state: PreviewState): string[] {
  return state.needs
    .filter((need) => !need.split("|").some((name) => object.bind[name] !== undefined))
    .map((need) => need.split("|")[0]);
}
