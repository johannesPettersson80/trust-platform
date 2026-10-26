// The alarm list (specification 39, §5.7 item 4, §5.9 item 2, §13.6): tabs for the active, shelved,
// suppressed and out-of-service alarms with their counts; a filter row with a search and quick
// filters, saying how many rows it shows; a row per alarm in the runtime's order, or sorted by a
// column's heading; columns for the priority, time, tag, message and state, the narrowest list
// keeping the priority and the message; an Acknowledge button on unacknowledged rows, and pages
// for rows that do not fit. Without an alarm source it says only where the alarms show (§11,
// item 6).

import { alarmPriority, blinking, priorityMark, priorityName, rowStateName } from "../alarms";
import { formatPath } from "../valueFormats";
import { fitWidth, textWidth, timeOfDay } from "../format";
import type { AlarmColumn, AlarmFilter, AlarmRowModel, AlarmTab, DrawFrame, HmiObjectModel, ListView } from "../model";
import { token } from "../parts";
import { el, SvgNode, text } from "../svg";
import { pager, PAGER_WIDTH } from "./listParts";
import { say, translate } from "../speech";
import type { Word } from "../words";

const TABS: Array<{ tab: AlarmTab; label: Word; empty: Word }> = [
  { tab: "active", label: "Active", empty: "No active alarms" },
  { tab: "shelved", label: "Shelved", empty: "No shelved alarms" },
  { tab: "suppressed", label: "Suppressed", empty: "No suppressed alarms" },
  { tab: "out_of_service", label: "Out of service", empty: "No alarms out of service" },
];

const PAD = 16;
/** The tab row. */
const HEADER = 52;
/** The filter row, under the tabs, and its chips' height. */
const FILTERS = 36;
const CHIP = 28;
/** The column headings' row, under the filters. */
const HEADINGS = 28;
/** Where the column headings start. */
const HEADINGS_TOP = HEADER + FILTERS;
/** Where the rows start. */
const ROWS_TOP = HEADINGS_TOP + HEADINGS;
/** The columns in their order and the room each takes; the message takes what is left and the
 * state as much as its longest text needs. */
const COLUMNS: Array<{ column: AlarmColumn; label: Word; width: number }> = [
  { column: "priority", label: "Priority", width: 96 },
  { column: "time", label: "Time", width: 84 },
  { column: "tag", label: "Tag", width: 112 },
  { column: "message", label: "Message", width: 160 },
  { column: "value", label: "Value", width: 152 },
  { column: "state", label: "State", width: 160 },
];
/** What a narrow list leaves out, first to last. */
const LEFT_OUT: AlarmColumn[] = ["value", "tag", "time", "state"];
const PRIORITY_RANK: Record<string, number> = { high: 0, medium: 1, low: 2 };
/** A row of cells. */
const ROW = 50;
/** The pager's row, when the tab row has no room for it. */
const FOOTER = 48;
const BUTTON = { width: 128, height: 40 };
/** The distance between the lines of the sentence a list without an alarm source says. */
const LINE = 22;
/** Average character widths of 15 px semibold and 14 px text. */

