// The slider (§5.8): a track from its minimum to its maximum, filled in the accent up to the
// value, with a thumb and the value as a number. While it is dragged it shows the draft.

import { fitWidth, formatValue } from "../format";
import type { DrawFrame, HmiObjectModel } from "../model";
import { hatchBox, stateAttributes, token } from "../parts";
import { el, SvgNode, text } from "../svg";
import { liveValueOf } from "../values";
import { controlGroup, outcomeOutline } from "./controls";

const THUMB = 11;
/** Room for the value as a number, beside a horizontal track or above a vertical one. */
const NUMBER_ROOM = 64;
/** Room for the label above the track. */
const LABEL_ROOM = 22;

/** A slider's limits: its own, else 0 to 100. */
export function sliderLimits(object: HmiObjectModel): [number, number] {
  return [object.min ?? 0, object.max ?? 100];
}

/** The track in the slider's own coordinates: from (x1, y1) at the minimum to (x2, y2) at the
 * maximum. */
export function sliderTrack(object: HmiObjectModel): [number, number, number, number] {
  const [width, height] = object.size;
  const label = object.label ? LABEL_ROOM : 0;
  if (object.variant === "vertical") return [width / 2, height - THUMB - 4, width / 2, label + NUMBER_ROOM / 2 + THUMB];
  // With a label the track runs below it; otherwise through the middle.
  const y = object.label ? Math.max(height / 2, label + (height - label) / 2) : height / 2;
  return [THUMB + 4, y, width - NUMBER_ROOM - THUMB, y];
}

/** The value at a point of the page (in design pixels), along the slider's track, within its
 * limits and rounded to its decimals. */
export function sliderValueAt(object: HmiObjectModel, x: number, y: number): number {
  const [x1, y1, x2, y2] = sliderTrack(object);
  const [min, max] = sliderLimits(object);
  const local = { x: x - object.at[0], y: y - object.at[1] };
  const length = Math.hypot(x2 - x1, y2 - y1);
  const along = length === 0 ? 0 : ((local.x - x1) * (x2 - x1) + (local.y - y1) * (y2 - y1)) / (length * length);
  const share = Math.min(1, Math.max(0, along));
  const factor = 10 ** (object.decimals ?? 0);
  return Math.round((min + share * (max - min)) * factor) / factor;
}

export function drawSlider(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const live = liveValueOf(object, "value", frame);
  const draft = frame.drafts?.[object.id];
  const [min, max] = sliderLimits(object);
  const vertical = object.variant === "vertical";
  const shown = draft ?? (typeof live?.value === "number" ? live.value : undefined);
  const unknown = draft === undefined && (!live || live.state === "unknown" || shown === undefined);
  const share = shown === undefined || max === min ? 0 : Math.min(1, Math.max(0, (shown - min) / (max - min)));
  const [x1, y1, x2, y2] = sliderTrack(object);
  const at = { x: x1 + (x2 - x1) * share, y: y1 + (y2 - y1) * share };
  const children: SvgNode[] = [
    el("line", {
      "data-part": "track",
      x1,
      y1,
      x2,
      y2,
      stroke: token("raised"),
      "stroke-width": 6,
      "stroke-linecap": "round",
    }),
  ];
  const number = vertical
    ? { x: width / 2, y: (object.label ? LABEL_ROOM : 0) + 18, "text-anchor": "middle" }
    : { x: width - 4, y: y1 + 6, "text-anchor": "end" };
  if (object.label) {
    const room = vertical ? width - 8 : width - (x1 - THUMB) - 4;
    children.push(
      text(fitWidth(object.label, room, 14, 500), {
        x: vertical ? width / 2 : x1 - THUMB,
        y: 16,
        "text-anchor": vertical ? "middle" : undefined,
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 500,
      }),
    );
  }
  if (unknown) {
    const box = vertical
      ? { x: width / 2 - 6, y: y2 - 3, width: 12, height: y1 - y2 + 6 }
      : { x: x1 - 3, y: y1 - 6, width: x2 - x1 + 6, height: 12 };
    children.push(hatchBox(box, 6));
    children.push(text("?", { ...number, fill: token("unknown"), "font-size": 16, "font-weight": 650 }));
  } else {
    children.push(
      el("line", {
        "data-part": "fill",
        x1,
        y1,
        x2: at.x,
        y2: at.y,
        stroke: token("accent"),
        "stroke-width": 6,
        "stroke-linecap": "round",
      }),
    );
    children.push(
      el("circle", {
        "data-part": "thumb",
        cx: at.x,
        cy: at.y,
        r: THUMB,
        fill: token("text"),
        stroke: token("accent"),
        "stroke-width": draft === undefined ? 2 : 3,
      }),
    );
    const unit = object.unit ?? object.bind.value?.unit;
    const words = `${formatValue(shown ?? null, object)}${unit ? ` ${unit}` : ""}`;
    children.push(
      text(words, {
        ...number,
        fill: draft === undefined ? token("text") : token("accent"),
        "font-size": 16,
        "font-weight": 650,
        "font-variant-numeric": "tabular-nums",
      }),
    );
  }
  if (live?.staleMs !== undefined && draft === undefined) {
    children.push(
      el("rect", {
        "data-part": "stale",
        x: 0.5,
        y: 0.5,
        width: width - 1,
        height: height - 1,
        rx: 10,
        fill: "none",
        stroke: token("stale"),
        "stroke-dasharray": "5 4",
      }),
    );
  }
  children.push(...outcomeOutline(object, frame, { x: 0, y: 0, width, height }, 10));
  return controlGroup(
    "slider",
    object,
    frame,
    { ...stateAttributes(live), "data-draft": draft === undefined ? undefined : "true" },
    children,
  );
}
