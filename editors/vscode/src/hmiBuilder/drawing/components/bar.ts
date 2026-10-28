// The bar: a slim capsule filled with the accent gradient from the range minimum to the value,
// vertical or horizontal (§5.2, §14.1).

import { alarmRange, limitTick } from "../alarmRanges";
import { fitWidth, formatValue } from "../format";
import type { DrawFrame, HmiObjectModel, LiveValue } from "../model";
import { ACCENT_ACROSS, ACCENT_UP, Box, hatchBox, stateAttributes, stateMarks, token } from "../parts";
import { rangeReading, RangeReading } from "../ranges";
import { el, run, SvgNode, text, tspan } from "../svg";
import { liveValueOf } from "../values";

const CAPTION_BELOW = 44;
const CAPTION_BESIDE = 84;
/** The room a label takes above a bar (§5.3, item 7). */
export const LABEL_ROOM = 22;

/** A bar's label above its bars, in the label style of the other displays (§5.3, item 7). */
export function barLabel(object: HmiObjectModel, width: number, centred: boolean): SvgNode[] {
  if (!object.label) return [];
  const words = fitWidth(object.label, width - 8, 14, 500);
  return [
    text(words, {
      x: centred ? width / 2 : 4,
      y: 15,
      "text-anchor": centred ? "middle" : "start",
      fill: token("muted"),
      "font-size": 14,
      "font-weight": 500,
      "data-part": "label",
    }),
  ];
}

export function drawBar(object: HmiObjectModel, drawFrame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const horizontal = object.variant === "horizontal";
  const live = liveValueOf(object, "value", drawFrame);
  const reading = rangeReading(live, object);
  const ageRow = object.label && live?.staleMs !== undefined ? LABEL_ROOM : 0;
  const top = (object.label ? LABEL_ROOM : 0) + ageRow;
  let track: Box;
  if (horizontal) {
    const thickness = Math.min(18, Math.max(8, height - 24 - top));
    track = { x: 4, y: top + (height - top - thickness) / 2, width: width - 8 - CAPTION_BESIDE, height: thickness };
  } else {
    const thickness = Math.min(26, Math.max(8, width - 28));
    track = { x: (width - thickness) / 2, y: 6 + top, width: thickness, height: height - 6 - top - CAPTION_BELOW };
  }
  const children = barParts("", track, horizontal, live, reading, horizontal ? ACCENT_ACROSS : ACCENT_UP, object);
  children.push(...barLabel(object, width, !horizontal));
  children.push(
    ...(horizontal
      ? inlineCaption(object, live, { x: track.x + track.width + 14, y: track.y + track.height / 2 + 6 })
      : valueCaption(object, live, { x: width / 2, y: height - 22, "text-anchor": "middle" })),
  );
  children.push(...stateMarks(object, live, width, height, 16 + ageRow));
  return el("g", { class: "hmi-object hmi-bar", "data-range": reading?.range, ...stateAttributes(live) }, children);
}

/** The value as a caption under a bar: the number and, below it, the unit; "?" when unknown. */
export function valueCaption(
  object: HmiObjectModel,
  live: LiveValue | undefined,
  at: { x: number; y: number; "text-anchor": string },
): SvgNode[] {
  if (!live || live.state === "unknown") {
    return [text("?", { ...at, fill: token("unknown"), "font-size": 16, "font-weight": 600 })];
  }
  const unit = object.unit ?? object.bind.value?.unit;
  const number = text(formatValue(live.value, object), {
    ...at,
    fill: token("text"),
    "font-size": 16,
    "font-weight": 600,
    "font-variant-numeric": "tabular-nums",
  });
  return unit
    ? [number, text(unit, { ...at, y: at.y + 17, fill: token("muted"), "font-size": 14, "font-weight": 500 })]
    : [number];
}