export function drawAlarmList(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  const alarms = frame.alarms;
  if (alarms?.unavailable !== undefined) return drawUnavailable(width, height, alarms.unavailable);
  const view = alarms?.views?.[object.id] ?? {};
  const tab = view.tab ?? "active";
  const stale = alarms?.stale === true;
  const listed = (name: AlarmTab): AlarmRowModel[] => alarms?.summary?.[name] ?? [];
  const all = listed(tab);
  const rows = sorted(filteredRows(all, view.filter), tab, view.sort);
  const stateWidth = Math.min(
    280,
    Math.max(160, Math.ceil(Math.max(0, ...rows.map((row) => textWidth(rowStateName(row, tab), 14)))) + 16),
  );
  // The value column only where a row has a limit alarm's reading (§5.9 item 2).
  const valueWidth = rows.some((row) => row.raised)
    ? Math.max(152, ...rows.map((row) => Math.ceil(textWidth(valueText(row, frame.formats), 14, 600)) + 16))
    : 0;
  const columns = placed(width, tab === "active", stateWidth, valueWidth);
  const children: SvgNode[] = [card(width, height, stale)];

  let x = PAD;
  for (const { tab: name, label } of TABS) {
    const content = `${say(label)} ${listed(name).length}`;
    const tabWidth = Math.round(textWidth(content, 14, 700) + 20);
    const selected = name === tab;
    children.push(
      el(
        "g",
        {
          "data-part": "alarm-tab",
          "data-alarm-tab": name,
          "data-selected": selected ? "true" : undefined,
          cursor: "pointer",
        },
        [
          el("rect", {
            x,
            y: 8,
            width: tabWidth,
            height: 36,
            rx: 18,
            fill: selected ? token("raised") : "transparent",
            stroke: selected ? token("accent") : "none",
          }),
          text(content, {
            x: x + tabWidth / 2,
            y: 31,
            fill: selected ? token("accent") : token("muted"),
            "font-size": 14,
            "font-weight": selected ? 700 : 550,
            "text-anchor": "middle",
          }),
        ],
      ),
    );
    x += tabWidth + 6;
  }

  // The pager sits in the tab row when there is room, otherwise in a row of its own at the foot.
  const headerFits = x + PAGER_WIDTH <= width - PAD;
  const room = (footer: boolean): number => Math.max(1, Math.floor((height - ROWS_TOP - (footer ? FOOTER : 8)) / ROW));
  const paged = rows.length > room(false);
  const capacity = paged && !headerFits ? room(true) : room(false);
  const lastPage = Math.max(0, Math.floor((rows.length - 1) / capacity) * capacity);
  const first = Math.min(Math.max(0, view.first ?? 0), lastPage);
  const shown = rows.slice(first, first + capacity);
  if (paged) {
    const top = headerFits ? 8 : height - FOOTER + 4;
    children.push(pager("alarm", first, shown.length, rows.length, width - PAD, top));
  }

  children.push(...filterRow(view.filter, rows.length, all.length, width), ...headings(columns, view.sort));
  if (shown.length === 0) {
    const empty = say(TABS.find((entry) => entry.tab === tab)?.empty ?? "No alarms");
    const top = ROWS_TOP;
    children.push(
      text(empty, {
        x: width / 2,
        y: top + (height - top) / 2,
        fill: token("muted"),
        "font-size": 15,
        "font-weight": 550,
        "text-anchor": "middle",
      }),
    );
  }
  shown.forEach((row, index) => {
    children.push(drawRow(row, tab, ROWS_TOP + index * ROW, width, stale, frame.nowMs ?? 0, columns, frame.formats));
  });

  return el("g", { class: "hmi-object hmi-alarm-list", "data-stale": stale ? "true" : undefined }, children);
}

/** The list's card, its edge dashed while the data is stale (§13.6, item 8). */
function card(width: number, height: number, stale: boolean): SvgNode {
  return el("rect", {
    "data-part": "card",
    x: 0.5,
    y: 0.5,
    width: Math.max(0, width - 1),
    height: Math.max(0, height - 1),
    rx: 14,
    fill: token("surface"),
    stroke: stale ? token("stale") : token("border"),
    "stroke-dasharray": stale ? "6 4" : undefined,
  });
}

/**
 * A list without an alarm source (§11, item 6): its card and the sentence saying where the alarms
 * show, broken between words into lines that fit, with nothing that reads as data.
 */
