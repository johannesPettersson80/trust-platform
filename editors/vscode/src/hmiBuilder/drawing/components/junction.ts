// A junction (specification 39, §6, item 2): where pipes meet, a dot in the pipes' colour; its
// ports are its centre, one on each side.

import type { HmiObjectModel } from "../model";
import { token } from "../parts";
import { el, SvgNode } from "../svg";

export function drawJunction(object: HmiObjectModel): SvgNode {
  const [width, height] = object.size;
  return el("g", { class: "hmi-object hmi-junction" }, [
    el("circle", {
      "data-part": "dot",
      cx: width / 2,
      cy: height / 2,
      r: Math.max(3, Math.min(width, height) / 4),
      fill: token("muted"),
    }),
  ]);
}
