import type { HmiPageModel, PageRegion } from "../drawing/model";

/** Use the native layout with the page currently being dragged. Never commit the merged
 * result: only the authored page goes into the text document. */
export function withSharedPanels(page: HmiPageModel, composition?: HmiPageModel): HmiPageModel {
  const shared = composition?.regions?.filter((region) => region.panel != null) ?? [];
  const content = composition?.regions?.find((region) => region.panel == null);
  if (!composition || !content || !shared.length) return page;
  const object = [...page.object];
  const pipe = [...page.pipe];
  const regions: PageRegion[] = [
    { ...content, objects: object.map((item) => item.id), pipes: pipe.map((_, index) => index) },
  ];
  for (const region of shared) {
    const first = pipe.length;
    const ids = new Set(region.objects);
    for (const item of composition.object.filter((item) => ids.has(item.id))) {
      const old = item.attach?.startsWith("pipe:") ? Number(item.attach.slice(5)) : undefined;
      object.push(old === undefined ? item : { ...item, attach: `pipe:${first + region.pipes.indexOf(old)}` });
    }
    pipe.push(...region.pipes.map((index) => composition.pipe[index]));
    regions.push({ ...region, pipes: region.pipes.map((_, index) => first + index) });
  }
  return { ...page, object, pipe, regions };
}
