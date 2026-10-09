import * as assert from "assert";

import { drawPage } from "../../hmiBuilder/drawing";
import type { HmiObjectModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { findAll, renderToString, SvgNode, textsOf } from "../../hmiBuilder/drawing/svg";

// Specification 39, §5.2 and §5.3: bar, gauge and split bar.

function draw(object: HmiObjectModel, values: Record<string, LiveValue>): SvgNode {
  const svg = drawPage({ title: "Ranges", object: [object], pipe: [] }, { size: [1280, 720], values });
  const found = findAll(svg, (node) => node.attrs["data-object"] === object.id);
  assert.strictEqual(found.length, 1, renderToString(svg));
  return found[0];
}

function display(type: string, bind: Record<string, string>, extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  const bindings = Object.fromEntries(
    Object.entries(bind).map(([property, path]) => [property, { path, direction: "read" as const }]),
  );
  return { id: type, type, at: [0, 0], size: [60, 200], rotation: 0, bind: bindings, ...extra };
}

function part(node: SvgNode, name: string): SvgNode | undefined {
  return findAll(node, (child) => child.attrs["data-part"] === name)[0];
}

function value(number: number, state: LiveValue["state"] = "confirmed"): LiveValue {
  return { value: number, state };
}

suite("HMI drawing: ranges", () => {
  test("a labelled stale bar reserves a separate age row above its track", () => {
    const node = draw(display("bar", { value: "Main.Level" }, { label: "LT-101", size: [64, 300] }), {
      "Main.Level": { ...value(50), staleMs: 3000 },
    });
    const label = part(node, "label")!;
    const age = findAll(node, (child) => child.tag === "text" && child.text === "3 s")[0];
    assert.ok(age, "the stale age remains visible");
    assert.ok(Number(age.attrs.y) - Number(label.attrs.y) >= 20, "title and age have separate baselines");
    assert.ok(Number(part(node, "track")!.attrs.y) > Number(age.attrs.y), "age remains above the track");
  });
  test("a bar fills from its minimum to the value within its range", () => {
    const node = draw(display("bar", { value: "Main.Level" }, { min: 0, max: 200, decimals: 0 }), {
      "Main.Level": value(50),
    });
    const track = part(node, "track");
    const fill = part(node, "fill");
    assert.ok(track && fill, renderToString(node));
    assert.strictEqual(fill.attrs["data-fraction"], "0.25");
    assert.strictEqual(Number(fill.attrs.height), Number(track.attrs.height) * 0.25);
    assert.strictEqual(
      Number(fill.attrs.y) + Number(fill.attrs.height),
      Number(track.attrs.y) + Number(track.attrs.height),
      "a vertical bar fills from the bottom",
    );
    assert.ok(textsOf(node).includes("50"), textsOf(node).join(" "));
  });

  test("a bar shows its label above its track, a horizontal bar too, and a split bar above its bars", () => {
    const labelled = (node: SvgNode, words: string): SvgNode | undefined =>
      findAll(node, (child) => child.tag === "text" && child.text === words)[0];
    const trackTop = (node: SvgNode): number => {
      const tracks = findAll(node, (child) => /(^|-)track$/.test(child.attrs["data-part"] ?? ""));
      assert.ok(tracks.length > 0, `the tracks: ${renderToString(node)}`);
      return Math.min(...tracks.map((track) => Number(track.attrs.y)));
    };
    const vertical = draw(display("bar", { value: "Main.Feed" }, { size: [80, 300], label: "Feed", unit: "%" }), {
      "Main.Feed": value(60),
    });
    const feed = labelled(vertical, "Feed");
    assert.ok(feed, renderToString(vertical));
    assert.ok(
      Number(feed.attrs.y) < trackTop(vertical),
      `the label at ${feed.attrs.y}, the track from ${trackTop(vertical)}`,
    );
    const horizontal = draw(
      display("bar", { value: "Main.Feed" }, { size: [260, 64], label: "Feed", variant: "horizontal", unit: "%" }),
      { "Main.Feed": value(60) },
    );
    const across = labelled(horizontal, "Feed");
    assert.ok(across, renderToString(horizontal));
    assert.ok(
      Number(across.attrs.y) < trackTop(horizontal),
      `the label at ${across.attrs.y}, the track from ${trackTop(horizontal)}`,
    );
    const split = draw(
      display(
        "split-bar",
        { first: "Main.A", request: "Main.R", second: "Main.B" },
        { size: [120, 300], label: "Heat" },
      ),
      {
        "Main.A": value(40),
        "Main.R": value(50),
        "Main.B": value(60),
      },
    );
    const heat = labelled(split, "Heat");
    assert.ok(heat, renderToString(split));
    assert.ok(
      Number(heat.attrs.y) < trackTop(split),
      `the label at ${heat.attrs.y}, the tracks from ${trackTop(split)}`,
    );
  });

  test("a horizontal bar fills from the left", () => {
    const node = draw(
      display("bar", { value: "Main.Level" }, { variant: "horizontal", size: [200, 48], min: 0, max: 100 }),
      { "Main.Level": value(40) },
    );
    const track = part(node, "track");
    const fill = part(node, "fill");
    assert.ok(track && fill);
    assert.strictEqual(fill.attrs.x, track.attrs.x);
    assert.strictEqual(Number(fill.attrs.width), Number(track.attrs.width) * 0.4);
  });

  test("a value outside the range clamps the fill and marks over or under range", () => {
    const over = draw(display("bar", { value: "Main.Level" }, { min: 0, max: 100 }), { "Main.Level": value(130) });
    const under = draw(display("bar", { value: "Main.Level" }, { min: 0, max: 100 }), { "Main.Level": value(-5) });
    assert.strictEqual(part(over, "fill")?.attrs["data-fraction"], "1");
    assert.strictEqual(over.attrs["data-range"], "over");
    assert.strictEqual(part(under, "fill")?.attrs["data-fraction"], "0");
    assert.strictEqual(under.attrs["data-range"], "under");
    assert.ok(part(over, "range-mark"), "over range is marked");
  });

  test("an unknown bar shows no fill and a question mark", () => {
    const node = draw(display("bar", { value: "Main.Level" }), { "Main.Level": value(50, "unknown") });
    assert.strictEqual(part(node, "fill"), undefined);
    assert.ok(textsOf(node).includes("?"));
  });

  test("a gauge sweeps to the value's fraction of its range", () => {
    const node = draw(display("gauge", { value: "Main.Speed" }, { size: [160, 160], min: 0, max: 80, decimals: 0 }), {
      "Main.Speed": value(50),
    });
    assert.strictEqual(part(node, "fill")?.attrs["data-fraction"], "0.625");
    assert.ok(part(node, "track"));
    assert.ok(textsOf(node).includes("50"));
  });

  test("a split bar shows the request and both outputs", () => {
    const node = draw(
      display(
        "split-bar",
        { request: "Loop.Request", first: "Loop.Heat", second: "Loop.Cool" },
        { size: [96, 200], min: 0, max: 100 },
      ),
      { "Loop.Request": value(75), "Loop.Heat": value(0), "Loop.Cool": value(50) },
    );
    assert.strictEqual(part(node, "first-fill")?.attrs["data-fraction"], "0");
    assert.strictEqual(part(node, "second-fill")?.attrs["data-fraction"], "0.5");
    assert.strictEqual(part(node, "request")?.attrs["data-fraction"], "0.75");
  });
});

suite("HMI drawing: layout", () => {
  const textWith = (node: SvgNode, content: string): SvgNode | undefined =>
    findAll(node, (child) => child.tag === "text" && child.children.some((span) => span.text === content))[0];

  test("a value's unit follows its number in one text run, so they cannot overlap", () => {
    const svg = drawPage(
      {
        title: "Layout",
        pipe: [],
        object: [
          {
            id: "level",
            type: "value",
            at: [0, 0],
            size: [220, 72],
            rotation: 0,
            decimals: 1,
            unit: "%",
            bind: { value: { path: "A.Level", direction: "read" } },
          },
        ],
      },
      { size: [1280, 720], values: { "A.Level": value(62.5) } },
    );
    const run = textWith(svg, "62.5");
    assert.ok(run, renderToString(svg));
    assert.deepStrictEqual(
      run.children.map((span) => span.text),
      ["62.5", "%"],
    );
    assert.ok(Number(run.children[1].attrs.dx) > 0, "a gap separates the unit");
  });

  test("a horizontal bar shows its value inline to the right of the track", () => {
    const node = draw(
      display("bar", { value: "Main.Level" }, { variant: "horizontal", size: [300, 56], unit: "%", decimals: 0 }),
      { "Main.Level": value(40) },
    );
    const track = part(node, "track");
    const run = textWith(node, "40");
    assert.ok(track && run, renderToString(node));
    assert.deepStrictEqual(
      run.children.map((span) => span.text),
      ["40", "%"],
    );
    assert.ok(Number(run.attrs.x) > Number(track.attrs.x) + Number(track.attrs.width), "right of the track");
  });

  test("a gauge labels the ends of its range as plain numbers, like a trend's axis", () => {
    const node = draw(display("gauge", { value: "Main.Temp" }, { size: [200, 200], min: 0, max: 100, decimals: 1 }), {
      "Main.Temp": { value: 37.4, state: "confirmed" },
    });
    const texts = textsOf(node);
    assert.ok(texts.includes("0") && texts.includes("100"), texts.join(" | "));
    assert.ok(texts.includes("37.4"), "the value keeps its decimals");
    assert.ok(!texts.includes("0.0") && !texts.includes("100.0"), texts.join(" | "));
  });

  test("a gauge marks a commanded target on its arc and names it under the value", () => {
    const node = draw(display("gauge", { value: "Pump.Speed" }, { size: [200, 200], min: 0, max: 1500, decimals: 0 }), {
      "Pump.Speed": { value: 1180, state: "commanded", target: 1500 },
    });
    assert.strictEqual(part(node, "target")?.attrs["data-fraction"], "1");
    const target = findAll(node, (child) => child.text === "→ 1500")[0];
    const maximum = findAll(node, (child) => child.text === "1500")[0];
    assert.ok(target && maximum, renderToString(node));
    assert.ok(Number(target.attrs.y) < Number(maximum.attrs.y), "the target reads above the range labels");
  });

  test("a stale gauge fades its value and keeps a neutral track", () => {
    const node = draw(display("gauge", { value: "A.Pressure" }, { size: [200, 200], min: 0, max: 6 }), {
      "A.Pressure": { value: 4.25, state: "confirmed", staleMs: 12_000 },
    });
    assert.strictEqual(part(node, "track")?.attrs.stroke, "var(--hmi-line)");
    assert.ok(Number(part(node, "fill")?.attrs.opacity) < 1);
    assert.ok(textsOf(node).includes("12 s"));
  });

  test("gauges and bars fill with the accent gradient in their direction", () => {
    const gauge = draw(display("gauge", { value: "A.Speed" }, { size: [160, 160] }), { "A.Speed": value(50) });
    const bar = draw(display("bar", { value: "A.Speed" }), { "A.Speed": value(50) });
    const across = draw(display("bar", { value: "A.Speed" }, { variant: "horizontal", size: [300, 56] }), {
      "A.Speed": value(50),
    });
    assert.strictEqual(part(gauge, "fill")?.attrs.stroke, "url(#hmi-accent-x)");
    assert.strictEqual(part(bar, "fill")?.attrs.fill, "url(#hmi-accent-y)");
    assert.strictEqual(part(across, "fill")?.attrs.fill, "url(#hmi-accent-x)");
  });

  test("an unknown status hatches only its lamp", () => {
    const node = draw(display("status", { state: "A.Sensor" }, { size: [200, 40], label: "Sensor" }), {
      "A.Sensor": value(0, "unknown"),
    });
    assert.strictEqual(
      findAll(node, (child) => child.tag === "rect" && child.attrs.fill === "url(#hmi-unknown-hatch)").length,
      0,
      renderToString(node),
    );
    assert.strictEqual(part(node, "lamp")?.attrs.fill, "url(#hmi-unknown-hatch)");
  });
});

suite("HMI drawing: rendering rules", () => {
  test("no straight horizontal or vertical line carries a filter, which SVG would drop", () => {
    const node = draw(
      display("split-bar", { request: "Loop.Request", first: "Loop.Heat", second: "Loop.Cool" }, { size: [96, 200] }),
      { "Loop.Request": value(75), "Loop.Heat": value(10), "Loop.Cool": value(50) },
    );
    const straight = findAll(
      node,
      (child) =>
        child.tag === "line" &&
        child.attrs.filter !== undefined &&
        (child.attrs.x1 === child.attrs.x2 || child.attrs.y1 === child.attrs.y2),
    );
    assert.deepStrictEqual(
      straight.map((line) => line.attrs["data-part"]),
      [],
    );
  });
});
