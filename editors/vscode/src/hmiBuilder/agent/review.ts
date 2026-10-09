import type { HmiObjectModel, HmiPageModel } from "../drawing/model";

export interface PageChanges {
  added: string[];
  removed: string[];
  modified: Array<{ id: string; properties: string[] }>;
  pageProperties: string[];
  orderChanged: boolean;
}
function changedProperties(before: object, after: object, excluded: string[] = []): string[] {
  const previous = before as Record<string, unknown>,
    next = after as Record<string, unknown>;
  return [...new Set([...Object.keys(before), ...Object.keys(after)])]
    .sort()
    .filter((key) => !excluded.includes(key) && JSON.stringify(previous[key]) !== JSON.stringify(next[key]));
}
/** Compare the native models, retaining stable identities and authored-property differences. */
export function describePageChanges(before: HmiPageModel | undefined, after: HmiPageModel): PageChanges {
  const previous = new Map<string, HmiObjectModel>((before?.object ?? []).map((object) => [object.id, object]));
  const next = new Set(after.object.map((object) => object.id));
  return {
    added: after.object.filter((object) => !previous.has(object.id)).map((object) => object.id),
    removed: [...previous.keys()].filter((id) => !next.has(id)),
    modified: after.object.flatMap((object) => {
      const original = previous.get(object.id);
      if (!original) return [];
      const properties = changedProperties(original, object, ["id"]);
      return properties.length ? [{ id: object.id, properties }] : [];
    }),
    pageProperties: changedProperties(before ?? {}, after, ["object"]),
    orderChanged:
      JSON.stringify([...previous.keys()].filter((id) => next.has(id))) !==
      JSON.stringify(after.object.filter((object) => previous.has(object.id)).map((object) => object.id)),
  };
}
