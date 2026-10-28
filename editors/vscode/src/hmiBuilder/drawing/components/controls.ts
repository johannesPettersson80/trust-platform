// Controls (§5.2, §12.4–12.5): the setpoint entry and the numeric input, and the buttons. Each
// shows the latest outcome of its command; on a read-only page they look unavailable.

import { fitWidth, formatValue, textWidth } from "../format";
import type { CommandState, DrawFrame, HmiObjectModel } from "../model";
import { ACCENT_ACROSS, card, chip, fontSize, hatch, stateAttributes, token, TokenName } from "../parts";
import { el, run, SvgNode, text, tspan } from "../svg";
import { liveValueOf } from "../values";
import { say } from "../speech";
import type { Word } from "../words";

const INSET = 16;
/** Below this height a setpoint entry has no room for its three lines: it draws one. */
const COMPACT_HEIGHT = 80;
/** The least room a label is drawn in: about four letters at 14 px, so it never shows one. */
const MIN_LABEL = 32;
/** The height a setpoint entry keeps for its label line and footer above and below its number. */
const NUMBER_ROOM = 56;
/** How much an unavailable control's decorative surface shows through. Text stays readable. */
const UNAVAILABLE = 0.45;

/** Subdue decoration once, without multiplying opacity onto labels, readings or outcomes. */
function unavailableContent(node: SvgNode): SvgNode {
  if (String(node.attrs["data-part"] ?? "").startsWith("outcome")) return node;
  const attrs = { ...node.attrs };
  if (node.tag === "text" || node.tag === "tspan") {
    if (attrs.fill === token("page")) attrs.fill = token("text");
  } else if (["rect", "circle", "ellipse", "path", "line", "polyline", "polygon"].includes(node.tag)) {
    attrs.opacity = String(Number(attrs.opacity ?? 1) * UNAVAILABLE);
  }
  return { ...node, attrs, children: node.children.map(unavailableContent) };
}

/** How an outcome reads, and its colour. */
export const OUTCOMES: Record<CommandState["outcome"], [Word, TokenName]> = {
  pending: ["Sending…", "pending"],
  applied: ["Applied", "muted"],
  confirmed: ["Confirmed", "accent"],
  refused: ["Refused", "alarm"],
  failed: ["Failed", "alarm"],
  "timed-out": ["Not confirmed", "warning"],
  unknown: ["Unknown", "unknown"],
};

/** The sign of each outcome on a button (§3.5). */
const SIGNS: Record<CommandState["outcome"], string> = {
  pending: "…",
  applied: "✓",
  confirmed: "✓",
  refused: "!",
  failed: "!",
  "timed-out": "!",
  unknown: "?",
};

/** A tooltip: the words a pointer resting on the control shows. */
export function tooltip(words: string): SvgNode {
  return { ...el("title"), text: words };
}

/** Why a control is held, when its `interlock` permit reads FALSE (§8.4 item 4, §12.6): the
 * permit's reason, or an empty text without one; `undefined` while it is not held. The program
 * decides: a press still goes to the runtime, which refuses it with that reason. */
export function heldReason(object: HmiObjectModel, frame: DrawFrame): string | undefined {
  const permit = liveValueOf(object, "interlock", frame);
  if (!permit || permit.state === "unknown" || permit.value !== false) return undefined;
  const reason = liveValueOf(object, "interlock-reason", frame);
  return typeof reason?.value === "string" ? reason.value.trim() : "";
}

/** A control's group: its outcome and availability for styles and tests. Its first child is a
 * clear target over the whole box, so a press anywhere on it uses the control (§3.5). A variant
 * that looks different (a `stop` button) adds its class, `hmi-<kind>-<variant>`. A held control is
 * greyed, with its reason as the tooltip until a command of its own has an outcome. */
