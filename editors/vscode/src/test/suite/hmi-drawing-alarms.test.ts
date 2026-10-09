import * as assert from "assert";

import { drawPage } from "../../hmiBuilder/drawing";
import type {
  AlarmFrame,
  AlarmRecordModel,
  AlarmRowModel,
  AlarmSummaryModel,
  DrawFrame,
  HmiObjectModel,
} from "../../hmiBuilder/drawing/model";
import { findAll, renderToString, SvgNode, textsOf } from "../../hmiBuilder/drawing/svg";

// Specification 39, §5.7 item 4 and §13.6: the alarm list.

const T0 = Date.UTC(2026, 9, 2, 12, 0, 0);

function alarm(
  path: string,
  message: string,
  priority: string,
  unacknowledged: boolean,
  minute: number,
  extra: Partial<AlarmRowModel> = {},
): AlarmRowModel {
  return {
    path,
    message,
    priority,
    state: unacknowledged ? "ActiveUnacknowledged" : "ActiveAcknowledged",
    active: true,
    unacknowledged,
    shelved: false,
    out_of_service: false,
    suppressed: false,
    first_out: false,
    annunciating: true,
    raise_count: 1,
    since_ms: T0 + minute * 60_000,
    ...extra,
  };
}

const OVERFLOW = alarm("Tank.Overflow", "Tank overflow", "high", true, 30);
const LEVEL = alarm("Tank.High", "Tank level high", "medium", false, 10);
const SHELVED = alarm("Pump.Dry", "Pump running dry", "low", false, 5, { shelved: true, annunciating: false });
const AWAY = alarm("Mixer.Jam", "Mixer jammed", "high", false, 1, { out_of_service: true, annunciating: false });

function summary(lists: Partial<AlarmSummaryModel> = {}): AlarmSummaryModel {
  const active = lists.active ?? [];
  return {
    active,
    shelved: lists.shelved ?? [],
    suppressed: lists.suppressed ?? [],
    out_of_service: lists.out_of_service ?? [],
    unacknowledged: active.filter((row) => row.unacknowledged).length,
    annunciating: active.length,
    newest: active[0] ?? null,
  };
}

function list(): HmiObjectModel {
  return { id: "alarms", type: "alarm-list", at: [0, 0], size: [720, 376], rotation: 0, bind: {} };
}

/** A list wide enough for every column (§5.9 item 2). */
const WIDE: [number, number] = [1200, 376];

function draw(alarms?: AlarmFrame, size?: [number, number], formats?: DrawFrame["formats"]): SvgNode {
  const shown = size ? { ...list(), size } : list();
  const svg = drawPage(
    { title: "Alarms", object: [shown], pipe: [] },
    { size: [1280, 720], values: {}, alarms, formats },
  );
  const found = findAll(svg, (node) => node.attrs["data-object"] === "alarms");
  assert.strictEqual(found.length, 1, renderToString(svg));
  return found[0];
}

function parts(node: SvgNode, name: string): SvgNode[] {
  return findAll(node, (child) => child.attrs["data-part"] === name);
}

function rows(node: SvgNode): SvgNode[] {
  return parts(node, "alarm-row");
}

/** A row's cells by column. */
function cells(row: SvgNode): Record<string, string> {
  return Object.fromEntries(
    parts(row, "alarm-cell").map((cell) => [cell.attrs["data-column"], textsOf(cell).join("")]),
  );
}

