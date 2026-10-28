// Trend: pens over the last `span` seconds, limit bands, and a legend with each pen's current
// value (§5.7); range chips for 5 minutes to 24 hours, and a window paused where its operator left
// it, with Back to live (§5.9 item 1).

import { equipmentBadgeWidth } from "../alarms";
import { withValueFormat } from "../valueFormats";
import { fitWidth, formatAge, formatValue, textWidth, timeOfDay } from "../format";
import type { DrawFrame, HmiObjectModel, LiveValue, TrendView } from "../model";
import { Box, card, token, TokenName } from "../parts";
import { el, run, SvgNode, text, tspan } from "../svg";
import { liveValueOf } from "../values";
import { say } from "../speech";

const LIMITS = ["high-limit", "low-limit"];
/** A trend's bindings that are no pens: its limit bands and its visibility (§18 phase 5 item 2). */
export const NOT_PENS = [...LIMITS, "visible"];
const PEN_COLOURS: TokenName[] = ["pen-1", "pen-2", "pen-3", "pen-4"];
const DEFAULT_SPAN_SECONDS = 300;
const PAD = 16;
const AXIS = 52;
const LANE = 22;
const LANE_GAP = 6;
/** Space between the plot and the first lane, so a lane label clears the bottom axis label. */
const LANES_TOP = 12;
const LEGEND_ROW = 24;
/** The tool row's height and its chips' (§5.9 item 1). */
const TOOLS = 30;
const CHIP = 26;
/** The range chips' spans, in seconds. */
const RANGES = [300, 3600, 28800, 86400];
const TIME_ROW = 22;
/** A generous width of one 14 px character, used only to decide where the legend wraps. */
const CHAR = 9;

type Sample = [number, number];

interface Pen {
  name: string;
  colour: TokenName;
  dashed: boolean;
  isBoolean: boolean;
  unit?: string;
  decimals?: number;
  live: LiveValue;
  samples: Sample[];
}

interface TimeWindow {
  start: number;
  end: number;
}

interface Axis {
  low: number;
  high: number;
  ticks: number[];
}

const round = (value: number): number => Math.round(value * 10) / 10;
const clamp = (value: number): number => Math.min(1, Math.max(0, value));

function pensOf(object: HmiObjectModel, frame: DrawFrame): Pen[] {
  return Object.keys(object.bind)
    .filter((name) => !NOT_PENS.includes(name))
    .sort()
    .map((name, index) => {
      const binding = object.bind[name];
      const formatted = withValueFormat(object, frame, name);
      const live = liveValueOf(object, name, frame) ?? { value: null, state: "unknown" };
      const samples = (frame.history?.[binding.path] ?? [])
        .filter(([time, value]) => Number.isFinite(time) && Number.isFinite(value))
        .sort((first, second) => first[0] - second[0]);
      return {
        name,
        colour: PEN_COLOURS[index % PEN_COLOURS.length],
        dashed: index >= PEN_COLOURS.length,
        isBoolean: typeof live.value === "boolean",
        unit: binding.unit ?? formatted.unit,
        decimals: formatted.decimals,
        live,
        samples,
      };
    });
}

/** The samples inside the window, starting with the value at the window's start when an
 * older sample gives it: interpolated for numbers, held for booleans. */
function visible(samples: Sample[], window: TimeWindow, held: boolean): Sample[] {
  const inside = samples.filter(([time]) => time >= window.start && time <= window.end);
  const before = samples.filter(([time]) => time < window.start).pop();
  if (!before || inside.length === 0 || inside[0][0] === window.start) return inside;
  const [beforeTime, beforeValue] = before;
  const [firstTime, firstValue] = inside[0];
  const atStart = held
    ? beforeValue
    : beforeValue + ((firstValue - beforeValue) * (window.start - beforeTime)) / (firstTime - beforeTime);
  return [[window.start, atStart], ...inside];
}

function niceStep(rough: number): number {
  const power = 10 ** Math.floor(Math.log10(rough));
  const fraction = rough / power;
  const nice = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 2.5 ? 2.5 : fraction <= 5 ? 5 : 10;
  return nice * power;
}

/** The value axis: the object's range when both ends are set, otherwise the values in view with
 * a margin, rounded out to whole steps. */
