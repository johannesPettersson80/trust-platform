// Alarms on the runtime display (specification 39, §13.6; specification 40, §5.1): the alarm list
// and the records the value stream sends, and the top bar's counter, newest alarm and note of
// hidden alarms.

import { AlarmSound, AlarmSounds } from "./alarmSound";
import type { AlarmFrame, AlarmRecordModel, AlarmSummaryModel, HmiPageModel, ListView } from "../drawing/model";
import type { DisplayHost } from "./runtimeDisplay";
import { say, translate } from "../drawing/speech";

export type { AlarmRowModel as AlarmRow } from "../drawing/model";
export type AlarmSummary = AlarmSummaryModel;
export type AlarmRecord = AlarmRecordModel;

/** A stream message with the list or new records. */
export interface AlarmMessage {
  alarms?: AlarmSummary;
  events?: AlarmRecord[];
}

/** The top bar's alarm area. */
export interface AlarmArea {
  count: HTMLElement;
  newest: HTMLElement;
  hidden: HTMLElement;
  /** Silence, shown while a High alarm sounds (§5.9 item 2). */
  silence: HTMLButtonElement;
}

/** How the top bar draws a priority's mark and a time. */
export interface AlarmPainting {
  /** SVG markup of a priority's mark. */
  mark(priority: string): string;
  /** Local time of day with seconds. */
  time(ms: number): string;
}

/** The most records kept, as many as the runtime keeps. */
const MAX_RECORDS = 1000;
const PRIORITIES = ["high", "medium", "low"];

/** The records the runtime keeps, fetched once for a page or popup that has an event log
 * (specification 39, §5.7, item 5). */
export async function loadRecords(host: DisplayHost, model: HmiPageModel, view: AlarmView): Promise<void> {
  if (!model.object.some((object) => object.type === "event-log")) return;
  try {
    const answer = await host.fetchJson("/api/hmi/alarms");
    if (answer.status === 200 && answer.body && typeof answer.body === "object")
      view.receive(answer.body as AlarmMessage);
  } catch {
    // The stream's records still arrive; the log starts with them.
  }
}

export class AlarmView {
  summary: AlarmSummary | undefined;
  records: AlarmRecord[] = [];
  /** Each alarm list's and event log's tab and page, by object id. */
  readonly views: Record<string, ListView> = {};
  private drawn: string | undefined;
  /** The alarm sound, when the top bar shows the alarms beside its Silence (§5.9 item 2). */
  private readonly sound: AlarmSound;

  constructor(sounds?: AlarmSounds) {
    this.sound = new AlarmSound(sounds);
  }

  /** Silences the High alarms sounding now, without acknowledging them. */
  silence(): void {
    this.sound.silence(this.summary);
  }

  /** What the drawing gets for alarm lists and event logs. */
  frame(stale: boolean): AlarmFrame {
    return { summary: this.summary, records: this.records, stale, views: this.views };
  }

  receive(message: AlarmMessage): void {
    if (message.alarms) {
      this.summary = message.alarms;
      this.sound.update(this.summary);
    }
    if (!Array.isArray(message.events ?? [])) return;
    for (const record of message.events ?? []) {
      if (!this.records.some((known) => known.seq === record.seq)) this.records.push(record);
    }
    this.records.sort((a, b) => a.seq - b.seq);
    if (this.records.length > MAX_RECORDS) this.records.splice(0, this.records.length - MAX_RECORDS);
  }

  /** Fills the top bar's alarm area; while the data is stale it is marked and nothing blinks. */
  drawTopBar(area: AlarmArea, stale: boolean, painting: AlarmPainting): void {
    const summary = this.summary;
    const key = JSON.stringify([summary ?? null, stale, this.sound.sounding]);
    if (key === this.drawn) return;
    this.drawn = key;
    area.silence.hidden = !this.sound.sounding;
    const annunciating = summary?.active ?? [];
    const top = PRIORITIES.find((priority) => annunciating.some((alarm) => alarm.priority === priority));
    const document = area.count.ownerDocument;
    const label =
      annunciating.length === 0
        ? say("No alarms")
        : annunciating.length === 1
          ? say("1 alarm")
          : say("{count} alarms", { count: annunciating.length });
    area.count.replaceChildren(
      ...(top ? [markOf(document, painting, top)] : []),
      spanOf(document, "hmi-alarm-label", label),
    );
    flag(area.count, "data-priority", top);
    flag(area.count, "data-blink", !stale && (summary?.unacknowledged ?? 0) > 0 ? "true" : undefined);
    flag(area.count, "data-stale", stale ? "true" : undefined);
    const newest = summary?.newest;
    if (newest) {
      area.newest.replaceChildren(
        markOf(document, painting, newest.priority),
        spanOf(document, "hmi-alarm-message", translate(newest.message || newest.path)),
        spanOf(document, "hmi-alarm-time", painting.time(newest.since_ms)),
      );
      area.newest.title = newest.path;
    } else {
      area.newest.replaceChildren();
      area.newest.removeAttribute("title");
    }
    flag(area.newest, "data-priority", newest?.priority);
    flag(area.newest, "data-stale", stale && newest ? "true" : undefined);
    const hidden = (
      [
        [summary?.shelved.length ?? 0, "{count} shelved"],
        [summary?.suppressed.length ?? 0, "{count} suppressed"],
        [summary?.out_of_service.length ?? 0, "{count} out of service"],
      ] as const
    )
      .filter(([count]) => count > 0)
      .map(([count, words]) => say(words, { count }));
    area.hidden.textContent = hidden.join(" · ");
  }
}

function markOf(document: Document, painting: AlarmPainting, priority: string): HTMLElement {
  const mark = document.createElement("span");
  mark.className = "hmi-alarm-mark";
  // The drawing module's own markup, for a priority it normalises; no program text goes in.
  mark.innerHTML = painting.mark(priority);
  return mark;
}

function spanOf(document: Document, className: string, content: string): HTMLElement {
  const span = document.createElement("span");
  span.className = className;
  span.textContent = content;
  return span;
}

function flag(element: Element, name: string, value: string | undefined): void {
  if (value === undefined) element.removeAttribute(name);
  else element.setAttribute(name, value);
}
