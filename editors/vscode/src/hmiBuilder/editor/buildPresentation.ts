// Static Build presentation. Runtime bindings remain in the document; this is a view only.
import type { DrawFrame, HmiPageModel, PlacedFaceplate } from "../drawing/model";

import { withValueFormat } from "../drawing/valueFormats";

export function buildPage(
  page: HmiPageModel,
  previews: Record<string, string> = {},
  formats?: DrawFrame["formats"],
): HmiPageModel {
  return {
    ...page,
    object: page.object.map((object) => {
      if (previews[object.id]) return object;
      const shown = withValueFormat(object, { formats });
      return {
        ...shown,
        ...(object.type === "value" && object.bind.value && !object.text ? { text: `—${shown.unit ?? ""}` } : {}),
        bind: {},
      };
    }),
    pipe: page.pipe.map(({ bind: _binding, ...pipe }) => pipe),
  };
}

export function buildPlacements(
  placed: Record<string, PlacedFaceplate | null>,
  formats?: DrawFrame["formats"],
): Record<string, PlacedFaceplate | null> {
  return Object.fromEntries(
    Object.entries(placed).map(([id, value]) => [
      id,
      value ? { ...value, model: { ...buildPage(value.model, {}, formats), size: value.model.size } } : null,
    ]),
  );
}
