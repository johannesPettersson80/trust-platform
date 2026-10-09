// The pager the alarm list and the event log share (specification 39, §5.7), for rows that do
// not fit.

import { token } from "../parts";
import { el, SvgNode, text } from "../svg";
import { say } from "../speech";

/** The room the pager takes, from its right edge. */
export const PAGER_WIDTH = 204;
const BUTTON = { width: 40, height: 36 };

/**
 * "1–6 of 9" with buttons for the previous and the next page, its right edge at `right`. Each
 * usable button names the first row of its page in `data-page-first`, so the display only has to
 * remember it.
 */
export function pager(kind: string, first: number, shown: number, total: number, right: number, top: number): SvgNode {
  const page = (direction: "previous" | "next", left: number, target: number | undefined): SvgNode => {
    const usable = target !== undefined;
    return el(
      "g",
      {
        "data-part": `${kind}-page`,
        [`data-${kind}-page`]: direction,
        "data-page-first": target,
        "data-disabled": usable ? undefined : "true",
        cursor: usable ? "pointer" : undefined,
      },
      [
        el("rect", {
          x: left,
          y: top,
          width: BUTTON.width,
          height: BUTTON.height,
          rx: BUTTON.height / 2,
          fill: token("raised"),
          stroke: token("border"),
        }),
        text(direction === "previous" ? "‹" : "›", {
          x: left + BUTTON.width / 2,
          y: top + 24,
          fill: usable ? token("text") : token("unknown"),
          "font-size": 20,
          "font-weight": 700,
          "text-anchor": "middle",
        }),
      ],
    );
  };
  const capacity = Math.max(1, shown);
  const previous = first > 0 ? Math.max(0, first - capacity) : undefined;
  const next = first + shown < total ? first + shown : undefined;
  return el("g", { "data-part": `${kind}-pager` }, [
    text(say("{first}–{last} of {total}", { first: first + 1, last: first + shown, total }), {
      x: right - 2 * BUTTON.width - 20,
      y: top + 23,
      fill: token("muted"),
      "font-size": 14,
      "font-weight": 550,
      "text-anchor": "end",
      "font-variant-numeric": "tabular-nums",
    }),
    page("previous", right - 2 * BUTTON.width - 8, previous),
    page("next", right - BUTTON.width, next),
  ]);
}