function drawUnavailable(width: number, height: number, sentence: string): SvgNode {
  const room = Math.max(0, width - 2 * PAD);
  const lines: string[] = [];
  for (const word of sentence.split(" ")) {
    const last = lines.length - 1;
    if (last >= 0 && textWidth(`${lines[last]} ${word}`, 15, 550) <= room) lines[last] += ` ${word}`;
    else lines.push(word);
  }
  const top = height / 2 - ((lines.length - 1) * LINE) / 2 + 5;
  return el("g", { class: "hmi-object hmi-alarm-list" }, [
    card(width, height, false),
    el(
      "g",
      { "data-part": "alarm-unavailable" },
      lines.map((line, index) =>
        text(fitWidth(line, room, 15, 550), {
          x: width / 2,
          y: top + index * LINE,
          fill: token("muted"),
          "font-size": 15,
          "font-weight": 550,
          "text-anchor": "middle",
        }),
      ),
    ),
  ]);
}

/** A column where it is drawn. */
interface Placed {
  column: AlarmColumn;
  label: Word;
  x: number;
  width: number;
}

/** The columns a list of `width` has room for, left to right; the message takes what is left. */
function placed(width: number, acknowledge: boolean, stateWidth: number, valueWidth: number): Placed[] {
  const button = acknowledge ? BUTTON.width + 12 : 0;
  const needed = (columns: typeof COLUMNS): number =>
    2 * PAD + button + columns.reduce((sum, entry) => sum + entry.width, 0);
  let columns = COLUMNS.filter((entry) => valueWidth > 0 || entry.column !== "value").map((entry) =>
    entry.column === "state"
      ? { ...entry, width: stateWidth }
      : entry.column === "value"
        ? { ...entry, width: valueWidth }
        : entry,
  );
  for (const column of LEFT_OUT) {
    if (needed(columns) <= width) break;
    columns = columns.filter((entry) => entry.column !== column);
  }
  const spare = Math.max(0, width - needed(columns));
  let x = PAD;
  return columns.map((entry) => {
    const room = entry.column === "message" ? entry.width + spare : entry.width;
    const at = { column: entry.column, label: entry.label, x, width: room };
    x += room;
    return at;
  });
}

/** The headings over the columns; the sorted one with its direction. */
function headings(columns: Placed[], sort: ListView["sort"]): SvgNode[] {
  return columns.map(({ column, label, x, width }) => {
    const sorted = sort?.column === column ? (sort.reversed ? "reversed" : "natural") : undefined;
    const words = say(label);
    const right = x + textWidth(words, 14, 600) + 6;
    const arrow =
      sorted === undefined
        ? []
        : [
            el("path", {
              d:
                sorted === "natural"
                  ? `M${right} ${HEADINGS_TOP + 12} l8 0 l-4 5 z`
                  : `M${right} ${HEADINGS_TOP + 17} l8 0 l-4 -5 z`,
              fill: token("accent"),
            }),
          ];
    return el(
      "g",
      { "data-part": "alarm-heading", "data-alarm-sort": column, "data-sorted": sorted, cursor: "pointer" },
      [
        el("rect", { x, y: HEADINGS_TOP, width, height: HEADINGS, fill: "transparent" }),
        text(words, {
          x,
          y: HEADINGS_TOP + 19,
          fill: sorted ? token("accent") : token("muted"),
          "font-size": 14,
          "font-weight": 600,
        }),
        ...arrow,
      ],
    );
  });
}

/** Whether a filter keeps fewer than every row. */
function filtering(filter: AlarmFilter | undefined): boolean {
  return Boolean(
    filter && (filter.text?.trim() || filter.priorities?.length || filter.unacknowledged || filter.equipment),
  );
}

/** The rows a filter keeps, in their order. */
function filteredRows(rows: AlarmRowModel[], filter: AlarmFilter | undefined): AlarmRowModel[] {
  if (!filtering(filter)) return rows;
  const wanted = filter?.text?.trim().toLowerCase();
  return rows.filter(
    (row) =>
      (!filter?.priorities?.length || filter.priorities.includes(alarmPriority(row.priority))) &&
      (!filter?.unacknowledged || row.unacknowledged) &&
      (!filter?.equipment || row.equipment === filter.equipment) &&
      (!wanted ||
        [translate(row.message || row.path), tagOf(row), row.path].some((field) =>
          field.toLowerCase().includes(wanted),
        )),
  );
}

