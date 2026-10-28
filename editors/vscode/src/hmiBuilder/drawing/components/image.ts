// Image: a project asset, fitted inside its box without distortion (§5.1).

import type { DrawFrame, HmiObjectModel } from "../model";
import { token } from "../parts";
import { el, SvgNode, text } from "../svg";
import { say } from "../speech";

export function drawImage(object: HmiObjectModel, drawFrame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  if (!object.image) {
    return el("g", { class: "hmi-object hmi-image" }, [
      el("rect", {
        x: 0.5,
        y: 0.5,
        width: width - 1,
        height: height - 1,
        rx: 14,
        fill: "none",
        stroke: token("border"),
        "stroke-dasharray": "6 5",
      }),
      text(say("No image"), {
        x: width / 2,
        y: height / 2 + 5,
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 500,
        "text-anchor": "middle",
      }),
    ]);
  }
  const href = drawFrame.asset ? drawFrame.asset(object.image) : object.image;
  return el("g", { class: "hmi-object hmi-image" }, [
    el("image", { href, x: 0, y: 0, width, height, preserveAspectRatio: "xMidYMid meet" }),
  ]);
}
