// Shared drawing parts: tokens, gradients and glow, the card, the unknown hatch and the
// value-state marks (§5.3, §14.1).

import { formatAge, formatValue, textWidth } from "./format";
import type { HmiObjectModel, LiveValue } from "./model";
import { el, SvgNode, text } from "./svg";
import { say } from "./speech";

export const UNKNOWN_HATCH = "hmi-unknown-hatch";
export const GLOW = "url(#hmi-glow)";
export const ACCENT_ACROSS = "url(#hmi-accent-x)";
export const ACCENT_UP = "url(#hmi-accent-y)";
export const SECOND_UP = "url(#hmi-second-y)";

export type TokenName =
  | "page"
  | "surface"
  | "raised"
  | "border"
  | "line"
  | "text"
  | "muted"
  | "accent"
  | "accent-2"
  | "accent-3"
  | "pen-1"
  | "pen-2"
  | "pen-3"
  | "pen-4"
  | "unknown"
  | "stale"
  | "pending"
  | "warning"
  | "alarm"
  | "alarm-high"
  | "alarm-medium"
  | "alarm-low";

/** A design token as a paint value. */
export function token(name: TokenName): string {
  return `var(--hmi-${name})`;
}

function gradient(id: string, from: [number, number], to: [number, number], stops: TokenName[]): SvgNode {
  return el(
    "linearGradient",
    { id, x1: from[0], y1: from[1], x2: to[0], y2: to[1] },
    stops.map((name, index) => el("stop", { offset: index / (stops.length - 1), "stop-color": token(name) })),
  );
}

/** Definitions every drawing shares: the hatch, the accent gradients, the glow, the backdrop. */
export function definitions(): SvgNode {
  return el("defs", {}, [
    el(
      "pattern",
      { id: UNKNOWN_HATCH, patternUnits: "userSpaceOnUse", width: 8, height: 8, patternTransform: "rotate(45)" },
      [el("line", { x1: 0, y1: 0, x2: 0, y2: 8, stroke: token("unknown"), "stroke-width": 2, "stroke-opacity": 0.55 })],
    ),
    gradient("hmi-accent-x", [0, 0], [1, 0], ["accent", "accent-2"]),
    gradient("hmi-accent-y", [0, 1], [0, 0], ["accent-2", "accent"]),
    gradient("hmi-second-y", [0, 1], [0, 0], ["accent-2", "accent-3"]),
    el("radialGradient", { id: "hmi-backdrop", cx: 0.12, cy: 0, r: 0.85 }, [
      el("stop", { offset: 0, "stop-color": token("accent-2"), "stop-opacity": 0.16 }),
      el("stop", { offset: 1, "stop-color": token("accent-2"), "stop-opacity": 0 }),
    ]),
    el("filter", { id: "hmi-glow", x: "-50%", y: "-50%", width: "200%", height: "200%" }, [
      el("feGaussianBlur", { in: "SourceGraphic", stdDeviation: 3.5, result: "blur" }),
      el("feMerge", {}, [el("feMergeNode", { in: "blur" }), el("feMergeNode", { in: "SourceGraphic" })]),
    ]),
  ]);
}

/** The page behind the objects: the page tone with a faint accent glow from the top left. */
export function backdrop(width: number, height: number): SvgNode[] {
  return [
    el("rect", { "data-part": "page", x: 0, y: 0, width, height, fill: token("page") }),
    el("rect", { x: 0, y: 0, width, height, fill: "url(#hmi-backdrop)" }),
  ];
}

/** The text size for an object of a given height, never below 14 px (§3.5). */
export function fontSize(height: number, share = 0.42, maximum = 44): number {
  return Math.min(maximum, Math.max(14, Math.round(height * share)));
}

/** A rectangle in an object's own coordinates. */
export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The card an object sits on: the surface tone, rounded, with a faint border; amber when stale. */
export function card(width: number, height: number, live: LiveValue | undefined, radius = 14): SvgNode {
  const stale = live?.staleMs !== undefined;
  return el("rect", {
    "data-part": "card",
    x: 0.5,
    y: 0.5,
    width: Math.max(0, width - 1),
    height: Math.max(0, height - 1),
    rx: radius,
    fill: token("surface"),
    stroke: stale ? token("stale") : token("border"),
  });
}

