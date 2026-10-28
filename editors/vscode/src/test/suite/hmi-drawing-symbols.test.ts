import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { drawPage } from "../../hmiBuilder/drawing";
import type { DrawFrame, HmiObjectModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { findAll, renderToString, SvgNode, textsOf } from "../../hmiBuilder/drawing/svg";
import { LABEL_HEIGHT, SPIN_PERIOD_MS, SYMBOLS } from "../../hmiBuilder/drawing/symbols";
import { tokensCss } from "../../hmiBuilder/drawing/tokens";

// Specification 39, §5.3 item 4 and §14.2: the two-dimensional P&ID symbols of milestone 2A, the
// pump, the on/off valve and the control valve, where what moves in the plant moves in the symbol.

const ROOT = path.resolve(__dirname, "../../../../..");

function draw(object: HmiObjectModel, values: Record<string, LiveValue>, frame: Partial<DrawFrame> = {}): SvgNode {
  const svg = drawPage({ title: "Symbols", object: [object], pipe: [] }, { size: [1280, 720], values, ...frame });
  const found = findAll(svg, (node) => node.attrs["data-object"] === object.id);
  assert.strictEqual(found.length, 1, renderToString(svg));
  return found[0];
}

type SymbolType = "pump" | "valve" | "control-valve";

function equipment(
  type: SymbolType,
  bind: Record<string, string>,
  extra: Partial<HmiObjectModel> = {},
): HmiObjectModel {
  const [width, height] = SYMBOLS[type].size;
  const bindings = Object.fromEntries(
    Object.entries(bind).map(([property, bound]) => [property, { path: bound, direction: "read" as const }]),
  );
  return {
    id: type,
    type,
    tag: "P-101",
    at: [0, 0],
    size: [width, height + LABEL_HEIGHT],
    rotation: 0,
    bind: bindings,
    ...extra,
  };
}

function on(value: boolean | number, state: LiveValue["state"] = "confirmed"): LiveValue {
  return { value, state };
}

function part(node: SvgNode, name: string): SvgNode | undefined {
  return findAll(node, (child) => child.attrs["data-part"] === name)[0];
}

function sign(node: SvgNode, name: string): SvgNode | undefined {
  return findAll(node, (child) => child.attrs["data-sign"] === name)[0];
}

/** The text drawn in a part, as one string. */
function words(node: SvgNode | undefined): string {
  return node ? textsOf(node).join(" ") : "";
}

/** The vertical extent of the symbol, in the object's pixels. */
function symbolBottom(drawn: SvgNode, type: SymbolType): number {
  const symbol = part(drawn, "symbol");
  assert.ok(symbol, renderToString(drawn));
  const match = /translate\(([\d.]+) ([\d.]+)\) scale\(([\d.]+)\)/.exec(String(symbol.attrs.transform));
  assert.ok(match, String(symbol.attrs.transform));
  return Number(match[2]) + SYMBOLS[type].size[1] * Number(match[3]);
}

suite("HMI drawing: P&ID symbols", () => {
  test("a pump is its P&ID symbol: the casing with its outlet, the stand and the impeller, with an inlet and an outlet", () => {
    const pump = draw(equipment("pump", { running: "P1.Running" }), { "P1.Running": on(false) });
    for (const name of ["casing", "stand", "impeller"])
      assert.ok(part(pump, name), `the ${name}: ${renderToString(pump)}`);
    const [width] = SYMBOLS.pump.size;
    assert.deepStrictEqual(Object.keys(SYMBOLS.pump.ports).sort(), ["inlet", "outlet"]);
    assert.strictEqual(SYMBOLS.pump.ports.inlet.direction, "left");
    assert.strictEqual(SYMBOLS.pump.ports.inlet.at[0], 0, "the inlet is on the symbol's left edge");
    assert.strictEqual(SYMBOLS.pump.ports.outlet.at[0], width, "the outlet leaves at the symbol's right edge");
  });

  test("a running pump's impeller turns, steadily across redraws, and a stopped one stands still", () => {
    const pump = equipment("pump", { running: "P1.Running" });
    const running = draw(pump, { "P1.Running": on(true) }, { nowMs: 12_345 });
    const impeller = part(running, "impeller");
    assert.strictEqual(impeller?.attrs.class, "hmi-spin", renderToString(running));
    // A redraw at 12 345 ms continues the turn where it is, not from the start.
    assert.match(String(impeller?.attrs.style), new RegExp(`animation-delay: -${12_345 % SPIN_PERIOD_MS}ms`));
    assert.strictEqual(running.attrs["data-equipment"], "running");
    const stopped = draw(pump, { "P1.Running": on(false) }, { nowMs: 12_345 });
    assert.strictEqual(part(stopped, "impeller")?.attrs.class, undefined);
    assert.strictEqual(stopped.attrs["data-equipment"], "stopped");
  });

  test("nothing turns while the value is old or only commanded, and an unknown state draws no impeller", () => {
    const pump = equipment("pump", { running: "P1.Running" });
    const stale = draw(pump, { "P1.Running": { value: true, state: "confirmed", staleMs: 12_000 } });
    assert.strictEqual(part(stale, "impeller")?.attrs.class, undefined, "a stale value does not turn");
    const outline = part(stale, "stale-outline");
    assert.strictEqual(outline?.attrs.stroke, "var(--hmi-stale)");
    // The outline marks the symbol and leaves the label clear.
    const tagTop = Number(part(stale, "tag")?.attrs.y) - 14;
    assert.ok(
      Number(outline?.attrs.y) + Number(outline?.attrs.height) <= tagTop,
      `the outline ends above the tag: ${renderToString(outline!)}`,
    );
    assert.ok(words(sign(stale, "stale")).includes("12 s"), renderToString(stale));
    const commanded = draw(pump, { "P1.Running": { value: true, state: "commanded", target: true } });
    assert.strictEqual(part(commanded, "impeller")?.attrs.class, undefined, "a commanded value does not turn");
    const unknown = draw(pump, { "P1.Running": { value: true, state: "unknown" } });
    assert.strictEqual(part(unknown, "impeller"), undefined, "an unknown state never shows the last one");
    assert.strictEqual(part(unknown, "casing")?.attrs.fill, "url(#hmi-unknown-hatch)");
    assert.strictEqual(words(part(unknown, "state")), "?");
    assert.strictEqual(unknown.attrs["data-equipment"], undefined);
  });

  test("the tag and the state word stand under the symbol, in 14 px", () => {
    const drawn = draw(equipment("pump", { running: "P1.Running" }), { "P1.Running": on(true) });
    const tag = part(drawn, "tag");
    const state = part(drawn, "state");
    assert.strictEqual(words(tag), "P-101");
    assert.strictEqual(words(state), "Running");
    const bottom = symbolBottom(drawn, "pump");
    for (const label of [tag, state]) {
      assert.ok(Number(label?.attrs.y) > bottom, `${words(label)} at ${label?.attrs.y}, the symbol down to ${bottom}`);
      assert.strictEqual(Number(label?.attrs["font-size"]), 14);
    }
  });

  test("a fault keeps its word in the alarm colour with ! beside it, manual shows M, and out of service shows OOS and dims the symbol", () => {
    const pump = equipment("pump", {
      running: "P1.Running",
      fault: "P1.Fault",
      manual: "P1.Manual",
      "out-of-service": "P1.OutOfService",
    });
    const fault = draw(pump, {
      "P1.Running": on(true),
      "P1.Fault": on(true),
      "P1.Manual": on(false),
      "P1.OutOfService": on(false),
    });
    const state = part(fault, "state");
    assert.strictEqual(words(state), "Fault");
    assert.strictEqual(state?.attrs.fill, "var(--hmi-alarm)");
    assert.strictEqual(part(fault, "impeller")?.attrs.class, "hmi-spin", "a running pump with a fault still turns");
    const alarm = sign(fault, "fault");
    assert.ok(alarm && words(alarm).includes("!"), renderToString(fault));
    const bottom = symbolBottom(fault, "pump");
    for (const rect of findAll(alarm, (child) => child.tag === "rect" || child.tag === "circle")) {
      const top = Number(rect.attrs.y ?? Number(rect.attrs.cy) - Number(rect.attrs.r));
      assert.ok(top >= bottom, `the sign lies beside the label, not on the symbol: ${renderToString(rect)}`);
    }
    // The word ends where the signs begin, whatever the font's widths, so no sign covers it.
    assert.strictEqual(state?.attrs["text-anchor"], "end");
    const firstSign = findAll(alarm, (child) => child.tag === "rect")[0];
    assert.ok(
      Number(firstSign?.attrs.x) > Number(state?.attrs.x),
      `the sign starts after the word: ${renderToString(fault)}`,
    );
    const modes = draw(pump, {
      "P1.Running": on(false),
      "P1.Fault": on(false),
      "P1.Manual": on(true),
      "P1.OutOfService": on(true),
    });
    assert.ok(words(sign(modes, "manual")).includes("M"), renderToString(modes));
    assert.ok(words(sign(modes, "out-of-service")).includes("OOS"), renderToString(modes));
    // A sign has an edge that shows on the page, so it reads as a sign and not as loose text.
    for (const name of ["manual", "out-of-service"]) {
      assert.strictEqual(
        findAll(sign(modes, name)!, (child) => child.tag === "rect")[0]?.attrs.stroke,
        "var(--hmi-muted)",
        name,
      );
    }
    assert.ok(Number(part(modes, "symbol")?.attrs.opacity) < 1, "out of service dims the symbol");
  });

  test("an on/off valve shows where it stands: the ball's bore in line and the body hollow when open, across and filled when closed", () => {
    const valve = equipment("valve", { open: "V1.Open", closed: "V1.Closed", fault: "V1.Fault" });
    const shown = (open: boolean, closed: boolean, fault = false): SvgNode =>
      draw(valve, { "V1.Open": on(open), "V1.Closed": on(closed), "V1.Fault": on(fault) });
    const open = shown(true, false);
    assert.strictEqual(Number(part(open, "bore")?.attrs["data-angle"]), 0);
    assert.strictEqual(part(open, "body")?.attrs["data-filled"], "false");
    assert.strictEqual(words(part(open, "state")), "Open");
    const closed = shown(false, true);
    assert.strictEqual(Number(part(closed, "bore")?.attrs["data-angle"]), 90);
    assert.strictEqual(part(closed, "body")?.attrs["data-filled"], "true");
    assert.strictEqual(words(part(closed, "state")), "Closed");
    const moving = shown(false, false);
    assert.strictEqual(Number(part(moving, "bore")?.attrs["data-angle"]), 45);
    assert.strictEqual(words(part(moving, "state")), "Moving");
    const both = shown(true, true);
    assert.strictEqual(Number(part(both, "bore")?.attrs["data-angle"]), 45);
    assert.strictEqual(words(part(both, "state")), "Fault");
    assert.ok(sign(both, "fault"), renderToString(both));
    const onlyOpen = draw(equipment("valve", { open: "V1.Open" }), { "V1.Open": on(false) });
    assert.strictEqual(words(part(onlyOpen, "state")), "Closed");
    assert.deepStrictEqual(Object.keys(SYMBOLS.valve.ports).sort(), ["inlet", "outlet"]);
  });

  test("a control valve's plug stands at its position, the body is filled only when closed, and the state word gives the position", () => {
    const valve = equipment("control-valve", { position: "FV.Position" });
    const at = (position: number): SvgNode => draw(valve, { "FV.Position": on(position) });
    const closed = at(0);
    assert.strictEqual(Number(part(closed, "plug")?.attrs["data-lift"]), 0);
    assert.strictEqual(part(closed, "body")?.attrs["data-filled"], "true");
    assert.strictEqual(words(part(closed, "state")), "0 %");
    const partly = at(62);
    assert.strictEqual(Number(part(partly, "plug")?.attrs["data-lift"]), 0.62);
    assert.strictEqual(part(partly, "body")?.attrs["data-filled"], "false");
    assert.strictEqual(words(part(partly, "state")), "62 %");
    assert.strictEqual(Number(part(at(100), "plug")?.attrs["data-lift"]), 1);
    assert.strictEqual(
      Number(part(at(140), "plug")?.attrs["data-lift"]),
      1,
      "a position beyond the travel stops at its end",
    );
  });

  test("a symbol keeps its proportions in any size, centred above its label", () => {
    const [width, height] = SYMBOLS.pump.size;
    const wide = draw(equipment("pump", {}, { size: [width * 3, height + LABEL_HEIGHT] }), {});
    assert.strictEqual(part(wide, "symbol")?.attrs.transform, `translate(${width} 0) scale(1)`);
  });

  test("when the viewer's system asks for reduced motion, nothing turns", () => {
    const css = tokensCss({});
    assert.match(css, /@keyframes hmi-spin/);
    assert.match(css, /@media \(prefers-reduced-motion: reduce\)\s*\{[^}]*\.hmi-spin\s*\{[^}]*animation:\s*none/);
  });

  test("a new pump, valve or control valve starts with its symbol at its design size and room for its label", () => {
    const catalogue = JSON.parse(
      fs.readFileSync(path.join(ROOT, "crates/trust-runtime/src/hmi/catalogue/components.json"), "utf8"),
    ) as {
      components: Array<{ type: string; size: [number, number] }>;
    };
    for (const type of ["pump", "valve", "control-valve"] as const) {
      const [width, height] = SYMBOLS[type].size;
      const entry = catalogue.components.find((component) => component.type === type);
      assert.ok(entry, `the catalogue has the ${type}`);
      assert.ok(
        entry.size[0] >= width && entry.size[1] === height + LABEL_HEIGHT,
        `${type}: ${JSON.stringify(entry.size)}`,
      );
    }
  });
});
