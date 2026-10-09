// The switch, the selector and the mode selector (§5.8): each shows the program's value, never
// the one asked for, and "?" over the neutral hatch when it is unknown.

import { fitWidth, textWidth } from "../format";
import type { DrawFrame, HmiObjectModel, LiveValue } from "../model";
import { ACCENT_ACROSS, hatchBox, stateAttributes, token } from "../parts";
import { el, SvgNode, text } from "../svg";
import { isOn, liveValueOf } from "../values";
import { controlGroup, fitted, OutlineBox, outcomeOutline } from "./controls";
import { say, translate } from "../speech";

/** The modes a mode selector offers when its options do not narrow them. */
const MODES = ["Manual", "Auto", "Track"];

/** The stale outline of a control whose value is stale, around a box of it. */
function staleOutline(live: LiveValue | undefined, box: OutlineBox, radius: number): SvgNode[] {
  if (live?.staleMs === undefined) return [];
  return [
    el("rect", {
      "data-part": "stale",
      x: box.x + 0.5,
      y: box.y + 0.5,
      width: box.width - 1,
      height: box.height - 1,
      rx: radius,
      fill: "none",
      stroke: token("stale"),
      "stroke-dasharray": "5 4",
    }),
  ];
}

/** A switch: a pill whose knob sits right and lights the track when on, with the word On or
 * Off beside the knob; a label, when there is room, on its left. The track has a control's usual
 * size, at most 32 px high and centred, so a tall object keeps a calm toggle beside its label while
 * the whole object still takes the click. */
const SWITCH_TRACK_HEIGHT = 32;

export function drawSwitch(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const live = liveValueOf(object, "value", frame);
  const trackHeight = Math.min(height, SWITCH_TRACK_HEIGHT);
  const top = (height - trackHeight) / 2;
  // Its two states' words, off first: its options (a loop's ["Auto", "Manual"]), else Off and On.
  const [offWord, onWord] =
    object.options?.length === 2 ? object.options.map((option) => translate(option)) : [say("Off"), say("On")];
  const wordWidth = Math.max(textWidth(offWord, 14, 650), textWidth(onWord, 14, 650));
  const trackWidth = Math.min(
    width,
    Math.max(64, Math.round(trackHeight * 2.2), Math.round(wordWidth + trackHeight + 8)),
  );
  const x = width - trackWidth;
  const radius = trackHeight / 2;
  const unknown = !live || live.state === "unknown";
  const on = !unknown && isOn(live.value);
  const children: SvgNode[] = [];
  if (object.label && x >= 40) {
    children.push(
      text(fitted(object.label, x - 12, 7.4), {
        x: 0,
        y: height / 2 + 5,
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 500,
      }),
    );
  }
  children.push(
    el("rect", {
      "data-part": "track",
      x: x + 0.5,
      y: top + 0.5,
      width: trackWidth - 1,
      height: trackHeight - 1,
      rx: radius,
      fill: on ? ACCENT_ACROSS : token("raised"),
      stroke: on ? "none" : token("border"),
    }),
  );
  // No words below 14 px (specification 39, §14.1).
  const words = { y: height / 2 + 5, "font-size": 14, "font-weight": 650 };
  if (unknown) {
    children.push(hatchBox({ x: x + 1.5, y: top + 1.5, width: trackWidth - 3, height: trackHeight - 3 }, radius));
    children.push(text("?", { ...words, x: x + trackWidth / 2, "text-anchor": "middle", fill: token("unknown") }));
  } else {
    const knob = radius - 3;
    const cx = on ? width - 3 - knob : x + 3 + knob;
    children.push(
      el("circle", { "data-part": "knob", cx, cy: height / 2, r: knob, fill: on ? token("page") : token("muted") }),
    );
    children.push(
      on
        ? text(onWord, {
            ...words,
            x: x + (trackWidth - 2 * knob - 6) / 2,
            "text-anchor": "middle",
            fill: token("page"),
          })
        : text(offWord, {
            ...words,
            x: x + 6 + 2 * knob + (trackWidth - 2 * knob - 6) / 2,
            "text-anchor": "middle",
            fill: token("muted"),
          }),
    );
  }
  const track = { x, y: top, width: trackWidth, height: trackHeight };
  children.push(...staleOutline(live, track, radius), ...outcomeOutline(object, frame, track, radius));
  return controlGroup(
    "switch",
    object,
    frame,
    { ...stateAttributes(live), "data-on": unknown ? undefined : String(on) },
    children,
  );
}

/** An option as words: an enumeration's value name split where its words meet (OutOfService as
 * "Out of service"), in the display's language; a word in capitals, as PID, stays. */