function axisOf(object: Pick<HmiObjectModel, "min" | "max">, values: number[]): Axis {
  const { min, max } = object;
  if (min !== undefined && max !== undefined && Number.isFinite(min) && Number.isFinite(max) && max > min) {
    return { low: min, high: max, ticks: [min, (min + max) / 2, max] };
  }
  let low = values.length > 0 ? values.reduce((least, value) => Math.min(least, value)) : 0;
  let high = values.length > 0 ? values.reduce((most, value) => Math.max(most, value)) : 100;
  if (high === low) {
    low -= 1;
    high += 1;
  }
  const margin = (high - low) * 0.1;
  const step = niceStep((high - low + 2 * margin) / 3);
  low = Math.floor((low - margin) / step) * step;
  high = Math.ceil((high + margin) / step) * step;
  const ticks: number[] = [];
  for (let tick = low; tick <= high + step / 2; tick += step) ticks.push(Number(tick.toFixed(10)));
  return { low, high, ticks };
}

function spanLabel(seconds: number): string {
  if (seconds % 3600 === 0) return `−${say("{hours} h", { hours: seconds / 3600 })}`;
  if (seconds % 60 === 0) return `−${say("{minutes} min", { minutes: seconds / 60 })}`;
  return `−${say("{seconds} s", { seconds })}`;
}

/** A pen's value at the cursor: its last sample at or before the cursor's time. */
function atCursor(pen: Pen, cursorMs: number): LiveValue {
  const sample = pen.samples.filter(([time]) => time <= cursorMs).pop();
  if (!sample) return { value: null, state: "unknown" };
  return { value: pen.isBoolean ? sample[1] !== 0 : sample[1], state: "confirmed" };
}

/** A pen's legend entry: its name and current value in its state (§5.3), or its value at the
 * cursor (§5.9 item 1). */
function legendSpans(object: HmiObjectModel, pen: Pen, cursorMs: number | undefined): SvgNode[] {
  const spans = [tspan(pen.name, { dx: 6, fill: token("muted"), "font-weight": 500 })];
  const live = cursorMs === undefined ? pen.live : atCursor(pen, cursorMs);
  if (live.state === "unknown") {
    spans.push(tspan("?", { dx: 6, fill: token("unknown"), "font-weight": 600 }));
    return spans;
  }
  spans.push(tspan(formatValue(live.value, pen), { dx: 6, fill: token("text"), "font-weight": 600 }));
  if (pen.unit) spans.push(tspan(pen.unit, { dx: 3, fill: token("muted") }));
  if (live.staleMs !== undefined)
    spans.push(tspan(formatAge(live.staleMs), { dx: 6, fill: token("stale"), "font-weight": 600 }));
  if (live.state === "estimated") spans.push(tspan("est.", { dx: 6, fill: token("muted"), "font-style": "italic" }));
  return spans;
}

/** The legend: one text per row, so the browser lays the entries out one after the other and
 * they never overlap; a row wraps when the next entry would not fit. */
function legendOf(
  object: HmiObjectModel,
  pens: Pen[],
  width: number,
  top: number,
  view: TrendView,
): { nodes: SvgNode[]; height: number } {
  const rows: SvgNode[][] = [];
  let used = Infinity;
  for (const pen of pens) {
    const spans = legendSpans(object, pen, view.cursorMs);
    const characters = spans.reduce((sum, span) => sum + (span.text ?? "").length, 0);
    const entryWidth = 14 + characters * CHAR + spans.length * 6;
    const gap = used === Infinity ? 0 : 18;
    if (used + gap + entryWidth > width - 2 * PAD) {
      rows.push([]);
      used = 0;
    } else {
      used += gap;
    }
    // A hidden pen's entry stays, dimmed, to show it again (§5.9 item 1).
    const hidden = view.hidden?.includes(pen.name) === true;
    const dim = hidden ? { "fill-opacity": 0.4 } : {};
    const marker = tspan(pen.dashed ? "╌" : "●", { dx: used === 0 ? undefined : 18, fill: token(pen.colour), ...dim });
    rows[rows.length - 1].push(
      marker,
      el(
        "tspan",
        {
          "data-part": "legend",
          "data-pen": pen.name,
          "data-hidden": hidden ? "true" : undefined,
          cursor: "pointer",
          ...dim,
        },
        spans,
      ),
    );
    used += entryWidth;
  }
  const nodes = rows.map((entries, row) =>
    run({ x: PAD, y: top + row * LEGEND_ROW + 15, "font-size": 14, "font-variant-numeric": "tabular-nums" }, entries),
  );
  return { nodes, height: rows.length * LEGEND_ROW };
}

/** A range chip's words: "5 min", "1 h". */
function rangeWords(seconds: number): string {
  return seconds % 3600 === 0
    ? say("{hours} h", { hours: seconds / 3600 })
    : say("{minutes} min", { minutes: seconds / 60 });
}