/** The search, the quick filters and, while they keep fewer rows, how many they show of how many. */
function filterRow(filter: AlarmFilter | undefined, shown: number, total: number, width: number): SvgNode[] {
  const text_ = filter?.text?.trim();
  const priorities = filter?.priorities ?? [];
  const chips: Array<{ kind: string; value?: string; label: string; selected: boolean }> = [
    { kind: "text", label: text_ ? `“${text_}”` : say("Search"), selected: Boolean(text_) },
    ...(["high", "medium", "low"] as const).map((priority) => ({
      kind: "priority",
      value: priority,
      label: priorityName(priority),
      selected: priorities.includes(priority),
    })),
    { kind: "unacknowledged", label: say("Unacknowledged"), selected: Boolean(filter?.unacknowledged) },
    {
      kind: "equipment",
      label: filter?.equipment ? (filter.equipment.split(".").pop() ?? filter.equipment) : say("Equipment"),
      selected: Boolean(filter?.equipment),
    },
    ...(filtering(filter) ? [{ kind: "clear", label: say("Clear"), selected: false }] : []),
  ];
  const nodes: SvgNode[] = [];
  const count = filtering(filter) ? say("{count} of {total}", { count: shown, total }) : "";
  const end = width - PAD - (count ? textWidth(count, 14) + 12 : 0);
  let x = PAD;
  for (const chip of chips) {
    const chipWidth = Math.round(textWidth(chip.label, 14, 600) + 24);
    if (x + chipWidth > end) break;
    nodes.push(
      el(
        "g",
        {
          "data-part": "alarm-filter",
          "data-alarm-filter": chip.kind,
          "data-value": chip.value,
          "data-selected": chip.selected ? "true" : undefined,
          cursor: "pointer",
        },
        [
          el("rect", {
            x,
            y: HEADER + 2,
            width: chipWidth,
            height: CHIP,
            rx: CHIP / 2,
            fill: chip.selected ? token("raised") : "transparent",
            stroke: chip.selected ? token("accent") : token("border"),
          }),
          text(chip.label, {
            x: x + chipWidth / 2,
            y: HEADER + 21,
            fill: chip.selected ? token("accent") : token("muted"),
            "font-size": 14,
            "font-weight": 600,
            "text-anchor": "middle",
          }),
        ],
      ),
    );
    x += chipWidth + 6;
  }
  if (count)
    nodes.push(
      text(count, { x: width - PAD, y: HEADER + 21, fill: token("muted"), "font-size": 14, "text-anchor": "end" }),
    );
  return nodes;
}

/** A limit alarm's value against its limit, with the same decimals, at most three: "91.5 > 90.0". Two different numbers
 *  that those decimals show alike get more decimals, up to six, and the sign compares the two numbers. */
function valueText(row: AlarmRowModel, formats?: DrawFrame["formats"]): string {
  if (!row.raised) return "";
  const { value, limit, source } = row.raised;
  const format = source ? formats?.[formatPath(source)] : undefined;
  const unit = row.raised.unit ?? format?.unit;
  const decimals = (number: number): number => (String(Number(number.toFixed(3))).split(".")[1] ?? "").length;
  let places = format?.decimals ?? Math.max(decimals(value), decimals(limit));
  while (places < 6 && value !== limit && value.toFixed(places) === limit.toFixed(places)) places += 1;
  const sign = value > limit ? ">" : value < limit ? "<" : "=";
  return `${value.toFixed(places)} ${sign} ${limit.toFixed(places)}${unit ? ` ${unit}` : ""}`;
}

/** The tag of an alarm's equipment: the last name of the instance that holds it (§13.6 item 9). */
function tagOf(row: AlarmRowModel): string {
  return row.equipment?.split(".").pop() ?? "";
}

