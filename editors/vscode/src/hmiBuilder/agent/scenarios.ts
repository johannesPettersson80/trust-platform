import type { Catalogue } from "../editor/catalogue";
import type { DrawFrame, HmiPageModel } from "../drawing/model";
import { previewStates, previewValues } from "../drawing/previews";

/** Fixed synthetic samples use the shipped preview semantics and control outcome shapes.
 * No session, transport, command callback or runtime write is available here. */
export function scenarioFrame(
  models: HmiPageModel[],
  initial: DrawFrame["values"],
  state: string,
  catalogue: Catalogue,
): { values: DrawFrame["values"]; frame: Partial<DrawFrame>; unsupported: string[] } {
  let values = { ...initial };
  const frame: Partial<DrawFrame> = {};
  const unsupported: string[] = [];
  const generic = ["normal", "sample", "pending", "denied", "minimum", "maximum", "longest"].includes(state);
  for (const object of models.flatMap((model) => model.object)) {
    const preview = previewStates(object.type, Object.keys(object.bind)).find((candidate) => candidate.id === state);
    if (preview) values = previewValues(object, preview, values);
    else if (!generic) unsupported.push(object.id);
    if (state === "minimum" || state === "maximum")
      for (const binding of Object.values(object.bind)) {
        const current = values[binding.path];
        if (typeof current?.value === "number")
          values[binding.path] = { ...current, value: state === "minimum" ? (object.min ?? 0) : (object.max ?? 100) };
      }
    const control = catalogue.components.find((component) => component.type === object.type)?.category === "control";
    if (control && (state === "pending" || state === "denied")) {
      frame.commands ??= {};
      frame.commands[object.id] = {
        outcome: state === "pending" ? "pending" : "refused",
        reason: state === "pending" ? "Waiting for readback (synthetic)" : "Operator permission denied (synthetic)",
      };
    }
  }
  if (state === "longest") frame.longest = true;
  return { values, frame, unsupported };
}