/** The value and its unit in one run, for a caption beside a horizontal track. */
function inlineCaption(object: HmiObjectModel, live: LiveValue | undefined, at: { x: number; y: number }): SvgNode[] {
  if (!live || live.state === "unknown") {
    return [text("?", { ...at, fill: token("unknown"), "font-size": 16, "font-weight": 600 })];
  }
  const unit = object.unit ?? object.bind.value?.unit;
  return [
    run({ ...at, fill: token("text"), "font-size": 16, "font-weight": 600, "font-variant-numeric": "tabular-nums" }, [
      tspan(formatValue(live.value, object)),
      ...(unit ? [tspan(unit, { dx: 4, fill: token("muted"), "font-size": 14, "font-weight": 500 })] : []),
    ]),
  ];
}

/**
 * The track, fill and range mark of one bar. `prefix` names the parts when an object has more
 * than one bar, as the split bar does; `paint` is the fill's gradient.
 */
export function barParts(
  prefix: string,
  track: Box,
  horizontal: boolean,
  live: LiveValue | undefined,
  reading: RangeReading | undefined,
  paint: string,
  object?: HmiObjectModel,
): SvgNode[] {
  const name = (part: string): string => (prefix ? `${prefix}-${part}` : part);
  const radius = Math.min(track.width, track.height) / 2;
  const stale = live?.staleMs !== undefined;
  const parts: SvgNode[] = [
    el("rect", {
      "data-part": name("track"),
      ...track,
      rx: radius,
      fill: token("raised"),
      stroke: stale ? token("stale") : undefined,
      "stroke-dasharray": stale ? "4 3" : undefined,
    }),
  ];
  if (!reading && live?.state === "unknown") parts.push(hatchBox(track, radius));
  if (reading) {
    const fill: Box = horizontal
      ? { x: track.x, y: track.y, width: track.width * reading.fraction, height: track.height }
      : {
          x: track.x,
          y: track.y + track.height - track.height * reading.fraction,
          width: track.width,
          height: track.height * reading.fraction,
        };
    parts.push(
      el("rect", {
        "data-part": name("fill"),
        "data-fraction": reading.fraction,
        ...fill,
        rx: radius,
        fill: paint,
        opacity: stale ? 0.45 : undefined,
      }),
    );
    if (reading.range) parts.push(rangeMark(name("range-mark"), track, horizontal, reading.range));
  }
  const range = object && alarmRange(object, live);
  if (range) {
    const band = horizontal
      ? {
          x: track.x + track.width * range.from,
          y: track.y + track.height + 5,
          width: track.width * (range.to - range.from),
          height: 3,
        }
      : {
          x: track.x + track.width + 5,
          y: track.y + track.height * (1 - range.to),
          width: 3,
          height: track.height * (range.to - range.from),
        };
    parts.push(
      el("rect", {
        ...band,
        "data-part": "normal-range",
        "data-from": range.from,
        "data-to": range.to,
        fill: token("muted"),
        opacity: 0.3,
        rx: 1.5,
      }),
    );
    for (const tick of range.ticks) {
      const x = track.x + track.width * tick.fraction;
      const y = track.y + track.height * (1 - tick.fraction);
      parts.push(
        limitTick(
          tick,
          horizontal
            ? { x1: x, x2: x, y1: track.y - 3, y2: track.y + track.height + 3 }
            : { x1: track.x - 3, x2: track.x + track.width + 3, y1: y, y2: y },
          object,
        ),
      );
    }
  }
  return parts;
}

/** A small warning triangle at the end of the track the value has passed. */
function rangeMark(part: string, track: Box, horizontal: boolean, range: "over" | "under"): SvgNode {
  const size = 7;
  let points: string;
  if (horizontal) {
    const x = range === "over" ? track.x + track.width + 3 : track.x - 3;
    const direction = range === "over" ? 1 : -1;
    points = `${x},${track.y} ${x + direction * size},${track.y + track.height / 2} ${x},${track.y + track.height}`;
  } else {
    const y = range === "over" ? track.y - 3 : track.y + track.height + 3;
    const direction = range === "over" ? -1 : 1;
    const centre = track.x + track.width / 2;
    points = `${centre - size},${y} ${centre},${y + direction * size} ${centre + size},${y}`;
  }
  return el("polygon", { "data-part": part, points, fill: token("warning") });
}