export function controlGroup(
  kind: string,
  object: HmiObjectModel,
  frame: DrawFrame,
  attributes: Record<string, string | undefined>,
  children: SvgNode[],
  variant?: string,
): SvgNode {
  const command = frame.commands?.[object.id];
  const [width, height] = object.size;
  const target = el("rect", { "data-part": "target", x: 0, y: 0, width, height, fill: "transparent" });
  // A control the signed-in role may not use shows through, under a lock naming who may (§5.9).
  const locked = frame.locked?.[object.id];
  const held = heldReason(object, frame);
  const content = frame.readOnly || locked || held !== undefined ? children.map(unavailableContent) : children;
  if (locked) content.push(lockBadge(locked, width));
  if (held && !command) content.push(tooltip(held));
  return el(
    "g",
    {
      class: `hmi-object hmi-control hmi-${kind}${variant ? ` hmi-${kind}-${variant}` : ""}`,
      ...attributes,
      "data-outcome": command?.outcome,
      "data-readonly": frame.readOnly ? "true" : undefined,
      "data-locked": locked,
      "data-held": held !== undefined ? "true" : undefined,
    },
    [target, ...content],
  );
}

/** A role's name as the display shows it. */
export function roleWord(role: string): string {
  return role === "engineer" ? say("Engineer") : role === "operator" ? say("Operator") : say("Locked");
}

/** The lock over a control's top right corner, with the role that may use it. */
function lockBadge(role: string, width: number): SvgNode {
  const label = roleWord(role);
  const badge = Math.round(textWidth(label, 14, 600) + 36);
  const x = Math.max(0, width - badge - 8);
  return el("g", { "data-part": "lock", transform: `translate(${x} -12)` }, [
    el("rect", { x: 0, y: 0, width: badge, height: 24, rx: 12, fill: token("raised"), stroke: token("border") }),
    el("rect", { x: 10, y: 10, width: 10, height: 8, rx: 2, fill: token("muted") }),
    el("path", { d: "M12 10 V8 a3 3 0 0 1 6 0 V10", fill: "none", stroke: token("muted"), "stroke-width": 1.6 }),
    text(label, { x: 27, y: 17, fill: token("text"), "font-size": 14, "font-weight": 600 }),
  ]);
}

/** The setpoint entry and the numeric input: the program's value (never the requested one), the
 * limits, the mark that it can be changed, and the outcome with a refusal's reason. */
export function drawSetpoint(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  if (height < COMPACT_HEIGHT) return drawCompactSetpoint(object, frame);
  const live = liveValueOf(object, "value", frame);
  const command = frame.commands?.[object.id];
  const unit = object.unit ?? object.bind.value?.unit;
  const children: SvgNode[] = [card(width, height, live)];
  if (object.label)
    children.push(
      text(fitWidth(object.label, width - 2 * INSET, 14, 500), {
        x: INSET,
        y: 26,
        fill: token("muted"),
        "font-size": 14,
        "font-weight": 500,
      }),
    );
  // The number fills the room between the label's line and the footer: its text box (an ascent
  // of about 0.97 em above the baseline) starts below the label's (§14.5 item 6).
  const size = fontSize(height - NUMBER_ROOM, 1.03, 40);
  const number = {
    x: INSET,
    y: height - 24,
    "font-size": size,
    "font-weight": 600,
    "letter-spacing": -0.5,
    "font-variant-numeric": "tabular-nums",
  };
  if (live?.state === "unknown") {
    children.push(hatch(width, height));
    children.push(text("?", { ...number, fill: token("unknown") }));
  } else if (live) {
    children.push(
      run({ ...number, fill: token("text") }, [
        tspan(formatValue(live.value, object)),
        ...(unit
          ? [tspan(unit, { dx: 6, fill: token("muted"), "font-size": 15, "font-weight": 500, "letter-spacing": 0 })]
          : []),
      ]),
    );
  }
  const footer = { x: INSET, y: height - 8, "font-size": 14 };
  if ((command?.outcome === "refused" || command?.outcome === "failed") && command.reason) {
    children.push(
      text(fitWidth(command.reason, width - 2 * INSET, 14), { ...footer, fill: token("alarm"), "data-part": "reason" }),
    );
  } else if (object.min !== undefined || object.max !== undefined) {
    const limits = `${object.min ?? "…"} – ${object.max ?? "…"}${unit ? ` ${unit}` : ""}`;
    children.push(
      text(fitWidth(limits, width - 2 * INSET - 36, 14), { ...footer, fill: token("muted"), "data-part": "limits" }),
    );
  }
  children.push(editMark(width, height));
  if (command) {
    const [said, colour] = OUTCOMES[command.outcome];
    const word = say(said);
    children.push(chip(word, colour, width - 12, 12, false).node);
  }
  return controlGroup("setpoint", object, frame, stateAttributes(live), children);
}

