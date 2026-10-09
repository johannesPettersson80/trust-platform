import * as assert from "assert";

import { drawPage } from "../../hmiBuilder/drawing";
import type { DrawFrame, HmiBindingModel, HmiObjectModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { findAll, renderToString, SvgNode, textsOf } from "../../hmiBuilder/drawing/svg";
import { tokensCss } from "../../hmiBuilder/drawing/tokens";
import { formatValue } from "../../hmiBuilder/drawing/format";

// Specification 39, §5.7: trend and table.

const NOW = 1_000_000_000;

function draw(object: HmiObjectModel, frame: Partial<DrawFrame> = {}): SvgNode {
  const svg = drawPage({ title: "Data", object: [object], pipe: [] }, { size: [1280, 720], values: {}, ...frame });
  const found = findAll(svg, (node) => node.attrs["data-object"] === object.id);
  assert.strictEqual(found.length, 1, renderToString(svg));
  return found[0];
}

function read(path: string, unit?: string): HmiBindingModel {
  return { path, direction: "read", ...(unit ? { unit } : {}) };
}

function trend(bind: Record<string, HmiBindingModel>, extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  return { id: "loop", type: "trend", at: [0, 0], size: [480, 240], rotation: 0, bind, ...extra };
}

function table(extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  return {
    id: "data",
    type: "table",
    at: [0, 0],
    size: [320, 400],
    rotation: 0,
    bind: { value: read("Line.Data") },
    ...extra,
  };
}

function part(node: SvgNode, name: string): SvgNode[] {
  return findAll(node, (child) => child.attrs["data-part"] === name);
}

suite("HMI drawing: independent trend scales", () => {
  test("compact trend axis labels stay separated without changing the scale", () => {
    const svg = draw(trend({ MV: read("Output") }, { label: "MV", size: [608, 140], min: 0, max: 100 }));
    const labels = findAll(svg, (node) => node.tag === "text" && node.attrs["data-axis-pen"] === "MV");
    const y = labels.map((node) => Number(node.attrs.y)).sort((a, b) => a - b);
    assert.ok(y.length >= 2, "the compact MV plot retains both scale ends");
    for (let i = 1; i < y.length; i++) assert.ok(y[i] - y[i - 1] >= 20, "axis labels need readable vertical spacing");
    assert.deepStrictEqual(labels.flatMap(textsOf), ["0", "100"]);
  });

  test("each automatic pen uses its own visible samples", () => {
    const object = trend({ Temperature: read("T"), Speed: read("S") });
    const svg = draw(object, {
      nowMs: NOW,
      values: { T: { value: 50, state: "confirmed" }, S: { value: 500, state: "confirmed" } },
      history: {
        T: [
          [NOW - 1000, 30],
          [NOW, 50],
        ],
        S: [
          [NOW - 1000, 300],
          [NOW, 500],
        ],
      },
    });
    const lines = findAll(svg, (node) => node.tag === "polyline" && !!node.attrs["data-pen"]);
    assert.strictEqual(lines.length, 2);
    assert.strictEqual(
      lines[0].attrs.points,
      lines[1].attrs.points,
      "equivalent changes on different units occupy the same height on independent scales",
    );
  });

  test("automatic pens of one unit share their scale, so a setpoint reads against its measurement", () => {
    const object = trend({ measurement: read("PV", "m"), setpoint: read("SP", "m"), output: read("OUT", "%") });
    const svg = draw(object, {
      nowMs: NOW,
      values: {
        PV: { value: 0, state: "confirmed" },
        SP: { value: 1.5, state: "confirmed" },
        OUT: { value: 40, state: "confirmed" },
      },
      history: {
        PV: [
          [NOW - 1000, 0],
          [NOW, 0],
        ],
        SP: [
          [NOW - 1000, 1.5],
          [NOW, 1.5],
        ],
        OUT: [
          [NOW - 1000, 40],
          [NOW, 40],
        ],
      },
    });
    const height = (pen: string): number => {
      const line = findAll(svg, (node) => node.tag === "polyline" && node.attrs["data-pen"] === pen)[0];
      return Number(String(line.attrs.points).split(" ")[0].split(",")[1]);
    };
    assert.ok(height("setpoint") < height("measurement"), "the setpoint 1.5 m is drawn above the measurement 0 m");
    assert.notStrictEqual(height("output"), height("setpoint"), "the output in % keeps its own scale");
    const labels = findAll(svg, (node) => node.tag === "text" && node.attrs["data-axis-pen"] === "measurement")
      .flatMap(textsOf)
      .map(Number);
    assert.ok(Math.min(...labels) <= 0 && Math.max(...labels) >= 1.5, "the axis spans both metres pens");
  });

  test("each fixed binding range overrides the object range", () => {
    const object = trend(
      {
        Temperature: { ...read("T"), range: [0, 100] } as HmiBindingModel,
        Speed: { ...read("S"), range: [0, 1000] } as HmiBindingModel,
      },
      { min: 0, max: 2000 },
    );
    const svg = draw(object, {
      nowMs: NOW,
      values: { T: { value: 50, state: "confirmed" }, S: { value: 500, state: "confirmed" } },
      history: {
        T: [
          [NOW - 1000, 30],
          [NOW, 50],
        ],
        S: [
          [NOW - 1000, 300],
          [NOW, 500],
        ],
      },
    });
    const lines = findAll(svg, (node) => node.tag === "polyline" && !!node.attrs["data-pen"]);
    assert.strictEqual(lines.length, 2);
    assert.strictEqual(
      lines[0].attrs.points,
      lines[1].attrs.points,
      "the two configured scales map the same proportions",
    );
  });
});

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

function plotOf(node: SvgNode): Box {
  const plot = part(node, "plot")[0];
  assert.ok(plot, renderToString(node));
  return {
    x: Number(plot.attrs.x),
    y: Number(plot.attrs.y),
    width: Number(plot.attrs.width),
    height: Number(plot.attrs.height),
  };
}

function pen(node: SvgNode, name: string): SvgNode {
  const found = findAll(node, (child) => child.tag === "polyline" && child.attrs["data-pen"] === name);
  assert.strictEqual(found.length, 1, `pen ${name}: ${renderToString(node)}`);
  return found[0];
}

function pointsOf(line: SvgNode): Array<[number, number]> {
  return line.attrs.points
    .trim()
    .split(/\s+/)
    .map((pair) => pair.split(",").map(Number) as [number, number]);
}

function near(actual: number, expected: number, message: string): void {
  assert.ok(Math.abs(actual - expected) < 0.6, `${message}: ${actual} is not ${expected}`);
}

function samples(...pairs: Array<[number, number]>): Array<[number, number]> {
  return pairs.map(([secondsAgo, value]) => [NOW - secondsAgo * 1000, value]);
}

suite("HMI drawing: trend", () => {
  test("every binding other than the limits is a pen, in name order, in the pen colours, dashed from the fifth", () => {
    const names = ["e", "c", "a", "d", "b"];
    const bind: Record<string, HmiBindingModel> = { "high-limit": read("Loop.High") };
    const history: Record<string, Array<[number, number]>> = {};
    for (const name of names) {
      bind[name] = read(`Loop.${name}`);
      history[`Loop.${name}`] = samples([60, 1], [10, 2]);
    }
    const node = draw(trend(bind), { history, nowMs: NOW });
    const traces = findAll(node, (child) => child.tag === "polyline" && child.attrs["data-pen"] !== undefined);
    assert.deepStrictEqual(
      traces.map((trace) => trace.attrs["data-pen"]),
      ["a", "b", "c", "d", "e"],
    );
    assert.deepStrictEqual(
      traces.map((trace) => trace.attrs.stroke),
      ["pen-1", "pen-2", "pen-3", "pen-4", "pen-1"].map((name) => `var(--hmi-${name})`),
    );
    assert.deepStrictEqual(
      traces.map((trace) => trace.attrs["stroke-dasharray"] !== undefined),
      [false, false, false, false, true],
    );
  });

  test("the trend shows the last span seconds, labelled back from now, and ends each trace at its last sample", () => {
    const node = draw(trend({ pv: read("Loop.PV") }, { min: 0, max: 100, span: 300 }), {
      history: { "Loop.PV": samples([400, 0], [200, 100], [30, 100]) },
      nowMs: NOW,
    });
    const plot = plotOf(node);
    const points = pointsOf(pen(node, "pv"));
    // The value at the window's start lies between the samples on either side of it.
    near(points[0][0], plot.x, "the trace starts at the left edge");
    near(points[0][1], plot.y + plot.height / 2, "with the value at the window's start");
    near(points[1][0], plot.x + plot.width / 3, "200 s ago is a third of the way in");
    near(points[1][1], plot.y, "100 is the top of the range");
    near(points[points.length - 1][0], plot.x + (plot.width * 270) / 300, "the trace ends 30 s before now");
    const texts = textsOf(node);
    assert.ok(texts.includes("−5 min") && texts.includes("now"), texts.join(" | "));
  });

  test("range chips choose the span, the chosen one marked; a paused window ends where it was left, with Back to live", () => {
    // §5.9 item 1.
    const object = trend({ pv: read("Loop.PV") }, { min: 0, max: 100, span: 300 });
    const history = { "Loop.PV": samples([200, 0], [100, 100], [0, 50]) };
    const live = draw(object, { history, nowMs: NOW });
    const chips = part(live, "trend-range").map((chip) => [
      chip.attrs["data-trend-range"],
      textsOf(chip).join(""),
      chip.attrs["data-selected"] ?? "",
    ]);
    assert.deepStrictEqual(chips, [
      ["300", "5 min", "true"],
      ["3600", "1 h", ""],
      ["28800", "8 h", ""],
      ["86400", "24 h", ""],
    ]);
    assert.strictEqual(part(live, "trend-live").length, 0, "live, there is nothing to go back to");
    const paused = draw(object, { history, nowMs: NOW, trendViews: { loop: { spanMs: 60_000, endMs: NOW - 90_000 } } });
    assert.deepStrictEqual(
      part(paused, "trend-range").map((chip) => chip.attrs["data-selected"] ?? ""),
      ["", "", "", ""],
      "a span no chip names",
    );
    assert.ok(
      textsOf(part(paused, "trend-live")[0] ?? { tag: "g", attrs: {}, children: [] }).includes("Back to live"),
      renderToString(paused),
    );
    const plot = plotOf(paused);
    const points = pointsOf(pen(paused, "pv"));
    near(
      points[points.length - 1][0],
      plot.x + plot.width * (50 / 60),
      "100 s ago is 50 s into the minute that ends 90 s ago",
    );
    const texts = textsOf(paused);
    assert.ok(!texts.includes("now"), "a paused window does not end now: " + texts.join(" | "));
    assert.ok(
      texts.filter((line) => /^\d\d:\d\d:\d\d$/.test(line)).length === 2,
      "it names its start and end: " + texts.join(" | "),
    );
  });

  test("the cursor line gives each pen's value at its time; a hidden pen is not drawn and its legend entry is dimmed", () => {
    // §5.9 item 1.
    const object = trend({ pv: read("Loop.PV"), sp: read("Loop.SP") }, { min: 0, max: 100, span: 300 });
    const history = { "Loop.PV": samples([200, 10], [100, 20], [0, 30]), "Loop.SP": samples([200, 50], [0, 60]) };
    const values = {
      "Loop.PV": { value: 30, state: "confirmed" as const },
      "Loop.SP": { value: 60, state: "confirmed" as const },
    };
    const node = draw(object, {
      history,
      values,
      nowMs: NOW,
      trendViews: { loop: { cursorMs: NOW - 150_000, hidden: ["sp"] } },
    });
    const plot = plotOf(node);
    const cursor = part(node, "trend-cursor")[0];
    assert.ok(cursor, renderToString(node));
    near(Number(cursor.attrs.x1), plot.x + plot.width / 2, "150 s ago is half way");
    const entry = (name: string) =>
      findAll(node, (child) => child.attrs["data-part"] === "legend" && child.attrs["data-pen"] === name)[0];
    assert.ok(
      textsOf(entry("pv")).join(" ").includes("10"),
      "the value at the cursor, not now's 30: " + textsOf(entry("pv")).join(" "),
    );
    assert.ok(!textsOf(entry("pv")).join(" ").includes("30"), textsOf(entry("pv")).join(" "));
    assert.ok(
      textsOf(node).some((line) => /^\d\d:\d\d:\d\d$/.test(line)),
      "the cursor's time",
    );
    assert.strictEqual(
      findAll(node, (child) => child.tag === "polyline" && child.attrs["data-pen"] === "sp").length,
      0,
      "hidden",
    );
    assert.strictEqual(entry("sp").attrs["data-hidden"], "true");
  });

  test("the tool row ends with Export and Expand", () => {
    const node = draw(trend({ pv: read("Loop.PV") }), { history: { "Loop.PV": samples([60, 1]) }, nowMs: NOW });
    const actions = findAll(node, (child) => child.attrs["data-trend-action"] !== undefined).map(
      (child) => child.attrs["data-trend-action"],
    );
    assert.deepStrictEqual(actions, ["export", "expand"]);
  });

  test("a trend's visibility binding is no pen", () => {
    const node = draw(trend({ pv: read("Loop.PV"), visible: read("Loop.Shown") }), {
      history: { "Loop.PV": samples([60, 1]) },
      nowMs: NOW,
      values: { "Loop.Shown": { value: true, state: "confirmed" } },
    });
    assert.strictEqual(
      findAll(node, (child) => child.attrs["data-part"] === "legend" && child.attrs["data-pen"] === "visible").length,
      0,
    );
  });

  test("the span defaults to five minutes", () => {
    const node = draw(trend({ pv: read("Loop.PV") }), { history: { "Loop.PV": samples([60, 1]) }, nowMs: NOW });
    assert.ok(textsOf(node).includes("−5 min"), textsOf(node).join(" | "));
  });

  test("without a range the axis fits the samples and limits with a margin; with a range samples outside are drawn at its edge", () => {
    const fitted = draw(trend({ pv: read("Loop.PV"), "high-limit": read("Loop.High") }), {
      history: { "Loop.PV": samples([60, 10], [30, 20]) },
      values: { "Loop.High": { value: 30, state: "confirmed" } },
      nowMs: NOW,
    });
    const plot = plotOf(fitted);
    const [low, high] = pointsOf(pen(fitted, "pv"));
    const limit = part(fitted, "limit")[0];
    assert.ok(limit, renderToString(fitted));
    const limitY = Number(limit.attrs.y1);
    assert.ok(low[1] < plot.y + plot.height && low[1] > high[1], "the lowest sample sits above the bottom");
    assert.ok(limitY > plot.y && limitY < high[1], "the limit is inside the axis, above the samples");

    const clamped = draw(trend({ pv: read("Loop.PV") }, { min: 0, max: 100 }), {
      history: { "Loop.PV": samples([60, 150], [30, -20]) },
      nowMs: NOW,
    });
    const edges = pointsOf(pen(clamped, "pv"));
    const box = plotOf(clamped);
    near(edges[0][1], box.y, "above the range is drawn at the top");
    near(edges[1][1], box.y + box.height, "below the range is drawn at the bottom");
  });

  test("a boolean pen is a step line in its own lane under the plot", () => {
    const node = draw(trend({ pv: read("Loop.PV"), run: read("Pump.Running") }), {
      history: { "Loop.PV": samples([60, 1], [10, 2]), "Pump.Running": samples([90, 0], [60, 1], [20, 0]) },
      values: { "Pump.Running": { value: false, state: "confirmed" } },
      nowMs: NOW,
    });
    const plot = plotOf(node);
    const points = pointsOf(pen(node, "run"));
    assert.ok(
      points.every(([, y]) => y > plot.y + plot.height),
      "under the plot",
    );
    for (let index = 1; index < points.length; index += 1) {
      const [x0, y0] = points[index - 1];
      const [x1, y1] = points[index];
      assert.ok(x0 === x1 || y0 === y1, `segment ${index} is horizontal or vertical`);
    }
    const levels = new Set(points.map(([, y]) => y));
    assert.strictEqual(levels.size, 2, "high and low");
  });

  test("a limit is a dashed warning line with a faint band beyond it; an unknown limit is not drawn", () => {
    const node = draw(
      trend(
        { pv: read("Loop.PV"), "high-limit": read("Loop.High"), "low-limit": read("Loop.Low") },
        { min: 0, max: 100 },
      ),
      {
        history: { "Loop.PV": samples([60, 50]) },
        values: { "Loop.High": { value: 80, state: "confirmed" }, "Loop.Low": { value: null, state: "unknown" } },
        nowMs: NOW,
      },
    );
    const plot = plotOf(node);
    const limits = part(node, "limit");
    assert.strictEqual(limits.length, 1, renderToString(node));
    assert.strictEqual(limits[0].attrs.stroke, "var(--hmi-warning)");
    assert.ok(limits[0].attrs["stroke-dasharray"]);
    near(Number(limits[0].attrs.y1), plot.y + plot.height * 0.2, "80 of 0..100");
    const band = part(node, "limit-band")[0];
    assert.ok(band, renderToString(node));
    assert.strictEqual(band.attrs.fill, "var(--hmi-warning)");
    assert.ok(Number(band.attrs["fill-opacity"]) <= 0.15);
    near(Number(band.attrs.y), plot.y, "the band runs from the top");
    near(Number(band.attrs.height), plot.height * 0.2, "down to the high limit");
  });

  test("the legend shows each pen's name and current value in its state", () => {
    const values: Record<string, LiveValue> = {
      "Loop.PV": { value: 62.5, state: "confirmed" },
      "Loop.SP": { value: null, state: "unknown" },
      "Loop.MV": { value: 41, state: "confirmed", staleMs: 12_000 },
      "Loop.Model": { value: 60.2, state: "estimated" },
    };
    const node = draw(
      trend(
        { pv: read("Loop.PV", "°C"), sp: read("Loop.SP"), mv: read("Loop.MV"), model: read("Loop.Model") },
        { decimals: 1 },
      ),
      { values, nowMs: NOW },
    );
    const legend = (name: string): string[] => {
      const item = findAll(
        node,
        (child) => child.attrs["data-part"] === "legend" && child.attrs["data-pen"] === name,
      )[0];
      assert.ok(item, `legend ${name}: ${renderToString(node)}`);
      return textsOf(item);
    };
    assert.deepStrictEqual(legend("pv").slice(0, 3), ["pv", "62.5", "°C"]);
    assert.ok(legend("sp").includes("?"));
    assert.ok(legend("mv").includes("41.0") && legend("mv").includes("12 s"));
    assert.ok(legend("model").includes("est."));
  });

  test("the legend entries of a row flow in one text, so they never overlap; a long legend wraps and moves the plot down", () => {
    const legendRows = (node: SvgNode): SvgNode[] =>
      findAll(
        node,
        (child) => child.tag === "text" && findAll(child, (inner) => inner.attrs["data-part"] === "legend").length > 0,
      );
    const short = draw(
      trend({ mv: read("Loop.MV"), pv: read("Loop.PV"), sp: read("Loop.SP"), run: read("Loop.Run") }),
      { nowMs: NOW },
    );
    const rows = legendRows(short);
    assert.strictEqual(rows.length, 1, renderToString(short));
    assert.strictEqual(findAll(rows[0], (inner) => inner.attrs["data-part"] === "legend").length, 4);
    const bind: Record<string, HmiBindingModel> = {};
    for (let index = 1; index <= 8; index += 1) bind[`temperature-${index}`] = read(`Zone.T${index}`, "°C");
    const long = draw(trend(bind, { size: [360, 320] }), { nowMs: NOW });
    const wrapped = legendRows(long);
    assert.ok(wrapped.length >= 2, renderToString(long));
    const baselines = wrapped.map((row) => Number(row.attrs.y));
    assert.ok(
      baselines.every((y, index) => index === 0 || y > baselines[index - 1]),
      baselines.join(", "),
    );
    assert.ok(plotOf(long).y > plotOf(short).y, "the plot starts below the legend");
  });

  test("a boolean lane has its own background, and its label clears the bottom axis label", () => {
    const node = draw(trend({ pv: read("Loop.PV"), run: read("Pump.Running") }, { min: 0, max: 100 }), {
      history: { "Pump.Running": samples([90, 0], [60, 1]) },
      values: { "Pump.Running": { value: true, state: "confirmed" } },
      nowMs: NOW,
    });
    const plot = plotOf(node);
    const lane = part(node, "lane")[0];
    assert.ok(lane && Number(lane.attrs.y) > plot.y + plot.height, renderToString(node));
    const left = (content: string): SvgNode => {
      const found = findAll(node, (child) => child.text === content && Number(child.attrs.x) < plot.x)[0];
      assert.ok(found, `${content}: ${renderToString(node)}`);
      return found;
    };
    // Two 14 px labels need 18 px between their baselines not to touch.
    assert.ok(Number(left("run").attrs.y) - Number(left("0").attrs.y) >= 18);
  });

  test("axis labels are plain numbers, without the value decimals", () => {
    const node = draw(trend({ pv: read("Loop.PV") }, { min: 0, max: 100, decimals: 1 }), { nowMs: NOW });
    const texts = textsOf(node);
    for (const label of ["0", "50", "100"]) assert.ok(texts.includes(label), `${label}: ${texts.join(" | ")}`);
    assert.ok(!texts.includes("0.0") && !texts.includes("100.0"), texts.join(" | "));
  });

  test("the pen colours are tokens in both themes", () => {
    const css = tokensCss();
    for (const name of ["pen-1", "pen-2", "pen-3", "pen-4"]) {
      assert.strictEqual(css.split(`--hmi-${name}: #`).length - 1, 2, name);
    }
  });
});

suite("HMI drawing: table", () => {
  const rowsOf = (node: SvgNode): string[][] => part(node, "row").map((row) => textsOf(row));
  const headerOf = (node: SvgNode): string[] => {
    const header = part(node, "header")[0];
    assert.ok(header, `a header row: ${renderToString(node)}`);
    return textsOf(header);
  };

  test("a structure is one row per field with its name and value", () => {
    const node = draw(table({ decimals: 0 }), {
      values: { "Line.Data": { value: { Speed: 1180, Running: true, Mode: "Auto" }, state: "confirmed" } },
    });
    assert.deepStrictEqual(rowsOf(node), [["Speed", "1180"], ["Running"], ["Mode", "Auto"]]);
    const flags = part(node, "flag");
    assert.deepStrictEqual(
      flags.map((flag) => flag.attrs["data-on"]),
      ["true"],
      "a flag is a mark, lit while TRUE",
    );
  });

  test("flags are marks, texts are drawn whole from the left, and field names read as words", () => {
    const conditions = [
      { Text: "Tank T-101 level high-high", Active: false, Bypassed: false, FirstOut: false },
      { Text: "Door of the pump room open", Active: true, Bypassed: false, FirstOut: true },
    ];
    const node = draw(table({ size: [608, 300] }), {
      values: { "Line.Data": { value: conditions, state: "confirmed", lowerBounds: [1] } },
    });
    assert.deepStrictEqual(headerOf(node), ["#", "Text", "Active", "Bypassed", "First out"]);
    assert.deepStrictEqual(rowsOf(node), [
      ["1", "Tank T-101 level high-high"],
      ["2", "Door of the pump room open"],
    ]);
    assert.ok(!textsOf(node).some((content) => content === "TRUE" || content === "FALSE"), renderToString(node));
    assert.deepStrictEqual(
      part(node, "flag").map((flag) => flag.attrs["data-on"]),
      ["false", "false", "false", "true", "false", "true"],
    );
    const texts = findAll(node, (child) => child.text === "Door of the pump room open");
    assert.strictEqual(texts[0]?.attrs["text-anchor"] ?? "start", "start", "a text cell starts at its column's left");
  });

  test("the empty rows at the end of an array of structures are left out, with their count", () => {
    const rows = Array.from({ length: 16 }, (_, index) => ({
      Text: index < 3 ? `Condition ${index + 1}` : "",
      Active: index === 1,
      Count: 0,
    }));
    const node = draw(table({ size: [608, 600] }), {
      values: { "Line.Data": { value: rows, state: "confirmed", lowerBounds: [1] } },
    });
    assert.deepStrictEqual(
      rowsOf(node).map((row) => row[0]),
      ["1", "2", "3"],
    );
    assert.ok(textsOf(node).includes("Empty rows: 13"), renderToString(node));
  });

  test("a duration reads in words of its parts", () => {
    const node = draw(table(), {
      values: {
        "Line.Data": {
          value: { Long: "T#1h2m3s500ms", Zero: "T#0s", Back: "T#-1s500ms", Days: "T#1d1h1m1s" },
          state: "confirmed",
        },
      },
    });
    assert.deepStrictEqual(rowsOf(node), [
      ["Long", "1 h 2 min 3.5 s"],
      ["Zero", "0 s"],
      ["Back", "-1.5 s"],
      ["Days", "1 d 1 h 1 min 1 s"],
    ]);
  });

  test("a duration's seconds are rounded to the object's decimals, carrying a minute (§5.3 item 8)", () => {
    const shown = (literal: string, decimals: number) => formatValue(literal, { decimals });
    assert.strictEqual(shown("T#2m54s353ms", 0), "2 min 54 s");
    assert.strictEqual(shown("T#1m59s600ms", 0), "2 min");
    assert.strictEqual(shown("T#2m54s353ms", 1), "2 min 54.4 s");
    assert.strictEqual(shown("T#400ms", 0), "0 s");
    assert.strictEqual(shown("T#1h0m0s", 0), "1 h");
  });

  test("an array is one row per element, labelled with its index from the declared lower bound", () => {
    const node = draw(table(), {
      values: { "Line.Data": { value: [10, 20, 30], state: "confirmed", lowerBounds: [1] } },
    });
    assert.deepStrictEqual(rowsOf(node), [
      ["1", "10"],
      ["2", "20"],
      ["3", "30"],
    ]);
    const fromZero = draw(table(), { values: { "Line.Data": { value: [7], state: "confirmed" } } });
    assert.deepStrictEqual(rowsOf(fromZero), [["0", "7"]]);
  });

  test("an array of structures has one column per field", () => {
    const node = draw(table(), {
      values: {
        "Line.Data": {
          value: [
            { a: 1, b: 2 },
            { a: 3, b: 4 },
          ],
          state: "confirmed",
          lowerBounds: [1],
        },
      },
    });
    assert.deepStrictEqual(headerOf(node), ["#", "a", "b"]);
    assert.deepStrictEqual(rowsOf(node), [
      ["1", "1", "2"],
      ["2", "3", "4"],
    ]);
  });

  test("a two-dimensional array is a grid with row and column indices", () => {
    const node = draw(table(), {
      values: {
        "Line.Data": {
          value: [
            [1, 2, 3],
            [4, 5, 6],
          ],
          state: "confirmed",
          lowerBounds: [1, 0],
        },
      },
    });
    assert.deepStrictEqual(headerOf(node), ["0", "1", "2"]);
    assert.deepStrictEqual(rowsOf(node), [
      ["1", "1", "2", "3"],
      ["2", "4", "5", "6"],
    ]);
  });

  test("a value nested deeper is shown by its size", () => {
    const node = draw(table(), {
      values: {
        "Line.Data": {
          value: { Name: "P1", Samples: [1, 2, 3, 4], Config: { a: 1, b: 2, c: 3, d: 4 } },
          state: "confirmed",
        },
      },
    });
    assert.deepStrictEqual(rowsOf(node), [
      ["Name", "P1"],
      ["Samples", "[4]"],
      ["Config", "{4}"],
    ]);
  });

  test("rows that do not fit end with a +N more row; an empty array shows Empty", () => {
    const values = Array.from({ length: 50 }, (_, index) => index);
    const node = draw(table({ size: [320, 160] }), { values: { "Line.Data": { value: values, state: "confirmed" } } });
    const shown = part(node, "row").length;
    const more = textsOf(node).find((content) => /^\+\d+ more$/.test(content));
    assert.ok(shown > 0 && more, renderToString(node));
    assert.strictEqual(shown + Number(more.slice(1, -5)), 50);
    const lastRow = part(node, "row")[shown - 1];
    const moreRow = part(node, "more")[0];
    assert.ok(moreRow && Number(moreRow.attrs["data-y"]) > Number(lastRow.attrs["data-y"]));
    assert.ok(Number(moreRow.attrs["data-y"]) + 30 <= 160, "inside the box");

    const empty = draw(table(), { values: { "Line.Data": { value: [], state: "confirmed" } } });
    assert.ok(textsOf(empty).includes("Empty"));
  });

  test("an unknown table shows ? over the hatch; stale and estimated are marked", () => {
    const unknown = draw(table(), { values: { "Line.Data": { value: null, state: "unknown" } } });
    assert.ok(textsOf(unknown).includes("?"));
    assert.ok(findAll(unknown, (child) => child.attrs.fill === "url(#hmi-unknown-hatch)").length > 0);
    assert.strictEqual(part(unknown, "row").length, 0);
    const stale = draw(table(), { values: { "Line.Data": { value: [1], state: "confirmed", staleMs: 12_000 } } });
    assert.ok(textsOf(stale).includes("12 s"));
    const estimated = draw(table(), { values: { "Line.Data": { value: [1], state: "estimated" } } });
    assert.ok(textsOf(estimated).includes("est."));
  });
});

suite("HMI bounded collection tables", () => {
  test("the registered count hides unused capacity and refuses a missing count", () => {
    const object = table({ bind: { value: read("Line.Data"), "row-count": read("Line.Count") } });
    const frame = {
      values: {
        "Line.Data": {
          value: [
            { Name: "P101", State: "Running" },
            { Name: "unused", State: "Unknown" },
          ],
          state: "confirmed" as const,
        },
        "Line.Count": { value: 1, state: "confirmed" as const },
      },
    };
    const rendered = renderToString(draw(object, frame));
    assert.ok(rendered.includes("P101"));
    assert.ok(!rendered.includes("unused"));
    const unknown = renderToString(draw(object, { values: { "Line.Data": frame.values["Line.Data"] } }));
    assert.ok(!unknown.includes("P101"), "an unknown count cannot present unregistered entries as current members");
  });
});
