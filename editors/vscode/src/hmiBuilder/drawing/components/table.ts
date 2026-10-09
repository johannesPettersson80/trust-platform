// Table: a structure, an array, an array of structures or a two-dimensional array (§5.7).

import { fitText, fitWidth, formatValue } from "../format";
import type { DrawFrame, HmiObjectModel, StructuredValue } from "../model";
import { card, hatch, stateAttributes, stateChipRow, token } from "../parts";
import { el, SvgNode, text } from "../svg";
import { liveValueOf } from "../values";
import { say } from "../speech";
import { optionWords } from "./choices";

const PAD = 16;
const ROW = 30;
const CHAR = 7.6;

type Structure = { [field: string]: StructuredValue };

/** A cell: a flag drawn as a mark, a text read from the left, or a value read from the right. */
type Cell = { kind: "flag"; on: boolean } | { kind: "text" | "value"; content: string };

interface Row {
  label: string;
  cells: Cell[];
}

interface Grid {
  /** Column titles; `label` heads the index column. */
  header?: { label?: string; cells: string[] };
  rows: Row[];
  /** Rows at the end of an array of structures that hold nothing, left out and counted. */
  empty: number;
}

function isStructure(value: StructuredValue | undefined): value is Structure {
  return value !== null && value !== undefined && typeof value === "object" && !Array.isArray(value);
}

/** A cell: a flag as a mark, a text as written, a single value as the object formats it, and
 * anything nested by its size. */
function cell(value: StructuredValue | undefined, object: HmiObjectModel): Cell {
  if (value === undefined) return { kind: "value", content: "" };
  if (typeof value === "boolean") return { kind: "flag", on: value };
  if (Array.isArray(value)) return { kind: "value", content: `[${value.length}]` };
  if (isStructure(value)) return { kind: "value", content: `{${Object.keys(value).length}}` };
  const content = formatValue(value, object);
  return { kind: typeof value === "string" && content === value ? "text" : "value", content };
}

/** Holds nothing: an empty text, FALSE or 0, or a structure or array of nothing else. */
function holdsNothing(value: StructuredValue | undefined): boolean {
  if (value === "" || value === false || value === 0) return true;
  if (Array.isArray(value)) return value.every(holdsNothing);
  return isStructure(value) && Object.values(value).every(holdsNothing);
}

function gridOf(value: StructuredValue, lowerBounds: number[] | undefined, object: HmiObjectModel): Grid {
  if (isStructure(value)) {
    return {
      rows: Object.entries(value).map(([field, item]) => ({ label: optionWords(field), cells: [cell(item, object)] })),
      empty: 0,
    };
  }
  if (!Array.isArray(value)) return { rows: [{ label: "", cells: [cell(value, object)] }], empty: 0 };
  const index = (position: number, dimension: number): string => String((lowerBounds?.[dimension] ?? 0) + position);
  if (value.length > 0 && value.every(isStructure)) {
    const fields = [...new Set(value.flatMap((item) => Object.keys(item)))];
    let used = value.length;
    while (used > 0 && holdsNothing(value[used - 1])) used -= 1;
    return {
      header: { label: "#", cells: fields.map(optionWords) },
      rows: value.slice(0, used).map((item, position) => ({
        label: index(position, 0),
        cells: fields.map((field) => cell(item[field], object)),
      })),
      empty: value.length - used,
    };
  }
  if (value.length > 0 && value.every((item): item is StructuredValue[] => Array.isArray(item))) {
    const columns = value.reduce((most, row) => Math.max(most, row.length), 0);
    const positions = Array.from({ length: columns }, (_, column) => column);
    return {
      header: { cells: positions.map((column) => index(column, 1)) },
      rows: value.map((row, position) => ({
        label: index(position, 0),
        cells: positions.map((column) => cell(row[column], object)),
      })),
      empty: 0,
    };
  }
  return {
    rows: value.map((item, position) => ({ label: index(position, 0), cells: [cell(item, object)] })),
    empty: 0,
  };
}

/** Text cut to a cell's width. */
function clip(content: string, width: number): string {
  return fitText(content, width, CHAR);
}