/** The rows in the runtime's order, or sorted by a column in its own order or reversed. */
function sorted(rows: AlarmRowModel[], tab: AlarmTab, sort: ListView["sort"]): AlarmRowModel[] {
  if (!sort) return rows;
  const text = (row: AlarmRowModel): string => {
    if (sort.column === "tag") return tagOf(row);
    if (sort.column === "state") return rowStateName(row, tab);
    return translate(row.message || row.path);
  };
  const compare = (a: AlarmRowModel, b: AlarmRowModel): number => {
    if (sort.column === "priority")
      return (
        PRIORITY_RANK[alarmPriority(a.priority)] - PRIORITY_RANK[alarmPriority(b.priority)] || b.since_ms - a.since_ms
      );
    if (sort.column === "time") return b.since_ms - a.since_ms;
    // The highest value first; rows without one last.
    if (sort.column === "value") return (b.raised?.value ?? -Infinity) - (a.raised?.value ?? -Infinity);
    return text(a).localeCompare(text(b));
  };
  return [...rows].sort((a, b) => (sort.reversed ? -compare(a, b) : compare(a, b)));
}

function drawRow(
  row: AlarmRowModel,
  tab: AlarmTab,
  top: number,
  width: number,
  stale: boolean,
  nowMs: number,
  columns: Placed[],
  formats?: DrawFrame["formats"],
): SvgNode {
  const priority = alarmPriority(row.priority);
  // The runtime says whether the alarm's state allows this session to acknowledge it (§5.7 item 4).
  const acknowledge = tab === "active" && (row.actions?.includes("acknowledge") ?? false);
  const mark = priorityMark(priority, PAD + 10, top + ROW / 2, 20);
  if (row.unacknowledged && !stale) blinking(mark, nowMs);
  const middle = top + ROW / 2 + 5;
  const cell = ({ column, x, width: room }: Placed): SvgNode => {
    const bold = row.unacknowledged ? 700 : 550;
    const [content, left, size, weight, colour] =
      column === "priority"
        ? [priorityName(priority), x + 28, 14, 600, token("text")]
        : column === "time"
          ? [timeOfDay(row.since_ms), x, 14, 400, token("muted")]
          : column === "tag"
            ? [tagOf(row), x, 14, 600, token("text")]
            : column === "message"
              ? [translate(row.message || row.path), x, 15, bold, token("text")]
              : column === "value"
                ? [valueText(row, formats), x, 14, 600, token("text")]
                : [rowStateName(row, tab), x, 14, 400, token("muted")];
    return el("g", { "data-part": "alarm-cell", "data-column": column }, [
      text(fitWidth(content, Math.max(0, room - (left - x) - 12), size, weight), {
        x: left,
        y: middle,
        fill: colour,
        "font-size": size,
        "font-weight": weight,
        "font-variant-numeric": column === "time" || column === "value" ? "tabular-nums" : undefined,
      }),
    ]);
  };
  const children: SvgNode[] = [
    // The whole row opens the alarm (§5.7, item 4).
    el("rect", { x: 1, y: top, width: Math.max(0, width - 2), height: ROW, fill: "transparent", cursor: "pointer" }),
    el("line", { x1: PAD, y1: top, x2: width - PAD, y2: top, stroke: token("line"), "stroke-width": 1 }),
    mark,
    ...columns.map(cell),
  ];
  if (acknowledge) {
    const left = width - PAD - BUTTON.width;
    const buttonTop = top + (ROW - BUTTON.height) / 2;
    children.push(
      el("g", { "data-alarm-action": "acknowledge", "data-alarm": row.path, cursor: "pointer" }, [
        el("rect", {
          x: left,
          y: buttonTop,
          width: BUTTON.width,
          height: BUTTON.height,
          rx: BUTTON.height / 2,
          fill: token("raised"),
          stroke: token("accent"),
        }),
        text(say("Acknowledge"), {
          x: left + BUTTON.width / 2,
          y: buttonTop + 25,
          fill: token("accent"),
          "font-size": 14,
          "font-weight": 650,
          "text-anchor": "middle",
        }),
      ]),
    );
  }
  return el("g", { "data-part": "alarm-row", "data-alarm": row.path }, children);
}