suite("HMI drawing: alarm list", () => {
  test("the tabs count each list and Active is open first", () => {
    const node = draw({
      summary: summary({
        active: [OVERFLOW, LEVEL],
        shelved: [SHELVED],
        out_of_service: [AWAY, { ...AWAY, path: "Mixer.Other" }],
      }),
    });
    const tabs = parts(node, "alarm-tab");
    assert.deepStrictEqual(
      tabs.map((tab) => tab.attrs["data-alarm-tab"]),
      ["active", "shelved", "suppressed", "out_of_service"],
    );
    assert.deepStrictEqual(
      tabs.map((tab) => textsOf(tab).join("")),
      ["Active 2", "Shelved 1", "Suppressed 0", "Out of service 2"],
    );
    assert.deepStrictEqual(
      tabs.map((tab) => tab.attrs["data-selected"]),
      ["true", undefined, undefined, undefined],
    );
  });

  test("a row shows its priority, time, tag, message and state in columns under their headings, in the list's order", () => {
    // §5.9 item 2: the tag is the equipment's, the instance holding the alarm (§13.6 item 9).
    const overflow = { ...OVERFLOW, equipment: "Plant.T101" };
    const cleared = {
      ...alarm("Tank.Low", "Tank level low", "low", true, 20, { equipment: "Plant.T101" }),
      active: false,
      state: "ClearedUnacknowledged",
    };
    const node = draw({ summary: summary({ active: [overflow, cleared, LEVEL] }) }, WIDE);
    const headings = parts(node, "alarm-heading");
    assert.deepStrictEqual(
      headings.map((heading) => heading.attrs["data-alarm-sort"]),
      ["priority", "time", "tag", "message", "state"],
    );
    assert.deepStrictEqual(
      headings.map((heading) => textsOf(heading).join("")),
      ["Priority", "Time", "Tag", "Message", "State"],
    );
    const shown = rows(node);
    assert.deepStrictEqual(
      shown.map((row) => row.attrs["data-alarm"]),
      ["Tank.Overflow", "Tank.Low", "Tank.High"],
    );
    assert.strictEqual(parts(shown[0], "priority-mark")[0]?.attrs["data-priority"], "high");
    const first = cells(shown[0]);
    assert.match(first.time, /^\d\d:\d\d:\d\d$/);
    assert.deepStrictEqual(
      { ...first, time: "" },
      { priority: "High", time: "", tag: "T101", message: "Tank overflow", state: "Unacknowledged" },
    );
    assert.strictEqual(cells(shown[1]).state, "Cleared, unacknowledged");
    assert.strictEqual(cells(shown[2]).state, "Acknowledged");
    assert.strictEqual(cells(shown[2]).tag, "", "an alarm a program holds directly has no equipment");
  });

  test("a heading's column sorts the list, highest priority and newest first, else from A; reversed it shows so", () => {
    const pump = alarm("Pump.Dry", "Pump running dry", "low", false, 40);
    const list = summary({ active: [OVERFLOW, LEVEL, pump] });
    const order = (sort: { column: string; reversed?: boolean }) => {
      const node = draw({ summary: list, views: { alarms: { sort } } as AlarmFrame["views"] }, WIDE);
      const heading = parts(node, "alarm-heading").find(
        (candidate) => candidate.attrs["data-alarm-sort"] === sort.column,
      );
      return { rows: rows(node).map((row) => row.attrs["data-alarm"]), sorted: heading?.attrs["data-sorted"] };
    };
    assert.deepStrictEqual(order({ column: "time" }), {
      rows: ["Pump.Dry", "Tank.Overflow", "Tank.High"],
      sorted: "natural",
    });
    assert.deepStrictEqual(order({ column: "time", reversed: true }), {
      rows: ["Tank.High", "Tank.Overflow", "Pump.Dry"],
      sorted: "reversed",
    });
    assert.deepStrictEqual(order({ column: "priority" }).rows, ["Tank.Overflow", "Tank.High", "Pump.Dry"]);
    assert.deepStrictEqual(order({ column: "message" }).rows, ["Pump.Dry", "Tank.High", "Tank.Overflow"]);
  });

  test("the filter row searches and filters by priority, equipment and unacknowledged, and says how many it shows", () => {
    // §5.9 item 2: a search box and quick filters.
    const pump = alarm("Plant.P101.Dry", "Pump running dry", "low", false, 40, { equipment: "Plant.P101" });
    const valve = alarm("Plant.FV101.Stuck", "Valve stuck", "medium", true, 35, { equipment: "Plant.FV101" });
    const all = summary({ active: [OVERFLOW, valve, LEVEL, pump] });
    const filtered = (filter: object) =>
      draw({ summary: all, views: { alarms: { filter } } as AlarmFrame["views"] }, WIDE);
    const shown = (filter: object) => rows(filtered(filter)).map((row) => row.attrs["data-alarm"]);
    assert.deepStrictEqual(shown({ priorities: ["high", "medium"] }), [
      "Tank.Overflow",
      "Plant.FV101.Stuck",
      "Tank.High",
    ]);
    assert.deepStrictEqual(shown({ unacknowledged: true }), ["Tank.Overflow", "Plant.FV101.Stuck"]);
    assert.deepStrictEqual(shown({ equipment: "Plant.P101" }), ["Plant.P101.Dry"]);
    assert.deepStrictEqual(
      shown({ text: "TANK" }),
      ["Tank.Overflow", "Tank.High"],
      "the message, tag or path, in any case",
    );
    assert.deepStrictEqual(shown({ text: "p101" }), ["Plant.P101.Dry"]);
    const node = filtered({ unacknowledged: true, equipment: "Plant.FV101" });
    const chips = parts(node, "alarm-filter").map((chip) => [
      chip.attrs["data-alarm-filter"],
      chip.attrs["data-value"] ?? "",
      chip.attrs["data-selected"] ?? "",
    ]);
    assert.deepStrictEqual(chips, [
      ["text", "", ""],
      ["priority", "high", ""],
      ["priority", "medium", ""],
      ["priority", "low", ""],
      ["unacknowledged", "", "true"],
      ["equipment", "", "true"],
      ["clear", "", ""],
    ]);
    const texts = textsOf(node);
    assert.ok(texts.includes("1 of 4") && texts.includes("FV101"), texts.join("|"));
    assert.ok(
      !parts(draw({ summary: all }, WIDE), "alarm-filter").some((chip) => chip.attrs["data-alarm-filter"] === "clear"),
      "nothing to clear",
    );
  });

  test("a limit alarm shows its value against its limit from when it was raised, in a Value column", () => {
    // §5.9 item 2: the runtime keeps the reading of the read that raised it.
    const high = { ...LEVEL, raised: { value: 91.5, limit: 90, high: true } };
    const low = alarm("Tank.Low", "Tank level low", "low", true, 20, {
      raised: { value: 4.25, limit: 5, high: false },
    });
    const node = draw({ summary: summary({ active: [OVERFLOW, low, high] }) }, WIDE);
    assert.deepStrictEqual(
      parts(node, "alarm-heading").map((heading) => heading.attrs["data-alarm-sort"]),
      ["priority", "time", "tag", "message", "value", "state"],
    );
    const values = rows(node).map((row) => cells(row).value);
    assert.deepStrictEqual(values, ["", "4.25 < 5.00", "91.5 > 90.0"]);
    const narrow = parts(draw({ summary: summary({ active: [low] }) }, [720, 376]), "alarm-heading").map(
      (heading) => heading.attrs["data-alarm-sort"],
    );
    assert.deepStrictEqual(
      narrow,
      ["priority", "time", "message", "state"],
      "a narrow list leaves the value out first",
    );
  });

  test("alarm readings use the source precision and retain the captured engineering unit", () => {
    const high = { ...LEVEL, raised: { value: 91.5, limit: 90, high: true, source: "Main.TT.Value", unit: "°C" } };
    const node = draw({ summary: summary({ active: [high] }) }, WIDE, {
      "main.tt.value": { unit: "changed", decimals: 2 },
    });
    assert.strictEqual(
      cells(rows(node)[0]).value,
      "91.50 > 90.00 °C",
      "the historical unit wins over a later project unit",
    );
    const fallback = { ...LEVEL, raised: { value: 91.5, limit: 90, high: true, source: "Main.TT.Value" } };
    const withoutUnit = draw({ summary: summary({ active: [fallback] }) }, WIDE, {
      "main.tt.value": { unit: "m", decimals: 1 },
    });
    assert.strictEqual(cells(rows(withoutUnit)[0]).value, "91.5 > 90.0 m");
  });
  test("the value column makes room for its engineering unit instead of truncating the reading", () => {
    const row = {
      ...LEVEL,
      raised: { value: 100, limit: 90, high: true, source: "Main.Flow.Value", unit: "cubic metres per second" },
    };
    const node = draw({ summary: summary({ active: [row] }) }, [1500, 376], { "main.flow.value": { decimals: 2 } });
    assert.strictEqual(cells(rows(node)[0]).value, "100.00 > 90.00 cubic metres per second");
  });

  test("a narrow list leaves out the tag first, then the time, then the state", () => {
    const columns = (width: number) =>
      parts(draw({ summary: summary({ active: [OVERFLOW] }) }, [width, 376]), "alarm-heading").map(
        (heading) => heading.attrs["data-alarm-sort"],
      );
    assert.deepStrictEqual(columns(1200), ["priority", "time", "tag", "message", "state"]);
    assert.deepStrictEqual(columns(720), ["priority", "time", "message", "state"]);
    assert.deepStrictEqual(columns(640), ["priority", "message", "state"]);
    assert.deepStrictEqual(columns(480), ["priority", "message"]);
  });

  test("an unacknowledged row has an Acknowledge button and its mark blinks while the data is fresh", () => {
    const node = draw({ summary: summary({ active: [OVERFLOW, LEVEL] }) });
    assert.strictEqual(rows(node).length, 2, renderToString(node));
    const [first, second] = rows(node);
    const acknowledge = findAll(first, (child) => child.attrs["data-alarm-action"] === "acknowledge");
    assert.strictEqual(acknowledge.length, 1, renderToString(first));
    assert.strictEqual(acknowledge[0].attrs["data-alarm"], "Tank.Overflow");
    assert.ok(textsOf(acknowledge[0]).includes("Acknowledge"));
    assert.strictEqual(findAll(second, (child) => child.attrs["data-alarm-action"] !== undefined).length, 0);
    assert.strictEqual(parts(first, "priority-mark")[0].attrs["data-blink"], "true");
    assert.strictEqual(parts(second, "priority-mark")[0].attrs["data-blink"], undefined);
    const stale = draw({ summary: summary({ active: [OVERFLOW] }), stale: true });
    assert.strictEqual(
      parts(rows(stale)[0], "priority-mark")[0].attrs["data-blink"],
      undefined,
      "nothing blinks while stale",
    );
    assert.strictEqual(stale.attrs["data-stale"], "true");
  });

  test("rows that do not fit are shown in pages", () => {
    const many = Array.from({ length: 9 }, (_, index) =>
      alarm(`Line.A${index + 1}`, `Alarm ${index + 1}`, "low", false, index),
    );
    // Five rows fit under the tabs and the column headings of a 376 px list.
    const first = draw({ summary: summary({ active: many }) });
    assert.strictEqual(rows(first).length, 5);
    assert.ok(textsOf(parts(first, "alarm-pager")[0]).includes("1–5 of 9"), renderToString(first));
    const pages = parts(first, "alarm-page");
    assert.deepStrictEqual(
      pages.map((page) => [page.attrs["data-alarm-page"], page.attrs["data-disabled"]]),
      [
        ["previous", "true"],
        ["next", undefined],
      ],
    );
    const second = draw({ summary: summary({ active: many }), views: { alarms: { first: 5 } } });
    assert.deepStrictEqual(
      rows(second).map((row) => row.attrs["data-alarm"]),
      ["Line.A6", "Line.A7", "Line.A8", "Line.A9"],
    );
    assert.ok(textsOf(parts(second, "alarm-pager")[0]).includes("6–9 of 9"));
  });

  test("another tab lists its alarms, without Acknowledge", () => {
    const node = draw({
      summary: summary({ active: [OVERFLOW], shelved: [{ ...SHELVED, unacknowledged: true }] }),
      views: { alarms: { tab: "shelved" } },
    });
    assert.deepStrictEqual(
      rows(node).map((row) => row.attrs["data-alarm"]),
      ["Pump.Dry"],
    );
    assert.strictEqual(findAll(node, (child) => child.attrs["data-alarm-action"] !== undefined).length, 0);
    assert.strictEqual(
      parts(node, "alarm-tab").find((tab) => tab.attrs["data-selected"] === "true")?.attrs["data-alarm-tab"],
      "shelved",
    );
  });

  test("in the other tabs a row names that state and whether its condition is active", () => {
    const shelved = draw({ summary: summary({ shelved: [SHELVED] }), views: { alarms: { tab: "shelved" } } });
    assert.strictEqual(cells(rows(shelved)[0]).state, "Shelved, condition active", textsOf(shelved).join("|"));
    const away = draw({
      summary: summary({ out_of_service: [{ ...AWAY, active: false, state: "Normal" }] }),
      views: { alarms: { tab: "out_of_service" } },
    });
    assert.strictEqual(cells(rows(away)[0]).state, "Out of service, condition clear", textsOf(away).join("|"));
  });

  test("an empty tab says so, and the editor draws an empty list", () => {
    const editor = draw();
    assert.ok(textsOf(editor).includes("No active alarms"), textsOf(editor).join("|"));
    assert.deepStrictEqual(
      parts(editor, "alarm-tab").map((tab) => textsOf(tab).join("")),
      ["Active 0", "Shelved 0", "Suppressed 0", "Out of service 0"],
    );
    const away = draw({ summary: summary({ active: [OVERFLOW] }), views: { alarms: { tab: "out_of_service" } } });
    assert.ok(textsOf(away).includes("No alarms out of service"), textsOf(away).join("|"));
  });
});

