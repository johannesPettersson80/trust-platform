// The one look of the equipment symbols (specification 39, §14.2, item 5): a 4 px grid, a 2 px line
// with round joins and caps that stays 2 px at any size, and calm tones from the tokens, so both
// themes and a project's colours apply.

import { ACCENT_UP, token, UNKNOWN_HATCH } from "../parts";

/** The line every part is drawn with. */
export const LINE = {
  stroke: token("muted"),
  "stroke-width": 2,
  "stroke-linejoin": "round",
  "stroke-linecap": "round",
  "vector-effect": "non-scaling-stroke",
} as const;

/** The body's tone, one step above the page. */
export const BODY = token("raised");
/** A filled valve body, as a closed valve is drawn on a P&ID. */
export const FILLED = token("muted");
/** A moving part, in the brightest neutral. */
export const MOVING = token("text");
/** A moving part that stands still. */
export const STILL = token("muted");
/** A vessel's liquid: a measured value, in the accent of live values, as a bar's fill. */
export const LIQUID = ACCENT_UP;
/** The hatch of a body whose state is unknown (§5.3, item 1). */
export const HATCH = `url(#${UNKNOWN_HATCH})`;

/** A value to two decimals, for transforms and data attributes. */
export function round(value: number): number {
  return Math.round(value * 100) / 100;
}
