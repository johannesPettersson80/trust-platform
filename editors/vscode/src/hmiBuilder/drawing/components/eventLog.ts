// The event log (specification 39, §5.7 item 5, §13.7): a row per record, newest first, with
// what happened and the alarm's message or what a command asked, and below it the time, the path,
// the user and the detail; pages for rows that do not fit.

import { alarmPriority, priorityMark } from "../alarms";
import { fitWidth, timeOfDay } from "../format";
import type { AlarmRecordModel, DrawFrame, HmiObjectModel } from "../model";
import { token, TokenName } from "../parts";
import { el, SvgNode, text } from "../svg";
import { pager, PAGER_WIDTH } from "./listParts";
import { say, translate } from "../speech";
import type { Word } from "../words";

const KINDS: Record<string, Word> = {
  raised: "Raised",
  cleared: "Cleared",
  acknowledged: "Acknowledged",
  shelved: "Shelved",
  unshelved: "Unshelved",
  suppressed: "Suppressed",
  unsuppressed: "Unsuppressed",
  "out-of-service": "Out of service",
  "in-service": "In service",
  reset: "Reset",
  comment: "Comment",
};

const OUTCOMES: Record<string, { label: Word; colour: TokenName; sign: string }> = {
  confirmed: { label: "Confirmed", colour: "accent", sign: "✓" },
  applied: { label: "Applied", colour: "accent", sign: "✓" },
  refused: { label: "Refused", colour: "alarm", sign: "!" },
  "timed-out": { label: "Timed out", colour: "warning", sign: "!" },
};

const PAD = 16;
const HEADER = 56;
const ROW = 52;
const FOOTER = 48;

export function drawEventLog(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const alarms = frame.alarms;
  const stale = alarms?.stale === true;
  const records = [...(alarms?.records ?? [])].sort((a, b) => b.seq - a.seq);
  const children: SvgNode[] = [
    el("rect", {
      "data-part": "card",
      x: 0.5,
      y: 0.5,
      width: Math.max(0, width - 1),
      height: Math.max(0, height - 1),
      rx: 14,
      fill: token("surface"),
      stroke: stale ? token("stale") : token("border"),
      "stroke-dasharray": stale ? "6 4" : undefined,
    }),
    text(say("Events"), { x: PAD + 4, y: 33, fill: token("text"), "font-size": 15, "font-weight": 650 }),
  ];
  const headerFits = PAD + 80 + PAGER_WIDTH <= width - PAD;
  const room = (footer: boolean): number => Math.max(1, Math.floor((height - HEADER - (footer ? FOOTER : 8)) / ROW));
  const paged = records.length > room(false);
  const capacity = paged && !headerFits ? room(true) : room(false);
  const lastPage = Math.max(0, Math.floor((records.length - 1) / capacity) * capacity);
  const first = Math.min(Math.max(0, alarms?.views?.[object.id]?.first ?? 0), lastPage);
  const shown = records.slice(first, first + capacity);
  if (paged)
    children.push(
      pager("event", first, shown.length, records.length, width - PAD, headerFits ? 10 : height - FOOTER + 4),
    );
  if (shown.length === 0) {
    children.push(
      text(say("No events yet"), {
        x: width / 2,
        y: HEADER + (height - HEADER) / 2,
        fill: token("muted"),
        "font-size": 15,
        "font-weight": 550,
        "text-anchor": "middle",
      }),
    );
  }
  shown.forEach((record, index) => children.push(drawRecord(record, HEADER + index * ROW, width)));
  return el("g", { class: "hmi-object hmi-event-log", "data-stale": stale ? "true" : undefined }, children);
}

function drawRecord(record: AlarmRecordModel, top: number, width: number): SvgNode {
  const outcome = record.kind === "command" ? OUTCOMES[record.outcome ?? ""] : undefined;
  const kind = KINDS[record.kind];
  const what =
    record.kind === "command"
      ? outcome
        ? say(outcome.label)
        : (record.outcome ?? say("Command"))
      : kind
        ? say(kind)
        : record.kind;
  const detail = record.detail ? (record.kind === "comment" ? `“${record.detail}”` : record.detail) : undefined;
  const below = [timeOfDay(record.time_ms), record.path, record.user, detail]
    .filter((part): part is string => Boolean(part))
    .join(" · ");
  const right = width - PAD;
  return el("g", { "data-part": "event-row", "data-seq": record.seq, "data-kind": record.kind }, [
    el("line", { x1: PAD, y1: top, x2: width - PAD, y2: top, stroke: token("line"), "stroke-width": 1 }),
    markOf(record, outcome, PAD + 12, top + ROW / 2),
    text(fitWidth(`${what} · ${translate(record.message || record.path)}`, right - 48, 15, 600), {
      x: 48,
      y: top + 22,
      fill: token("text"),
      "font-size": 15,
      "font-weight": 600,
    }),
    text(fitWidth(below, right - 48, 14), {
      x: 48,
      y: top + 42,
      fill: token("muted"),
      "font-size": 14,
      "font-variant-numeric": "tabular-nums",
    }),
  ]);
}

/** An alarm's record carries its priority's mark; a command's record a mark of its outcome. */
function markOf(
  record: AlarmRecordModel,
  outcome: (typeof OUTCOMES)[string] | undefined,
  x: number,
  y: number,
): SvgNode {
  if (record.kind !== "command") return priorityMark(alarmPriority(record.priority), x, y, 20);
  const colour = outcome?.colour ?? "muted";
  return el("g", { "data-part": "outcome-mark", "data-outcome": record.outcome ?? "" }, [
    el("circle", { cx: x, cy: y, r: 9, fill: "none", stroke: token(colour), "stroke-width": 2 }),
    text(outcome?.sign ?? "·", {
      x,
      y: y + 4.5,
      fill: token(colour),
      "font-size": 12,
      "font-weight": 800,
      "text-anchor": "middle",
    }),
  ]);
}
