// The value display: a card with a label, a large number and its unit, and chips for the
// value's state (§5.2, §5.3, §14.1).

import { fitWidth, formatValue, textWidth } from "../format";
import type { DrawFrame, HmiObjectModel, LiveValue } from "../model";
import { card, fontSize, hatch, stateAttributes, stateChipRow, stateChips, token } from "../parts";
import { el, run, SvgNode, text, tspan } from "../svg";
import { liveValueOf } from "../values";

const INSET = 16;
/** Below this height a labelled card has no room for two lines: label and number share one. */
const COMPACT_HEIGHT = 72;

export function drawValue(object: HmiObjectModel, drawFrame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const live = liveValueOf(object, "value", drawFrame);
  if (object.label && height < COMPACT_HEIGHT) return drawCompactValue(object, live, width, height);
  const labelled = Boolean(object.label);
  const size = fontSize(labelled ? height - 26 : height, labelled ? 0.62 : 0.5);
  const baseline = height - Math.max(12, Math.round((height - (labelled ? 26 : 0) - size) / 2));
  const children: SvgNode[] = [card(width, height, live)];
  const chips = stateChipRow(object, live, width);
  if (object.label) {
    // The label ends before the state chips beside it, cut with "…" when it does not fit.
    const room = Math.min(width - 2 * INSET, chips.left - 8 - INSET);
    children.push(
      text(fitWidth(object.label, room, 14, 500), {
        x: INSET,
        y: 27,
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 500,
      }),
    );
  }
  const big = {
    x: INSET,
    y: baseline,
    "font-size": size,
    "font-weight": 600,
    "letter-spacing": -0.5,
    "font-variant-numeric": "tabular-nums",
  };
  if (live?.state === "unknown") {
    children.push(hatch(width, height));
    children.push(text("?", { ...big, fill: token("unknown") }));
  } else if (live) {
    const unit = object.unit ?? object.bind.value?.unit;
    children.push(
      run({ ...big, fill: token("text") }, [
        tspan(formatValue(live.value, object)),
        ...(unit
          ? [tspan(unit, { dx: 6, fill: token("muted"), "font-size": 15, "font-weight": 500, "letter-spacing": 0 })]
          : []),
      ]),
    );
  } else if (object.text) {
    children.push(text(fitWidth(object.text, width - 2 * INSET, size, 600), { ...big, fill: token("text") }));
  }
  children.push(...chips.nodes);
  return el("g", { class: "hmi-object hmi-value", ...stateAttributes(live) }, children);
}

/** A short card: the label on the left, the number with its unit on the right, on one line; the
 * state chips sit at the right end and the number moves left of them. */
function drawCompactValue(object: HmiObjectModel, live: LiveValue | undefined, width: number, height: number): SvgNode {
  const baseline = height / 2 + 6;
  const chips = stateChips(object, live, width, (height - 22) / 2);
  const right =
    chips.length > 0 ? Math.min(...chips.map((chip) => Number(chip.children[0].attrs.x))) - 8 : width - INSET;
  const numberSize = Math.min(22, fontSize(height, 0.42));
  const unit = object.unit ?? object.bind.value?.unit;
  // The label takes the room the number leaves, cut with "…" when it is longer.
  const shown = live?.state === "unknown" ? "?" : live ? formatValue(live.value, object) : (object.text ?? "");
  const numberWidth =
    textWidth(shown, numberSize, 600) + (live && live.state !== "unknown" && unit ? 4 + textWidth(unit, 14, 500) : 0);
  const label = fitWidth(object.label ?? "", right - numberWidth - 8 - INSET, 14, 500);
  const children: SvgNode[] = [
    card(width, height, live),
    text(label, { x: INSET, y: baseline, fill: token("muted"), "font-size": 14, "font-weight": 500 }),
  ];
  const number = {
    x: right,
    y: baseline,
    "font-size": numberSize,
    "font-weight": 600,
    "text-anchor": "end",
    "font-variant-numeric": "tabular-nums",
  };
  if (live?.state === "unknown") {
    children.push(hatch(width, height));
    children.push(text("?", { ...number, fill: token("unknown") }));
  } else if (live) {
    children.push(
      run({ ...number, fill: token("text") }, [
        tspan(shown),
        ...(unit ? [tspan(unit, { dx: 4, fill: token("muted"), "font-size": 14, "font-weight": 500 })] : []),
      ]),
    );
  }
  if (!live && object.text) {
    children.push(text(fitWidth(object.text, right - INSET, numberSize, 600), { ...number, fill: token("text") }));
  }
  children.push(...chips);
  return el("g", { class: "hmi-object hmi-value hmi-value-compact", ...stateAttributes(live) }, children);
}
