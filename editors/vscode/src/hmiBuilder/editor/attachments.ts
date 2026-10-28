// Instrument attachments in the editor (specification 39, §6 item 5): an attachment names an
// equipment by its id or a pipe by its place, `pipe:<n>`. When pipes are deleted or split, the
// places move; an attachment follows its pipe, and goes when its pipe or its equipment goes.

import type { HmiObjectModel, HmiPageModel } from "../drawing/model";

/** The page with each attachment to a pipe moved to that pipe's new place, given by `place` from
 * its old one; an attachment whose pipe went, or whose equipment is in `gone`, is removed. */
export function movedAttachments(
  page: HmiPageModel,
  place: (index: number) => number | undefined,
  gone: string[] = [],
): HmiPageModel {
  let changed = false;
  const object = page.object.map((candidate) => {
    const attach = candidate.attach;
    if (attach === undefined) return candidate;
    const pipe = /^pipe:(\d+)$/.exec(attach);
    const next = pipe ? withPlace(place(Number(pipe[1]))) : gone.includes(attach) ? undefined : attach;
    if (next === attach) return candidate;
    changed = true;
    return withAttach(candidate, next);
  });
  return changed ? { ...page, object } : page;
}

/** An object attached to `target`, or detached without one. */
export function withAttach(object: HmiObjectModel, target: string | undefined): HmiObjectModel {
  const { attach: _old, ...rest } = object;
  return target === undefined ? rest : { ...rest, attach: target };
}

function withPlace(index: number | undefined): string | undefined {
  return index === undefined ? undefined : `pipe:${index}`;
}

/** The places of the pipes that stay when the pipes at `gone` are deleted. */
export function afterDeleting(gone: Set<number>): (index: number) => number | undefined {
  return (index) => (gone.has(index) ? undefined : index - [...gone].filter((removed) => removed < index).length);
}
