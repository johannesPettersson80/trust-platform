import type { HmiPageModel } from "./model";

/** Gesture controllers work in screen coordinates. The drawing keeps definition-local
 * coordinates inside clipped SVG regions, so this read-only view supplies their origins. */
export function screenCoordinates(page: HmiPageModel): HmiPageModel {
  if (!page.regions?.length) return page;
  const offsets = new Map(page.regions.flatMap((region) => region.objects.map((id) => [id, region.at] as const)));
  return {
    ...page,
    regions: undefined,
    object: page.object.map((object) => {
      const offset = offsets.get(object.id);
      return offset ? { ...object, at: [object.at[0] + offset[0], object.at[1] + offset[1]] } : object;
    }),
  };
}