// Specification 39, §5.7 item 5: the event log.

function record(seq: number, kind: string, extra: Partial<AlarmRecordModel> = {}): AlarmRecordModel {
  return {
    seq,
    time_ms: T0 + seq * 1000,
    path: "Tank.Overflow",
    kind,
    message: "Tank overflow",
    priority: "high",
    ...extra,
  };
}

function log(records: AlarmRecordModel[] | undefined, views?: AlarmFrame["views"]): SvgNode {
  const object: HmiObjectModel = {
    id: "events",
    type: "event-log",
    at: [0, 0],
    size: [720, 376],
    rotation: 0,
    bind: {},
  };
  const svg = drawPage(
    { title: "Events", object: [object], pipe: [] },
    { size: [1280, 720], values: {}, alarms: records ? { records, views } : undefined },
  );
  const found = findAll(svg, (node) => node.attrs["data-object"] === "events");
  assert.strictEqual(found.length, 1, renderToString(svg));
  return found[0];
}

function eventRows(node: SvgNode): SvgNode[] {
  return parts(node, "event-row");
}

suite("HMI drawing: event log", () => {
  test("records are shown newest first with what happened, the message, and the time, path and user", () => {
    const node = log([
      record(1, "raised"),
      record(2, "acknowledged", { user: "anna" }),
      record(3, "command", {
        path: "Tank.Trip",
        message: "Write true",
        priority: undefined,
        user: "anna",
        outcome: "confirmed",
      }),
    ]);
    const rows = eventRows(node);
    assert.deepStrictEqual(
      rows.map((row) => row.attrs["data-seq"]),
      ["3", "2", "1"],
    );
    assert.ok(textsOf(rows[0]).includes("Confirmed · Write true"), textsOf(rows[0]).join("|"));
    assert.ok(
      textsOf(rows[0]).some((line) => /^\d\d:\d\d:\d\d · Tank\.Trip · anna$/.test(line)),
      textsOf(rows[0]).join("|"),
    );
    assert.ok(textsOf(rows[1]).includes("Acknowledged · Tank overflow"), textsOf(rows[1]).join("|"));
    assert.ok(textsOf(rows[2]).includes("Raised · Tank overflow"), textsOf(rows[2]).join("|"));
    assert.ok(
      textsOf(rows[2]).some((line) => /^\d\d:\d\d:\d\d · Tank\.Overflow$/.test(line)),
      "no user for the program's own transition",
    );
  });

  test("an alarm's record carries its priority's mark and a command's record its outcome", () => {
    const node = log([
      record(1, "raised"),
      record(2, "command", {
        priority: undefined,
        outcome: "refused",
        message: "Take out of service",
        detail: "this command needs the Engineer role",
      }),
    ]);
    assert.strictEqual(eventRows(node).length, 2, renderToString(node));
    const [command, raised] = eventRows(node);
    assert.strictEqual(parts(raised, "priority-mark")[0]?.attrs["data-priority"], "high");
    assert.strictEqual(parts(command, "outcome-mark")[0]?.attrs["data-outcome"], "refused");
    assert.ok(
      textsOf(command).some((line) => line.endsWith("· this command needs the Engineer role")),
      textsOf(command).join("|"),
    );
  });

  test("a comment and a shelving show their detail", () => {
    const node = log([
      record(1, "shelved", { user: "anna", detail: "600 s" }),
      record(2, "comment", { user: "anna", detail: "Level gauge checked" }),
    ]);
    assert.strictEqual(eventRows(node).length, 2, renderToString(node));
    const [comment, shelved] = eventRows(node);
    assert.ok(
      textsOf(comment).some((line) => line.endsWith("· anna · “Level gauge checked”")),
      textsOf(comment).join("|"),
    );
    assert.ok(
      textsOf(shelved).some((line) => line.endsWith("· anna · 600 s")),
      textsOf(shelved).join("|"),
    );
  });

  test("rows that do not fit are shown in pages", () => {
    const records = Array.from({ length: 12 }, (_, index) => record(index + 1, "raised"));
    const first = log(records);
    assert.deepStrictEqual(
      eventRows(first).map((row) => row.attrs["data-seq"]),
      ["12", "11", "10", "9", "8", "7"],
    );
    assert.ok(textsOf(parts(first, "event-pager")[0]).includes("1–6 of 12"), renderToString(first));
    const next = parts(first, "event-page").find((page) => page.attrs["data-event-page"] === "next");
    assert.strictEqual(next?.attrs["data-page-first"], "6");
    const second = log(records, { events: { first: 6 } });
    assert.deepStrictEqual(
      eventRows(second).map((row) => row.attrs["data-seq"]),
      ["6", "5", "4", "3", "2", "1"],
    );
  });

  test("the editor draws an empty log", () => {
    assert.ok(textsOf(log(undefined)).includes("No events yet"));
  });
});

