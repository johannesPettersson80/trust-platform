// How wide text is drawn (specification 39, §14.2, §14.5 item 6): the widths of the font the HMI
// ships.

import { FONT_CHARS, FONT_UNITS, FONT_WIDTHS } from "./fontMetrics";

/** Each weight's letter widths in font units, from the table of the font the HMI ships (§14.2). */
const WIDTHS: Array<[number, Map<string, number>]> = Object.entries(FONT_WIDTHS).map(([weight, packed]) => {
  const widths = new Map<string, number>();
  Array.from(FONT_CHARS).forEach((letter, index) =>
    widths.set(letter, parseInt(packed.slice(index * 3, index * 3 + 3), 36)),
  );
  return [Number(weight), widths];
});

/**
 * How wide a text is drawn at a size and weight: the sum of its letters' widths in the shipped
 * font, so a text of any words and any length is laid out where it really ends. A letter outside
 * the table counts a full em, so it is measured wide rather than narrow.
 */
export function textWidth(content: string, size: number, weight = 400): number {
  const widths = WIDTHS.reduce((nearest, entry) =>
    Math.abs(entry[0] - weight) < Math.abs(nearest[0] - weight) ? entry : nearest,
  )[1];
  let units = 0;
  for (const letter of content) units += widths.get(letter) ?? FONT_UNITS;
  return (units / FONT_UNITS) * size;
}