/** The neutral hatch over an area whose value is unknown. */
export function hatchBox(box: Box, radius = 3): SvgNode {
  return el("rect", { ...box, rx: radius, fill: `url(#${UNKNOWN_HATCH})` });
}

/** The neutral hatch over a whole card whose value is unknown. */
export function hatch(width: number, height: number, radius = 13): SvgNode {
  return hatchBox({ x: 1.5, y: 1.5, width: Math.max(0, width - 3), height: Math.max(0, height - 3) }, radius);
}

type At = { x: number; y: number; "text-anchor"?: string };

/** "est." for an estimated value. */
export function estimatedMark(at: At): SvgNode {
  return text(say("est."), { ...at, fill: token("muted"), "font-size": 14, "font-style": "italic" });
}

/** The commanded target, pending. */
export function commandedMark(object: HmiObjectModel, live: LiveValue, at: At): SvgNode {
  return text(`→ ${formatValue(live.target, object)}`, {
    ...at,
    fill: token("pending"),
    "font-size": 14,
    "font-weight": 600,
  });
}

/** The age of a stale value. */
export function staleMark(live: LiveValue, at: At): SvgNode {
  return text(formatAge(live.staleMs ?? 0), { ...at, fill: token("stale"), "font-size": 14, "font-weight": 600 });
}

/** A tinted pill with a short text, laid out from its right edge. */
export function chip(
  label: string,
  colour: TokenName,
  right: number,
  top: number,
  italic: boolean,
): { node: SvgNode; width: number } {
  const width = Math.round(textWidth(label, 14, 600) + 16);
  return {
    width,
    node: el("g", { "data-part": "chip" }, [
      el("rect", { x: right - width, y: top, width, height: 22, rx: 11, fill: token(colour), "fill-opacity": 0.14 }),
      text(label, {
        x: right - width / 2,
        y: top + 15.5,
        fill: token(colour),
        "font-size": 14,
        "font-weight": 600,
        "font-style": italic ? "italic" : undefined,
        "text-anchor": "middle",
      }),
    ]),
  };
}

/** The state of a value as chips from an object's right edge: stale age, target, "est."; at the
 * top of a card, or centred in a pill. */
export function stateChips(object: HmiObjectModel, live: LiveValue | undefined, width: number, top = 12): SvgNode[] {
  return stateChipRow(object, live, width, top).nodes;
}

/** The state chips at an object's top right, and where their row begins, so a title beside them
 * can end before it (§14.5 item 6). */
export function stateChipRow(
  object: HmiObjectModel,
  live: LiveValue | undefined,
  width: number,
  top = 12,
): { nodes: SvgNode[]; left: number } {
  if (!live) return { nodes: [], left: width };
  const chips: Array<[string, TokenName, boolean]> = [];
  if (live.staleMs !== undefined) chips.push([formatAge(live.staleMs), "stale", false]);
  if (live.state === "commanded") chips.push([`→ ${formatValue(live.target, object)}`, "pending", false]);
  if (live.state === "estimated") chips.push([say("est."), "muted", true]);
  const nodes: SvgNode[] = [];
  let right = width - 12;
  for (const [label, colour, italic] of chips) {
    const placed = chip(label, colour, right, top, italic);
    nodes.push(placed.node);
    right -= placed.width + 6;
  }
  return { nodes, left: nodes.length > 0 ? right + 6 : width };
}

/** The marks a value carries for its state, at an object's corners: "est." bottom left, the
 * commanded target bottom right, a stale age top right. */
export function stateMarks(
  object: HmiObjectModel,
  live: LiveValue | undefined,
  width: number,
  height: number,
  staleY = 16,
): SvgNode[] {
  if (!live) return [];
  const marks: SvgNode[] = [];
  if (live.state === "estimated") marks.push(estimatedMark({ x: 6, y: height - 6 }));
  if (live.state === "commanded")
    marks.push(commandedMark(object, live, { x: width - 6, y: height - 6, "text-anchor": "end" }));
  if (live.staleMs !== undefined) marks.push(staleMark(live, { x: width - 6, y: staleY, "text-anchor": "end" }));
  return marks;
}

/** The attributes that tell the state of an object's value to styles and tests. */
export function stateAttributes(live: LiveValue | undefined): Record<string, string | undefined> {
  return {
    "data-state": live?.state,
    "data-stale": live?.staleMs !== undefined ? "true" : undefined,
  };
}