/** A flag: a lit mark while TRUE, a ring while FALSE. */
function flagMark(on: boolean, x: number, y: number): SvgNode {
  return el("circle", {
    "data-part": "flag",
    "data-on": String(on),
    cx: x,
    cy: y,
    r: 6,
    fill: on ? token("accent") : "none",
    stroke: on ? token("accent") : token("muted"),
    "stroke-width": 1.5,
  });
}

/** What a column holds: texts if any cell is a text, flags if every cell is a flag, else values. */
function kindOf(rows: Row[], column: number): Cell["kind"] {
  const cells = rows.map((row) => row.cells[column]).filter((item): item is Cell => item !== undefined);
  if (cells.some((item) => item.kind === "text")) return "text";
  return cells.length > 0 && cells.every((item) => item.kind === "flag") ? "flag" : "value";
}

export function drawTable(object: HmiObjectModel, frame: DrawFrame): SvgNode {
  const [width, height] = object.size;
  let live = liveValueOf(object, "value", frame);
  if (object.bind["row-count"] && live && Array.isArray(live.value)) {
    const count = liveValueOf(object, "row-count", frame);
    const number =
      typeof count?.value === "number"
        ? count.value
        : typeof count?.value === "string" && /^\d+$/.test(count.value)
          ? Number(count.value)
          : NaN;
    live =
      count?.state === "confirmed" && Number.isInteger(number) && number >= 0 && number <= live.value.length
        ? { ...live, value: live.value.slice(0, number) }
        : { ...live, state: "unknown", value: null };
  }
  const children: SvgNode[] = [card(width, height, live)];
  const row = stateChipRow(object, live, width);
  const chips = row.nodes;
  let top = PAD;
  if (object.label) {
    // The title ends before the state chips beside it, cut with "…" when it does not fit.
    const room = Math.min(width - 2 * PAD, row.left - 8 - PAD);
    children.push(
      text(fitWidth(object.label, room, 15, 600), {
        x: PAD,
        y: top + 14,
        fill: token("text"),
        "font-size": 15,
        "font-weight": 600,
      }),
    );
    top += 28;
  } else if (chips.length > 0) {
    top = 42;
  }
  children.push(...chips);
  const group = (nodes: SvgNode[]): SvgNode =>
    el("g", { class: "hmi-object hmi-table", ...stateAttributes(live) }, nodes);
  const centred = { x: width / 2, y: (top + height) / 2 + 8, "text-anchor": "middle" };

  if (!live || live.state === "unknown" || live.value === null) {
    if (live) children.push(hatch(width, height));
    children.push(text("?", { ...centred, fill: token("unknown"), "font-size": 28, "font-weight": 600 }));
    return group(children);
  }
  if (Array.isArray(live.value) && live.value.length === 0) {
    children.push(text(say("Empty"), { ...centred, fill: token("muted"), "font-size": 15, "font-weight": 500 }));
    return group(children);
  }

  const grid = gridOf(live.value, live.lowerBounds, object);
  const columns = grid.rows.reduce((most, row) => Math.max(most, row.cells.length), grid.header?.cells.length ?? 0);
  const longestLabel = grid.rows.reduce(
    (most, row) => Math.max(most, row.label.length),
    grid.header?.label?.length ?? 0,
  );
  const labelWidth = Math.min(width * 0.45, Math.max(24, longestLabel * CHAR + 16));
  const cellsLeft = PAD + labelWidth;
  const room = width - PAD - cellsLeft;
  // With column titles, each column takes the room its longest title or cell needs, so a column
  // of texts gets the width that its flags and numbers beside it do not need.
  const kinds = Array.from({ length: columns }, (_, column) => (grid.header ? kindOf(grid.rows, column) : "value"));
  const weights = Array.from({ length: columns }, (_, column) =>
    grid.header
      ? grid.rows.reduce(
          (most, row) => {
            const item = row.cells[column];
            return Math.max(most, item === undefined ? 0 : item.kind === "flag" ? 2 : item.content.length);
          },
          Math.max(2, (grid.header.cells[column] ?? "").length),
        )
      : 1,
  );
  const total = weights.reduce((sum, weight) => sum + weight, 0) || 1;
  const widths = weights.map((weight) => (room * weight) / total);
  const lefts = widths.map((_, column) => cellsLeft + widths.slice(0, column).reduce((sum, part) => sum + part, 0));
  const number = { "font-size": 15, "font-weight": 600, "text-anchor": "end", "font-variant-numeric": "tabular-nums" };
  const separator = (y: number): SvgNode =>
    el("line", { x1: PAD, y1: y, x2: width - PAD, y2: y, stroke: token("line"), "stroke-width": 1 });
  /** Where a column's content stands: texts from the left, flags in the middle, values from the right. */
  const anchor = (column: number): Record<string, string | number> => {
    const left = lefts[column] ?? cellsLeft;
    const span = widths[column] ?? room;
    if (kinds[column] === "text") return { x: left + 8 };
    if (kinds[column] === "flag") return { x: left + span / 2, "text-anchor": "middle" };
    return { x: left + span, "text-anchor": "end" };
  };

  if (grid.header) {
    const baseline = top + 19;
    const titles: SvgNode[] = [];
    if (grid.header.label) {
      titles.push(
        text(grid.header.label, { x: PAD, y: baseline, fill: token("muted"), "font-size": 14, "font-weight": 600 }),
      );
    }
    grid.header.cells.forEach((title, column) => {
      titles.push(
        text(clip(title, widths[column] - 8), {
          ...anchor(column),
          y: baseline,
          fill: token("muted"),
          "font-size": 14,
          "font-weight": 600,
        }),
      );
    });
    children.push(
      el("g", { "data-part": "header" }, [
        ...titles,
        el("line", {
          x1: PAD,
          y1: top + ROW,
          x2: width - PAD,
          y2: top + ROW,
          stroke: token("border"),
          "stroke-width": 1,
        }),
      ]),
    );
    top += ROW;
  }

  const fit = Math.max(0, Math.floor((height - PAD - top) / ROW));
  const notes = grid.empty > 0 ? 1 : 0;
  const space = Math.max(0, fit - notes);
  const overflow = grid.rows.length > space;
  const shown = overflow ? Math.max(0, space - 1) : grid.rows.length;
  grid.rows.slice(0, shown).forEach((row, position) => {
    const rowTop = top + position * ROW;
    const baseline = rowTop + 20;
    const nodes: SvgNode[] = [];
    if (row.label !== "") {
      nodes.push(
        text(clip(row.label, labelWidth - 8), {
          x: PAD,
          y: baseline,
          fill: token("muted"),
          "font-size": 14,
          "font-weight": 500,
        }),
      );
    }
    row.cells.forEach((item, column) => {
      const at = anchor(column);
      if (item.kind === "flag") {
        nodes.push(
          flagMark(
            item.on,
            grid.header ? Number(at.x) : (lefts[column] ?? cellsLeft) + (widths[column] ?? room) - 8,
            rowTop + ROW / 2,
          ),
        );
      } else if (item.content !== "") {
        const style = item.kind === "text" ? { "font-size": 15, "font-weight": 500 } : number;
        nodes.push(
          text(clip(item.content, (widths[column] ?? room) - 8), { ...style, ...at, y: baseline, fill: token("text") }),
        );
      }
    });
    if (position < shown - 1 || overflow || notes > 0) nodes.push(separator(rowTop + ROW));
    children.push(el("g", { "data-part": "row", "data-y": rowTop }, nodes));
  });
  const note = (part: string, words: string, line: number): void => {
    const rowTop = top + line * ROW;
    children.push(
      el("g", { "data-part": part, "data-y": rowTop }, [
        text(words, { x: PAD, y: rowTop + 20, fill: token("muted"), "font-size": 14, "font-weight": 500 }),
      ]),
    );
  };
  if (overflow) note("more", say("+{count} more", { count: grid.rows.length - shown }), shown);
  if (grid.empty > 0) note("empty", say("Empty rows: {count}", { count: grid.empty }), shown + (overflow ? 1 : 0));
  return group(children);
}