// Specification 39, §13.6 items 4 and 9: objects show the alarms of their equipment.

suite("HMI drawing: equipment alarms", () => {
  const FAILED = alarm("Tank.P1.FailedToStart", "Feed pump failed to start", "high", true, 3, { equipment: "Tank.P1" });
  const DRY = alarm("Tank.P1.Dry", "Feed pump running dry", "low", false, 4, { equipment: "Tank.P1" });
  const pump: HmiObjectModel = {
    id: "pump",
    type: "status",
    at: [0, 0],
    size: [240, 48],
    rotation: 0,
    bind: { state: { path: "Tank.P1.Running", direction: "read" } },
  };
  const level: HmiObjectModel = {
    id: "level",
    type: "value",
    at: [0, 80],
    size: [240, 96],
    rotation: 0,
    bind: { value: { path: "Tank.Level", direction: "read" } },
  };

  function drawn(alarms: AlarmFrame): { pump: SvgNode; level: SvgNode } {
    const svg = drawPage(
      { title: "Plant", object: [pump, level], pipe: [] },
      { size: [1280, 720], values: {}, alarms },
    );
    const of = (id: string): SvgNode => findAll(svg, (node) => node.attrs["data-object"] === id)[0];
    return { pump: of("pump"), level: of("level") };
  }

  test("an object bound below an equipment instance shows its highest priority alarm's mark and their number", () => {
    const { pump: node } = drawn({ summary: summary({ active: [DRY, FAILED] }) });
    const badge = parts(node, "equipment-alarm");
    assert.strictEqual(badge.length, 1, renderToString(node));
    assert.strictEqual(badge[0].attrs["data-alarm"], "Tank.P1.FailedToStart");
    assert.strictEqual(parts(badge[0], "priority-mark")[0]?.attrs["data-priority"], "high");
    assert.ok(textsOf(badge[0]).includes("2"), textsOf(badge[0]).join("|"));
    assert.strictEqual(
      parts(badge[0], "priority-mark")[0].attrs["data-blink"],
      "true",
      "one of them waits for acknowledgement",
    );
  });

  test("an object bound elsewhere shows none, and the mark does not blink when acknowledged or stale", () => {
    const fresh = drawn({ summary: summary({ active: [{ ...FAILED, unacknowledged: false }] }) });
    assert.strictEqual(parts(fresh.level, "equipment-alarm").length, 0, "Tank is a program, not equipment");
    assert.strictEqual(parts(fresh.pump, "equipment-alarm").length, 1, renderToString(fresh.pump));
    const mark = parts(parts(fresh.pump, "equipment-alarm")[0], "priority-mark")[0];
    assert.strictEqual(mark.attrs["data-blink"], undefined);
    assert.ok(!textsOf(parts(fresh.pump, "equipment-alarm")[0]).includes("1"), "no number for one alarm");
    const stale = drawn({ summary: summary({ active: [FAILED] }), stale: true });
    assert.strictEqual(
      parts(parts(stale.pump, "equipment-alarm")[0], "priority-mark")[0].attrs["data-blink"],
      undefined,
    );
  });

  test("shelved, suppressed and out-of-service alarms of the equipment show no mark", () => {
    const { pump: node } = drawn({
      summary: summary({ shelved: [{ ...FAILED, shelved: true, annunciating: false }] }),
    });
    assert.strictEqual(parts(node, "equipment-alarm").length, 0);
  });

  test("a faceplate is its equipment's own view: only its symbol carries the mark, not each of its controls (§8.7)", () => {
    const symbol: HmiObjectModel = {
      id: "symbol",
      type: "pump",
      at: [16, 16],
      size: [128, 108],
      rotation: 0,
      bind: { running: { path: "Tank.P1.Running", direction: "read" } },
    };
    const starts: HmiObjectModel = {
      id: "starts",
      type: "value",
      at: [16, 300],
      size: [184, 56],
      rotation: 0,
      bind: { value: { path: "Tank.P1.Starts", direction: "read" } },
    };
    const faceplate = {
      title: "P-101",
      size: [640, 400] as [number, number],
      fields: {},
      object: [symbol, pump, starts],
      pipe: [],
    };
    const svg = drawPage(faceplate, {
      size: [640, 400],
      values: {},
      alarms: { summary: summary({ active: [FAILED] }) },
    });
    const marked = findAll(svg, (node) => node.attrs["data-part"] === "equipment-alarm").map(() => "mark");
    const owners = findAll(
      svg,
      (node) => node.attrs["data-object"] !== undefined && parts(node, "equipment-alarm").length > 0,
    ).map((node) => node.attrs["data-object"]);
    assert.deepStrictEqual(owners, ["symbol"], `${marked.length} marks`);
  });
});

