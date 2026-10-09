// Editor-only history (§3.4). A deterministic preview, never stored or sent to a runtime.
import type { DrawFrame, HmiPageModel } from "../drawing/model";
import { NOT_PENS } from "../drawing/components/trend";

/** All pages share an anchor so placed faceplates and the page use one time window. */
export const DESIGN_NOW_MS = 12 * 60 * 60 * 1000;

export function designHistory(pages: HmiPageModel[], values: DrawFrame["values"]): NonNullable<DrawFrame["history"]> {
  const history: NonNullable<DrawFrame["history"]> = {};
  for (const page of pages)
    for (const object of page.object) {
      if (object.type !== "trend") continue;
      for (const [name, binding] of Object.entries(object.bind)) {
        if (NOT_PENS.includes(name) || history[binding.path]) continue;
        const value = values[binding.path];
        if (
          !value ||
          value.state === "unknown" ||
          (typeof value.value !== "number" && typeof value.value !== "boolean")
        )
          continue;
        const current = Number(value.value);
        if (!Number.isFinite(current)) continue;
        const low = binding.range?.[0] ?? object.min ?? 0;
        const high = binding.range?.[1] ?? object.max ?? 100;
        const span = Math.max(1, object.span ?? 60) * 1000;
        history[binding.path] = Array.from({ length: 61 }, (_, index): [number, number] => {
          const fraction = index / 60;
          const sample =
            typeof value.value === "boolean"
              ? index === 60
                ? current
                : Math.floor(index / 15) % 2
              : current + (high - low) * 0.16 * Math.sin(fraction * 4 * Math.PI) * (1 - fraction);
          return [DESIGN_NOW_MS - span * (1 - fraction), sample];
        });
      }
    }
  return history;
}
