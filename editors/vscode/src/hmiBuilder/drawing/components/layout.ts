// Layout components: panel, group, line and page button (§5.2).

import { fitWidth } from "../format";
import type { HmiObjectModel } from "../model";
import { card, token } from "../parts";
import { el, SvgNode, text } from "../svg";

/** A panel: a surface card that groups the objects drawn on it, with its title. */
export function drawPanel(object: HmiObjectModel): SvgNode {
  const [width, height] = object.size;
  const children: SvgNode[] = [card(width, height, undefined, 16)];
  if (object.label) {
    children.push(
      text(fitWidth(object.label, width - 36, 15, 600), {
        x: 18,
        y: 30,
        fill: token("text"),
        "font-size": 15,
        "font-weight": 600,
      }),
    );
  }
  return el("g", { class: "hmi-object hmi-panel" }, children);
}

/** A group: a titled outline around related objects, without a fill. */
export function drawGroup(object: HmiObjectModel): SvgNode {
  const [width, height] = object.size;
  const children: SvgNode[] = [
    el("rect", {
      x: 0.5,
      y: 0.5,
      width: width - 1,
      height: height - 1,
      rx: 14,
      fill: "none",
      stroke: token("border"),
      "stroke-width": 1.5,
    }),
  ];
  if (object.label) {
    // The letter spacing widens each letter by 0.3 px beyond the font's width.
    const room = width - 32 - 0.3 * Array.from(object.label).length;
    children.push(
      text(fitWidth(object.label, room, 14, 600), {
        x: 16,
        y: 26,
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 600,
        "letter-spacing": 0.3,
      }),
    );
  }
  return el("g", { class: "hmi-object hmi-group" }, children);
}

/** A line across the middle of its box; rotate the object for other directions. */
export function drawLine(object: HmiObjectModel): SvgNode {
  const [width, height] = object.size;
  return el("g", { class: "hmi-object hmi-line" }, [
    el("line", {
      x1: 0,
      y1: height / 2,
      x2: width,
      y2: height / 2,
      stroke: token("border"),
      "stroke-width": Math.max(1, Math.min(height, 4)),
      "stroke-linecap": "round",
    }),
  ]);
}

/** A page button: a pill with its label; its click names the page (§5.4). */
export function drawPageButton(object: HmiObjectModel): SvgNode {
  const [width, height] = object.size;
  const label = object.text ?? object.label ?? object.click?.page ?? "";
  return el("g", { class: "hmi-object hmi-page-button" }, [
    el("rect", {
      x: 0.5,
      y: 0.5,
      width: width - 1,
      height: height - 1,
      rx: (height - 1) / 2,
      fill: token("raised"),
      stroke: token("border"),
    }),
    text(fitWidth(label, width - 24, 15, 600), {
      x: width / 2,
      y: height / 2 + 5,
      fill: token("text"),
      "font-size": 15,
      "font-weight": 600,
      "text-anchor": "middle",
    }),
  ]);
}
