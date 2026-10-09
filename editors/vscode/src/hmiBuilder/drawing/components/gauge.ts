// The gauge: a 270° arc swept from the range minimum to the value (§5.2).

import { alarmRange, limitTick } from "../alarmRanges";
import { formatValue } from "../format";
import type { DrawFrame, HmiObjectModel } from "../model";
import { ACCENT_ACROSS, commandedMark, estimatedMark, GLOW, staleMark, stateAttributes, token } from "../parts";
import { rangeReading } from "../ranges";
import { el, SvgNode, text } from "../svg";
import { liveValueOf } from "../values";

const START = 135;
const SWEEP = 270;

/** A point on a circle; angles in degrees, clockwise from three o'clock as SVG draws them. */
function point(cx: number, cy: number, radius: number, degrees: number): [number, number] {
  const radians = (degrees * Math.PI) / 180;
  return [Number((cx + radius * Math.cos(radians)).toFixed(2)), Number((cy + radius * Math.sin(radians)).toFixed(2))];
}

function arc(cx: number, cy: number, radius: number, from: number, sweep: number): string {
  const [x0, y0] = point(cx, cy, radius, from);
  const [x1, y1] = point(cx, cy, radius, from + sweep);
  return `M ${x0} ${y0} A ${radius} ${radius} 0 ${sweep > 180 ? 1 : 0} 1 ${x1} ${y1}`;
}

export function drawGauge(object: HmiObjectModel, drawFrame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const live = liveValueOf(object, "value", drawFrame);
  const reading = rangeReading(live, object);
  const cx = width / 2;
  const cy = height / 2;
  const stroke = Math.max(8, Math.round(Math.min(width, height) * 0.075));
  const radius = Math.min(width, height) / 2 - stroke - 4;
  const stale = live?.staleMs !== undefined;
  const children: SvgNode[] = [
    el("path", {
      "data-part": "track",
      d: arc(cx, cy, radius, START, SWEEP),
      fill: "none",
      stroke: token("line"),
      "stroke-width": stroke,
      "stroke-linecap": "round",
    }),
  ];
  if (reading) {
    children.push(
      el("path", {
        "data-part": "fill",
        "data-fraction": reading.fraction,
        d: arc(cx, cy, radius, START, SWEEP * reading.fraction),
        fill: "none",
        stroke: reading.range ? token("warning") : ACCENT_ACROSS,
        "stroke-width": stroke,
        "stroke-linecap": "round",
        filter: stale ? undefined : GLOW,
        // A stale value fades; its age is shown in the corner.
        opacity: stale ? 0.45 : undefined,
      }),
    );
  }
  const limits = alarmRange(object, live);
  if (limits) {
    children.push(
      el("path", {
        "data-part": "normal-range",
        "data-from": limits.from,
        "data-to": limits.to,
        d: arc(
          cx,
          cy,
          Math.max(1, radius - stroke - 5),
          START + SWEEP * limits.from,
          SWEEP * (limits.to - limits.from),
        ),
        fill: "none",
        stroke: token("muted"),
        opacity: 0.3,
        "stroke-width": 3,
        "stroke-linecap": "round",
      }),
    );
    for (const tick of limits.ticks) {
      const angle = START + SWEEP * tick.fraction;
      const [x1, y1] = point(cx, cy, radius - stroke / 2 - 3, angle);
      const [x2, y2] = point(cx, cy, radius + stroke / 2 + 3, angle);
      children.push(limitTick(tick, { x1, y1, x2, y2 }, object));
    }
  }
  const target =
    live?.state === "commanded" && typeof live.target === "number"
      ? rangeReading({ value: live.target, state: "confirmed" }, object)
      : undefined;
  if (target) {
    const angle = START + SWEEP * target.fraction;
    const [x0, y0] = point(cx, cy, radius - stroke, angle);
    const [x1, y1] = point(cx, cy, radius + stroke, angle);
    children.push(
      el("line", {
        "data-part": "target",
        "data-fraction": target.fraction,
        x1: x0,
        y1: y0,
        x2: x1,
        y2: y1,
        stroke: token("pending"),
        "stroke-width": 3,
        "stroke-linecap": "round",
      }),
    );
  }
  const size = Math.max(14, Math.round(radius * 0.42));
  const unknown = !live || live.state === "unknown";
  children.push(
    text(unknown ? "?" : formatValue(live.value, object), {
      x: cx,
      y: cy + size * 0.35,
      fill: unknown ? token("unknown") : token("text"),
      "font-size": size,
      "font-weight": 600,
      "letter-spacing": -0.5,
      "text-anchor": "middle",
      "font-variant-numeric": "tabular-nums",
    }),
  );
  const unit = object.unit ?? object.bind.value?.unit;
  let below = cy + size * 0.35 + 18;
  if (unit && !unknown) {
    children.push(
      text(unit, {
        x: cx,
        y: below,
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 500,
        "text-anchor": "middle",
      }),
    );
    below += 18;
  }
  // State marks sit under the value, clear of the range labels at the arc's ends.
  if (live?.state === "commanded")
    children.push(commandedMark(object, live, { x: cx, y: below, "text-anchor": "middle" }));
  if (live?.state === "estimated") children.push(estimatedMark({ x: cx, y: below, "text-anchor": "middle" }));
  if (live && stale) children.push(staleMark(live, { x: width - 6, y: 16, "text-anchor": "end" }));
  const [minX, minY] = point(cx, cy, radius, START);
  const [maxX, maxY] = point(cx, cy, radius, START + SWEEP);
  // The ends of the range are plain numbers, like a trend's axis; the value keeps its decimals.
  children.push(
    text(formatValue(object.min ?? 0, {}), {
      x: minX,
      y: minY + stroke + 12,
      fill: token("muted"),
      "font-size": 14,
      "text-anchor": "middle",
    }),
  );
  children.push(
    text(formatValue(object.max ?? 100, {}), {
      x: maxX,
      y: maxY + stroke + 12,
      fill: token("muted"),
      "font-size": 14,
      "text-anchor": "middle",
    }),
  );
  return el("g", { class: "hmi-object hmi-gauge", "data-range": reading?.range, ...stateAttributes(live) }, children);
}
