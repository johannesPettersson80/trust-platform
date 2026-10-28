// A labelled placeholder for a component type this drawing module does not know.

import type { HmiObjectModel } from "../model";
import { token } from "../parts";
import { el, SvgNode, text } from "../svg";

export function drawPlaceholder(object: HmiObjectModel): SvgNode {
  const [width, height] = object.size;
  return el("g", { class: "hmi-object hmi-placeholder", "data-placeholder": "true" }, [
    el("rect", {
      x: 0.5,
      y: 0.5,
      width: Math.max(0, width - 1),
      height: Math.max(0, height - 1),
      rx: 14,
      fill: "none",
      stroke: token("border"),
      "stroke-width": 1.5,
      "stroke-dasharray": "6 5",
    }),
    text(object.type, {
      x: width / 2,
      y: height / 2 + 5,
      fill: token("muted"),
      "font-size": 14,
      "font-weight": 500,
      "text-anchor": "middle",
    }),
  ]);
}
