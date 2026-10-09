// A loop controller's signal lines from its configuration (specification 39, §6 item 5): the
// connected equipment that drives nothing is its measurement, the one that drives an input is the
// equipment it acts on; each line goes to the object of the page that shows that instance. Shared by
// the builder's webview, which keeps a controller's lines with its configuration.

import type { HmiObjectModel, HmiPageModel } from "../drawing/model";

/** A controller's connected equipment as its details give it. */
export interface LoopDependency {
  drives?: boolean;
  value?: string | null;
}

/** The ids of the page objects a controller's lines go to, from its connected equipment. */
export function loopSignals(
  page: HmiPageModel,
  dependencies: LoopDependency[],
): { measures?: string; acts_on?: string } {
  const shown = (path: string | null | undefined): string | undefined =>
    path ? page.object.find((object) => object.instance?.toLowerCase() === path.toLowerCase())?.id : undefined;
  return {
    measures: shown(dependencies.find((dependency) => !dependency.drives && dependency.value)?.value),
    acts_on: shown(dependencies.find((dependency) => dependency.drives && dependency.value)?.value),
  };
}

/** The controller with its lines as its configuration has them, or undefined when they already
 * are. */
export function withLoopSignals(
  object: HmiObjectModel,
  page: HmiPageModel,
  dependencies: LoopDependency[],
): HmiObjectModel | undefined {
  const { measures, acts_on } = loopSignals(page, dependencies);
  if (object.measures === measures && object.acts_on === acts_on) return undefined;
  const next: HmiObjectModel = { ...object };
  delete next.measures;
  delete next.acts_on;
  return { ...next, ...(measures ? { measures } : {}), ...(acts_on ? { acts_on } : {}) };
}