export function optionWords(option: string): string {
  const words = option.replace(/([a-z0-9])([A-Z])/g, "$1 $2").split(" ");
  return translate(
    words
      .map((word, index) =>
        index === 0 || (word.length > 1 && word === word.toUpperCase()) ? word : word.toLowerCase(),
      )
      .join(" "),
  );
}

/** The option a value stands for: the option with its name (any case), or for a number the
 * option at that position; otherwise the value itself. */
function optionOf(value: LiveValue["value"], options: string[]): string {
  if (typeof value === "string") return options.find((option) => option.toLowerCase() === value.toLowerCase()) ?? value;
  if (typeof value === "number" && Number.isInteger(value) && value >= 0 && value < options.length)
    return options[value];
  return value === null ? "?" : String(value);
}

/** A selector: a box with its label, the current choice and a chevron, the sign that a click
 * opens the list of options. */
export function drawSelector(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const live = liveValueOf(object, "value", frame);
  const unknown = !live || live.state === "unknown";
  const radius = Math.min(12, height / 2);
  const children: SvgNode[] = [
    el("rect", {
      "data-part": "box",
      x: 0.5,
      y: 0.5,
      width: width - 1,
      height: height - 1,
      rx: radius,
      fill: token("raised"),
      stroke: token("border"),
    }),
  ];
  const baseline = height / 2 + 5;
  let left = 14;
  if (object.label) {
    const label = fitWidth(object.label, width / 2 - 20, 14, 500);
    children.push(text(label, { x: left, y: baseline, fill: token("muted"), "font-size": 14, "font-weight": 500 }));
    left += Math.round(textWidth(label, 14, 500)) + 14;
  }
  if (unknown) {
    children.push(hatchBox({ x: 1.5, y: 1.5, width: width - 3, height: height - 3 }, radius));
    children.push(text("?", { x: left, y: baseline, fill: token("unknown"), "font-size": 16, "font-weight": 650 }));
  } else {
    const choice = fitWidth(optionWords(optionOf(live.value, object.options ?? [])), width - left - 40, 16, 650);
    children.push(
      text(choice, {
        x: left,
        y: baseline,
        fill: token("accent"),
        "font-size": 16,
        "font-weight": 650,
        "data-part": "choice",
      }),
    );
  }
  const cx = width - 20;
  children.push(
    el("path", {
      "data-part": "chevron",
      d: `M${cx - 5} ${height / 2 - 2} L${cx} ${height / 2 + 3} L${cx + 5} ${height / 2 - 2}`,
      fill: "none",
      stroke: token("muted"),
      "stroke-width": 1.8,
      "stroke-linecap": "round",
      "stroke-linejoin": "round",
    }),
  );
  const whole = { x: 0, y: 0, width, height };
  children.push(...staleOutline(live, whole, radius), ...outcomeOutline(object, frame, whole, radius));
  return controlGroup("selector", object, frame, stateAttributes(live), children);
}

/** A mode selector: its modes side by side in a track; the active one filled in the accent,
 * its word bold. */
export function drawModeSelector(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const live = liveValueOf(object, "mode", frame);
  const unknown = !live || live.state === "unknown";
  const modes = object.options && object.options.length > 0 ? object.options : MODES;
  const active = unknown || typeof live.value !== "string" ? undefined : live.value.toLowerCase();
  const radius = height / 2;
  const segment = (width - 8) / modes.length;
  const children: SvgNode[] = [
    el("rect", {
      "data-part": "track",
      x: 0.5,
      y: 0.5,
      width: width - 1,
      height: height - 1,
      rx: radius,
      fill: token("raised"),
      stroke: token("border"),
    }),
  ];
  modes.forEach((mode, index) => {
    const lit = mode.toLowerCase() === active;
    const x = 4 + index * segment;
    children.push(
      el("g", { "data-option": mode, "data-active": lit ? "true" : undefined }, [
        el("rect", {
          x,
          y: 4,
          width: segment,
          height: height - 8,
          rx: radius - 4,
          fill: lit ? ACCENT_ACROSS : "transparent",
        }),
        text(fitted(optionWords(mode), segment - 12, 7.6), {
          x: x + segment / 2,
          y: height / 2 + 5,
          "text-anchor": "middle",
          "font-size": 14,
          "font-weight": lit ? 700 : 500,
          fill: lit ? token("page") : token("muted"),
        }),
      ]),
    );
  });
  if (unknown) children.push(hatchBox({ x: 1.5, y: 1.5, width: width - 3, height: height - 3 }, radius));
  const whole = { x: 0, y: 0, width, height };
  children.push(...staleOutline(live, whole, radius), ...outcomeOutline(object, frame, whole, radius));
  return controlGroup("mode-selector", object, frame, stateAttributes(live), children);
}