/** The icons of Export (an arrow into a tray) and Expand (four corners), 30 px chips. */
const ICONS: Record<string, string> = {
  export: "M15 7 v10 M10 13 l5 5 l5 -5 M8 21 h14",
  expand: "M8 12 v-4 h4 M22 12 v-4 h-4 M8 15 v4 h4 M22 15 v4 h-4",
};

/** The tool row's chips, right-aligned at the top: Back to live while paused, the ranges, then
 * Export and Expand; where they do not all fit, the ranges go first, then the rest. Their left
 * edge, or the right edge when none is drawn. A label keeps at least 96 px. */
function toolsOf(
  spanSeconds: number,
  paused: boolean,
  width: number,
  top: number,
  labelled: boolean,
): { nodes: SvgNode[]; left: number } {
  type Chip = { part: string; attrs: Record<string, string>; label: string; selected: boolean; icon?: string };
  const live: Chip[] = paused
    ? [{ part: "trend-live", attrs: { "data-trend-action": "live" }, label: say("Back to live"), selected: true }]
    : [];
  const ranges: Chip[] = RANGES.map((seconds) => ({
    part: "trend-range",
    attrs: { "data-trend-range": String(seconds) },
    label: rangeWords(seconds),
    selected: seconds === spanSeconds,
  }));
  const tools: Chip[] = [
    {
      part: "trend-tool",
      attrs: { "data-trend-action": "export", "aria-label": say("Export") },
      label: "",
      selected: false,
      icon: ICONS.export,
    },
    {
      part: "trend-tool",
      attrs: { "data-trend-action": "expand", "aria-label": say("Expand") },
      label: "",
      selected: false,
      icon: ICONS.expand,
    },
  ];
  const room = width - 2 * PAD - (labelled ? 96 : 0);
  const widthOf = (chip: Chip): number => (chip.icon ? 30 : Math.round(textWidth(chip.label, 14, 600) + 20));
  const totalOf = (list: Chip[]): number => list.reduce((sum, chip) => sum + widthOf(chip) + 6, -6);
  const chips = [[...live, ...ranges, ...tools], [...live, ...tools], live].find((list) => totalOf(list) <= room) ?? [];
  if (chips.length === 0) return { nodes: [], left: width - PAD };
  const widths = chips.map(widthOf);
  const total = totalOf(chips);
  let x = width - PAD - total;
  const left = x;
  const nodes = chips.map((chip, index) => {
    const face = chip.icon
      ? el("path", {
          d: chip.icon,
          transform: `translate(${x} ${top - 2})`,
          fill: "none",
          stroke: token("muted"),
          "stroke-width": 1.8,
          "stroke-linecap": "round",
          "stroke-linejoin": "round",
        })
      : text(chip.label, {
          x: x + widths[index] / 2,
          y: top + 18,
          fill: chip.selected ? token("accent") : token("muted"),
          "font-size": 14,
          "font-weight": 600,
          "text-anchor": "middle",
        });
    const node = el(
      "g",
      { "data-part": chip.part, ...chip.attrs, "data-selected": chip.selected ? "true" : undefined, cursor: "pointer" },
      [
        el("rect", {
          x,
          y: top,
          width: widths[index],
          height: CHIP,
          rx: CHIP / 2,
          fill: chip.selected ? token("raised") : "transparent",
          stroke: chip.selected ? token("accent") : token("border"),
        }),
        face,
      ],
    );
    x += widths[index] + 6;
    return node;
  });
  return { nodes, left };
}

