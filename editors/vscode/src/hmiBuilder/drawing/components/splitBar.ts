// The split bar: two outputs from one request, for example heating and cooling (§5.2).

import { withValueFormat } from "../valueFormats";
import { formatValue } from "../format";
import type { DrawFrame, HmiObjectModel } from "../model";
import { ACCENT_UP, Box, SECOND_UP, token } from "../parts";
import { rangeReading } from "../ranges";
import { el, SvgNode, text } from "../svg";
import { liveValueOf } from "../values";
import { barLabel, barParts, LABEL_ROOM } from "./bar";

const CAPTION_BELOW = 30;

export function drawSplitBar(object: HmiObjectModel, drawFrame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const thickness = Math.min(24, Math.max(8, (width - 36) / 2));
  const gap = width - 2 * thickness - 24;
  const top = object.label ? LABEL_ROOM : 0;
  const hasUnit = ["first", "second"].some((property) => Boolean(withValueFormat(object, drawFrame, property).unit));
  const trackHeight = height - 12 - top - (hasUnit ? 48 : CAPTION_BELOW);
  const children: SvgNode[] = [...barLabel(object, width, true)];
  const outputs: Array<[string, number, string]> = [
    ["first", 12, SECOND_UP],
    ["second", 12 + thickness + gap, ACCENT_UP],
  ];
  for (const [output, x, paint] of outputs) {
    const track: Box = { x, y: 6 + top, width: thickness, height: trackHeight };
    const live = liveValueOf(object, output, drawFrame);
    const formatted = withValueFormat(object, drawFrame, output);
    children.push(...barParts(output, track, false, live, rangeReading(live, formatted), paint, formatted));
    const shown = live && live.state !== "unknown" ? formatValue(live.value, formatted) : "?";
    children.push(
      text(shown, {
        x: x + thickness / 2,
        y: height - (hasUnit ? 27 : 10),
        fill: shown === "?" ? token("unknown") : token("text"),
        "font-size": 16,
        "font-weight": 600,
        "text-anchor": "middle",
        "font-variant-numeric": "tabular-nums",
      }),
    );
    if (formatted.unit && shown !== "?")
      children.push(
        text(formatted.unit, {
          x: x + thickness / 2,
          y: height - 10,
          fill: token("muted"),
          "font-size": 14,
          "text-anchor": "middle",
        }),
      );
  }
  const request = rangeReading(liveValueOf(object, "request", drawFrame), object);
  if (request) {
    const y = 6 + top + trackHeight - trackHeight * request.fraction;
    children.push(
      el("line", {
        "data-part": "request",
        "data-fraction": request.fraction,
        x1: 4,
        y1: y,
        x2: width - 4,
        y2: y,
        stroke: token("text"),
        "stroke-width": 3,
        "stroke-linecap": "round",
      }),
    );
  }
  return el("g", { class: "hmi-object hmi-split-bar" }, children);
}
