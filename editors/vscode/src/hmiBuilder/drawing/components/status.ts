// The status indicator: a pill with a lamp that lights in the accent, or alarm red for an alarm,
// and stays dark when off (§5.2, §14.1). On is a filled disc and off a hollow ring, and a lit alarm
// lamp carries "!", so the state never rests on colour alone (§3.5).

import { fitWidth } from "../format";
import type { DrawFrame, HmiObjectModel } from "../model";
import { GLOW, stateAttributes, stateChipRow, token } from "../parts";
import { el, SvgNode, text } from "../svg";
import { isOn, liveValueOf } from "../values";

export function drawStatus(object: HmiObjectModel, drawFrame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const live = liveValueOf(object, "state", drawFrame);
  const unknown = live?.state === "unknown";
  const on = live !== undefined && !unknown && isOn(live.value);
  const alarm = object.variant === "alarm";
  const radius = Math.max(6, Math.round(height * 0.2));
  const centre = { x: height / 2, y: height / 2 };
  const lit = on ? (alarm ? token("alarm") : token("accent")) : undefined;
  const children: SvgNode[] = [
    el("rect", {
      "data-part": "pill",
      x: 0.5,
      y: 0.5,
      width: width - 1,
      height: height - 1,
      rx: (height - 1) / 2,
      fill: token("surface"),
      stroke: live?.staleMs !== undefined ? token("stale") : token("border"),
    }),
    el("circle", {
      "data-part": "lamp",
      cx: centre.x,
      cy: centre.y,
      r: radius,
      fill: unknown ? "url(#hmi-unknown-hatch)" : (lit ?? "none"),
      stroke: unknown ? token("unknown") : lit ? undefined : token("muted"),
      "stroke-width": lit ? undefined : 2,
      filter: lit ? GLOW : undefined,
    }),
  ];
  if (on && alarm) {
    children.push(
      text("!", {
        x: centre.x,
        y: centre.y + 5,
        fill: token("page"),
        "font-size": 14,
        "font-weight": 800,
        "text-anchor": "middle",
        "data-part": "alarm-mark",
      }),
    );
  }
  // The value's state as chips at the pill's right end, centred in its height.
  const chips = stateChipRow(object, live, width, (height - 22) / 2);
  const caption = object.label ?? object.tag;
  if (caption && width > height + 12) {
    // The caption ends before the chips, or clear of the pill's round end, cut with "…" when it
    // does not fit (§14.5 item 6).
    const end = chips.nodes.length > 0 ? chips.left - 8 : width - Math.max(12, height / 3);
    children.push(
      text(fitWidth(caption, end - (height - 2), 14, 500), {
        x: height - 2,
        y: height / 2 + 5,
        fill: token("text"),
        "font-size": 14,
        "font-weight": 500,
      }),
    );
  }
  if (unknown) {
    children.push(
      text("?", {
        x: centre.x,
        y: centre.y + 5,
        fill: token("text"),
        "font-size": 14,
        "font-weight": 700,
        "text-anchor": "middle",
      }),
    );
  }
  children.push(...chips.nodes);
  return el(
    "g",
    {
      class: "hmi-object hmi-status",
      "data-on": live === undefined || unknown ? undefined : String(on),
      ...stateAttributes(live),
    },
    children,
  );
}