export function drawTrend(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const pens = pensOf(object, frame);
  const view = frame.trendViews?.[object.id] ?? {};
  const newest = pens.reduce(
    (latest, pen) => Math.max(latest, pen.samples[pen.samples.length - 1]?.[0] ?? -Infinity),
    -Infinity,
  );
  const paused = view.endMs !== undefined;
  const end = view.endMs ?? frame.nowMs ?? (Number.isFinite(newest) ? newest : 0);
  const spanSeconds = Math.max(
    1,
    view.spanMs !== undefined ? view.spanMs / 1000 : (object.span ?? DEFAULT_SPAN_SECONDS),
  );
  const window: TimeWindow = { start: end - spanSeconds * 1000, end };
  const lanes = pens.filter((pen) => pen.isBoolean);
  const children: SvgNode[] = [card(width, height, undefined)];

  let top = PAD;
  const badgeWidth = equipmentBadgeWidth(object, frame);
  const toolWidth = width - (badgeWidth > 0 ? badgeWidth + 8 : 0);
  const tools = toolsOf(spanSeconds, paused, toolWidth, top, Boolean(object.label));
  children.push(...tools.nodes);
  if (object.label) {
    children.push(
      text(fitWidth(object.label, tools.left - 2 * PAD, 15, 600), {
        x: PAD,
        y: top + 17,
        fill: token("text"),
        "font-size": 15,
        "font-weight": 600,
      }),
    );
  }
  if (object.label || tools.nodes.length > 0) top += TOOLS;
  const legend = legendOf(object, pens, width, top, view);
  const hidden = new Set(view.hidden ?? []);
  children.push(...legend.nodes);
  top += legend.height + 8;

  const lanesHeight = lanes.length > 0 ? LANES_TOP + lanes.length * (LANE + LANE_GAP) - LANE_GAP : 0;
  const plot: Box = {
    x: PAD + AXIS,
    y: top,
    width: Math.max(1, width - 2 * PAD - AXIS),
    height: Math.max(1, height - PAD - TIME_ROW - lanesHeight - top),
  };
  const shown = new Map(pens.map((pen) => [pen.name, visible(pen.samples, window, pen.isBoolean)]));
  const limits = LIMITS.map((name) => [name, liveValueOf(object, name, frame)] as const).filter(
    (entry): entry is readonly [string, LiveValue & { value: number }] =>
      entry[1] !== undefined &&
      entry[1].state !== "unknown" &&
      typeof entry[1].value === "number" &&
      Number.isFinite(entry[1].value),
  );
  const numeric = pens.filter((pen) => !pen.isBoolean && !hidden.has(pen.name));
  // Automatic pens of one unit share their scale, so a measurement and its setpoint are read
  // against each other; a pen of another or no unit, or with a fixed range, keeps its own.
  const sharing = (pen: Pen): Pen[] =>
    pen.unit === undefined || object.bind[pen.name].range
      ? [pen]
      : numeric.filter((other) => other.unit === pen.unit && !object.bind[other.name].range);
  const axes = new Map(
    numeric.map((pen) => {
      const range = object.bind[pen.name].range;
      const bounds = range ? { min: range[0], max: range[1] } : object;
      const scaled = sharing(pen);
      return [
        pen.name,
        axisOf(bounds, [
          ...scaled.flatMap((member) => (shown.get(member.name) ?? []).map(([, value]) => value)),
          ...(scaled.includes(numeric[0]) ? limits.map(([, live]) => live.value) : []),
        ]),
      ] as const;
    }),
  );
  const axis = axes.get(numeric[0]?.name) ?? axisOf(object, []);
  const xOf = (time: number): number =>
    round(plot.x + clamp((time - window.start) / (window.end - window.start)) * plot.width);
  const yOf = (value: number, scale: Axis = axis): number =>
    round(plot.y + plot.height - clamp((value - scale.low) / (scale.high - scale.low)) * plot.height);

  children.push(
    el("rect", { "data-part": "plot", x: plot.x, y: plot.y, width: plot.width, height: plot.height, fill: "none" }),
  );
  // Keep the scale intact while thinning labels in short faceplate plots (§5.7).
  const tickCount = Math.min(axis.ticks.length, Math.floor(plot.height / 20) + 1);
  const ticks =
    tickCount <= 1
      ? [axis.ticks[0]]
      : Array.from(
          { length: tickCount },
          (_, index) => axis.ticks[Math.round((index * (axis.ticks.length - 1)) / (tickCount - 1))],
        );
  for (const tick of ticks) {
    const y = yOf(tick);
    children.push(
      el("line", { x1: plot.x, y1: y, x2: plot.x + plot.width, y2: y, stroke: token("border"), "stroke-width": 1 }),
    );
    children.push(
      text(formatValue(tick, {}), {
        x: plot.x - 10,
        y: y + 5,
        fill: token(numeric[0]?.colour ?? "muted"),
        "data-axis-pen": numeric[0]?.name,
        "font-size": 14,
        "text-anchor": "end",
        "font-variant-numeric": "tabular-nums",
      }),
    );
  }

  for (const [name, live] of limits) {
    if (live.value < axis.low || live.value > axis.high) continue;
    const y = yOf(live.value);
    const band =
      name === "high-limit" ? { y: plot.y, height: round(y - plot.y) } : { y, height: round(plot.y + plot.height - y) };
    children.push(
      el("rect", {
        "data-part": "limit-band",
        x: plot.x,
        width: plot.width,
        ...band,
        fill: token("warning"),
        "fill-opacity": 0.06,
      }),
    );
    children.push(
      el("line", {
        "data-part": "limit",
        "data-limit": name,
        x1: plot.x,
        y1: y,
        x2: plot.x + plot.width,
        y2: y,
        stroke: token("warning"),
        "stroke-width": 1.5,
        "stroke-dasharray": "5 4",
      }),
    );
  }

  const stroke = (pen: Pen): Record<string, string | number | undefined> => ({
    "data-pen": pen.name,
    fill: "none",
    stroke: token(pen.colour),
    "stroke-width": 2,
    "stroke-linejoin": "round",
    "stroke-linecap": "round",
    "stroke-dasharray": pen.dashed ? "6 4" : undefined,
  });
  for (const pen of numeric) {
    const samples = shown.get(pen.name) ?? [];
    if (samples.length === 0) continue;
    const points = samples.map(([time, value]) => `${xOf(time)},${yOf(value, axes.get(pen.name))}`).join(" ");
    children.push(el("polyline", { ...stroke(pen), points }));
  }

  // Boolean pens: step lines in their own lanes under the plot, high while TRUE.
  lanes.forEach((pen, index) => {
    const laneTop = plot.y + plot.height + LANES_TOP + index * (LANE + LANE_GAP);
    const [high, low] = [laneTop + 5, laneTop + LANE - 5];
    children.push(
      el("rect", {
        "data-part": "lane",
        x: plot.x,
        y: laneTop,
        width: plot.width,
        height: LANE,
        rx: 6,
        fill: token("raised"),
      }),
    );
    children.push(
      text(pen.name, {
        x: plot.x - 10,
        y: laneTop + LANE / 2 + 5,
        fill: token("muted"),
        "font-size": 14,
        "text-anchor": "end",
      }),
    );
    const points: Array<[number, number]> = [];
    let previous: number | undefined;
    for (const [time, value] of hidden.has(pen.name) ? [] : (shown.get(pen.name) ?? [])) {
      const x = xOf(time);
      const y = value ? high : low;
      if (previous !== undefined && previous !== y) points.push([x, previous]);
      points.push([x, y]);
      previous = y;
    }
    if (points.length > 0) {
      children.push(el("polyline", { ...stroke(pen), points: points.map(([x, y]) => `${x},${y}`).join(" ") }));
    }
  });

  // The cursor line and its time (§5.9 item 1).
  if (view.cursorMs !== undefined && view.cursorMs >= window.start && view.cursorMs <= window.end) {
    const x = xOf(view.cursorMs);
    children.push(
      el("line", {
        "data-part": "trend-cursor",
        x1: x,
        y1: plot.y,
        x2: x,
        y2: plot.y + plot.height,
        stroke: token("text"),
        "stroke-opacity": 0.7,
        "stroke-width": 1,
        "stroke-dasharray": "3 3",
      }),
    );
    const right = x > plot.x + plot.width - 80;
    children.push(
      text(timeOfDay(view.cursorMs), {
        x: right ? x - 6 : x + 6,
        y: plot.y + 16,
        fill: token("text"),
        "font-size": 14,
        "font-weight": 600,
        "text-anchor": right ? "end" : undefined,
        "font-variant-numeric": "tabular-nums",
      }),
    );
  }
  // The box being dragged to zoom into its period, and the row under the plot that pans (§5.9).
  if (view.box) {
    const [from, to] = [Math.min(view.box.fromMs, view.box.toMs), Math.max(view.box.fromMs, view.box.toMs)];
    children.push(
      el("rect", {
        "data-part": "trend-box",
        x: xOf(from),
        y: plot.y,
        width: Math.max(1, round(xOf(to) - xOf(from))),
        height: plot.height,
        fill: token("accent"),
        "fill-opacity": 0.14,
        stroke: token("accent"),
      }),
    );
  }
  children.push(
    el("rect", {
      "data-part": "trend-time",
      x: plot.x,
      y: plot.y + plot.height + lanesHeight,
      width: plot.width,
      height: TIME_ROW + 4,
      fill: "transparent",
      cursor: "ew-resize",
    }),
  );
  const timeBaseline = plot.y + plot.height + lanesHeight + 18;
  const label = { y: timeBaseline, fill: token("muted"), "font-size": 14 };
  // A paused window names its start and end; a live one counts back from now.
  children.push(text(paused ? timeOfDay(window.start) : spanLabel(spanSeconds), { ...label, x: plot.x }));
  children.push(
    text(paused ? timeOfDay(window.end) : say("now"), { ...label, x: plot.x + plot.width, "text-anchor": "end" }),
  );
  return el(
    "g",
    {
      class: "hmi-object hmi-trend",
      "data-window-start": window.start,
      "data-window-end": window.end,
      "data-paused": paused ? "true" : undefined,
    },
    children,
  );
}