/**
 * A short setpoint entry or numeric input: one line with the label on the left and the program's
 * value on the right, the edit mark inside at the right end. While a command has an outcome its
 * word, or a refusal's reason, takes the label's place, and the outline shows its colour. The
 * limits are asked for in the entry dialog (§5.8).
 */
function drawCompactSetpoint(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const live = liveValueOf(object, "value", frame);
  const command = frame.commands?.[object.id];
  const unit = object.unit ?? object.bind.value?.unit;
  const mark = Math.min(28, height - 12);
  const markX = width - 10 - mark;
  const baseline = Math.round(height / 2 + 5);
  const size = Math.min(22, fontSize(height, 0.42));
  const right = markX - 10;
  const shown = live?.state === "unknown" ? "?" : live ? formatValue(live.value, object) : "";
  const numberWidth =
    textWidth(shown, size, 600) + (live && live.state !== "unknown" && unit ? 4 + textWidth(unit, 14, 500) : 0);
  const room = right - numberWidth - 8 - INSET;
  const children: SvgNode[] = [card(width, height, live)];
  const number = {
    x: right,
    y: baseline,
    "font-size": size,
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
  // The slot left of the value: a refusal's reason when it fits, else the outcome's word, else
  // the label; none when only a letter or two would show.
  const left = { x: INSET, y: baseline, "font-size": 14 };
  const fits = (words: string): boolean => textWidth(words, 14, 600) <= room;
  const [said, colour] = command ? OUTCOMES[command.outcome] : (["Unknown", "muted"] as [Word, TokenName]);
  const word = command ? say(said) : "";
  if ((command?.outcome === "refused" || command?.outcome === "failed") && command.reason && fits(command.reason)) {
    children.push(text(command.reason, { ...left, fill: token("alarm"), "font-weight": 600, "data-part": "reason" }));
  } else if (command && fits(word)) {
    children.push(text(word, { ...left, fill: token(colour), "font-weight": 600, "data-part": "outcome-word" }));
  } else if (!command && object.label && room >= MIN_LABEL) {
    children.push(text(fitWidth(object.label, room, 14, 500), { ...left, fill: token("muted"), "font-weight": 500 }));
  }
  const at = { x: markX, y: (height - mark) / 2, size: mark };
  // Without room for the word, the outcome's sign takes the edit mark's place (§3.5).
  children.push(command && !fits(word) ? outcomeSign(command, at) : editMark(width, height, at));
  children.push(...outcomeOutline(object, frame, { x: 0, y: 0, width, height }, 14));
  return controlGroup("setpoint", object, frame, stateAttributes(live), children);
}

/** An outcome's sign in the edit mark's square. */
function outcomeSign(command: CommandState, at: { x: number; y: number; size: number }): SvgNode {
  const colour = OUTCOMES[command.outcome][1];
  return el("g", { "data-part": "outcome-sign", transform: `translate(${at.x} ${at.y})` }, [
    el("rect", {
      x: 0.5,
      y: 0.5,
      width: at.size - 1,
      height: at.size - 1,
      rx: 8,
      fill: token("raised"),
      stroke: token(colour),
    }),
    text(SIGNS[command.outcome], {
      x: at.size / 2,
      y: at.size / 2 + 5,
      "text-anchor": "middle",
      fill: token(colour),
      "font-size": 15,
      "font-weight": 800,
    }),
  ]);
}

/** The mark that a value can be changed: a pen in a rounded square at the right. */
function editMark(width: number, height: number, at?: { x: number; y: number; size: number }): SvgNode {
  const size = at?.size ?? 28;
  const x = at?.x ?? width - INSET - size;
  const y = at?.y ?? Math.max(34, (height - size) / 2 + 4);
  const scale = size / 28;
  return el("g", { "data-part": "edit", transform: `translate(${x} ${y})` }, [
    el("rect", {
      x: 0.5,
      y: 0.5,
      width: size - 1,
      height: size - 1,
      rx: 8,
      fill: token("raised"),
      stroke: token("border"),
    }),
    el("path", {
      d: "M9 19 L9 16 L17 8 L20 11 L12 19 Z M15.5 9.5 L18.5 12.5",
      fill: "none",
      stroke: token("accent"),
      "stroke-width": 1.6,
      "stroke-linejoin": "round",
      ...(scale !== 1 ? { transform: `scale(${scale})` } : {}),
    }),
  ]);
}

/** A button or a pulse button: an accent pill with its text; the outline and a sign at its right
 * end show the outcome. The `stop` variant is a raised pill outlined and lettered in the alarm
 * colour, so Stop never looks like Start (§8.4 item 4). */
export function drawButton(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const command = frame.commands?.[object.id];
  const radius = Math.min(height / 2, 22);
  const stop = object.variant === "stop";
  const face = stop ? { fill: token("raised"), stroke: token("alarm"), "stroke-width": 1.5 } : { fill: ACCENT_ACROSS };
  const children: SvgNode[] = [
    el("rect", {
      "data-part": "face",
      x: 0.5,
      y: 0.5,
      width: Math.max(0, width - 1),
      height: Math.max(0, height - 1),
      rx: radius,
      ...face,
    }),
  ];
  const label = object.text ?? object.label ?? "";
  const size = Math.min(16, fontSize(height, 0.4));
  const cx = width - Math.min(radius, 18);
  // With an outcome's sign at the right end, the text centres in the room left of it; a text that
  // does not fit is cut with "…" (§14.5 item 6).
  const [from, to] = command ? [12, cx - 9 - 4] : [12, width - 12];
  children.push(
    text(fitWidth(label, to - from, size, 650), {
      x: (from + to) / 2,
      y: height / 2 + 5,
      "text-anchor": "middle",
      fill: token(stop ? "alarm" : "page"),
      "font-size": size,
      "font-weight": 650,
    }),
  );
  if (command) {
    const [said, colour] = OUTCOMES[command.outcome];
    const word = say(said);
    children.push(
      el("rect", {
        "data-part": "outcome",
        x: -1.5,
        y: -1.5,
        width: width + 3,
        height: height + 3,
        rx: radius + 2,
        fill: "none",
        stroke: token(colour),
        "stroke-width": 2,
      }),
    );
    // A sign carries the outcome as well as the colour (§3.5).
    children.push(
      el("g", { "data-part": "outcome-sign" }, [
        el("circle", { cx, cy: height / 2, r: 9, fill: token("page"), "fill-opacity": 0.85 }),
        text(SIGNS[command.outcome], {
          x: cx,
          y: height / 2 + 4.5,
          "text-anchor": "middle",
          fill: token(colour),
          "font-size": 13,
          "font-weight": 800,
        }),
      ]),
    );
    // A refusal's reason, "Not accepted: …", is the button's tooltip (§21.2 item 3).
    const refusal = (command.outcome === "refused" || command.outcome === "failed") && command.reason;
    children.push(tooltip(refusal || word));
  }
  return controlGroup(object.type, object, frame, {}, children, stop ? "stop" : undefined);
}

/** A rectangle in an object's own coordinates. */
export interface OutlineBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The outline a control's latest outcome draws around a box of it (the whole control unless
 * given), with the outcome as a tooltip. */
export function outcomeOutline(object: HmiObjectModel, frame: DrawFrame, box: OutlineBox, radius: number): SvgNode[] {
  const command = frame.commands?.[object.id];
  if (!command) return [];
  const [word, colour] = OUTCOMES[command.outcome];
  return [
    el("rect", {
      "data-part": "outcome",
      x: box.x - 1.5,
      y: box.y - 1.5,
      width: box.width + 3,
      height: box.height + 3,
      rx: radius + 2,
      fill: "none",
      stroke: token(colour),
      "stroke-width": 2,
    }),
    tooltip(word),
  ];
}

/** Text cut to fit a width, at about `perChar` pixels a character. */
export function fitted(content: string, width: number, perChar: number): string {
  const room = Math.max(4, Math.floor(width / perChar));
  return content.length <= room ? content : `${content.slice(0, room - 1)}…`;
}
