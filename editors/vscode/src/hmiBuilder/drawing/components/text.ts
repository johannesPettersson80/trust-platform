// Text: words or a bound value, in the size of its variant — heading, body or caption (§5.1).

import { fitWidth, formatValue } from "../format";
import type { DrawFrame, HmiObjectModel } from "../model";
import { stateAttributes, token } from "../parts";
import { el, SvgNode, text } from "../svg";
import { liveValueOf } from "../values";

const VARIANTS = {
  heading: { size: 26, weight: 650, colour: "text" },
  body: { size: 18, weight: 500, colour: "text" },
  caption: { size: 14, weight: 500, colour: "muted" },
} as const;

export function drawText(object: HmiObjectModel, drawFrame: DrawFrame): SvgNode {
  const height = object.size[1];
  const variant = VARIANTS[(object.variant ?? "body") as keyof typeof VARIANTS] ?? VARIANTS.body;
  const live = liveValueOf(object, "text", drawFrame);
  const unknown = live?.state === "unknown";
  const content = live ? (unknown ? "?" : formatValue(live.value, object)) : (object.text ?? "");
  return el("g", { class: "hmi-object hmi-text", ...stateAttributes(live) }, [
    // Text that does not fit is cut with "…" (§14.5 item 6).
    text(fitWidth(content, object.size[0], variant.size, variant.weight), {
      x: 0,
      y: height / 2 + variant.size * 0.35,
      fill: unknown ? token("unknown") : token(variant.colour),
      "font-size": variant.size,
      "font-weight": variant.weight,
      "letter-spacing": variant === VARIANTS.heading ? -0.3 : undefined,
    }),
  ]);
}