// Spec 39 §13.6: equipment annunciation must never cover the trend's actions.
suite("HMI drawing: trend alarm spacing", () => {
  test("single and counted alarms leave every trend action clear at narrow and wide sizes", () => {
    for (const width of [320, 720, 1024])
      for (const count of [1, 2, 123]) {
        const trend: HmiObjectModel = {
          id: "trend",
          type: "trend",
          at: [0, 0],
          size: [width, 240],
          rotation: 0,
          label: "Level",
          bind: { level: { path: "Tank.P1.Level", direction: "read" } },
        };
        const active = Array.from({ length: count }, (_, index) =>
          alarm(`Tank.P1.Alarm${index}`, "High", "high", true, index, { equipment: "Tank.P1" }),
        );
        const svg = drawPage(
          { title: "Trend", object: [trend], pipe: [] },
          { size: [width, 240], values: {}, alarms: { summary: summary({ active }) } },
        );
        const badge = parts(svg, "equipment-alarm")[0];
        assert.ok(badge);
        const left = Math.min(
          ...badge.children
            .filter((node) => node.tag === "rect" || node.tag === "circle")
            .map((node) => (node.tag === "rect" ? Number(node.attrs.x) : Number(node.attrs.cx) - Number(node.attrs.r))),
        );
        const actions = findAll(
          svg,
          (node) => node.attrs["data-trend-action"] !== undefined || node.attrs["data-trend-range"] !== undefined,
        );
        assert.ok(
          actions.some((node) => node.attrs["data-trend-action"] === "expand"),
          "expand remains available",
        );
        for (const action of actions) {
          const rect = action.children.find((node) => node.tag === "rect")!;
          assert.ok(
            Number(rect.attrs.x) + Number(rect.attrs.width) + 8 <= left,
            `alarm overlaps ${action.attrs["data-trend-action"] ?? "range"}: width=${width}, count=${count}`,
          );
        }
      }
  });
});
