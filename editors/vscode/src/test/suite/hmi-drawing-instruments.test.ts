import * as assert from "assert";

import { drawPage } from "../../hmiBuilder/drawing";
import { attachOrigin } from "../../hmiBuilder/drawing/attachments";
import type { HmiObjectModel, HmiPageModel } from "../../hmiBuilder/drawing/model";
import { pipeRoutes } from "../../hmiBuilder/drawing/pipes";
import { findAll, renderToString, SvgNode } from "../../hmiBuilder/drawing/svg";
import { drawObject, part, stateWord, symbolObject, valuesOf, words } from "./symbolTestKit";

// Specification 39, §5.2 phase 2 (instrumentation), §5.3 and §6 item 5: ISA-5.1 instruments, a
// bubble with the letters of what they measure and their tag and value beside it; the redundant
// sensor pair with its disagreement; and an instrument's attachment to the equipment or pipe it
// measures, stored as a reference and drawn as a line to it.

/** An instrument at its catalogue size, tag and value beside the bubble. */
function instrument(type: string, properties: string[], extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  return symbolObject(type as never, properties, { size: [176, 56], ...extra });
}

function sign(node: SvgNode, name: string): SvgNode | undefined {
  return findAll(node, (child) => child.attrs["data-sign"] === name)[0];
}

/** Where the symbol ends on the right, in the object's pixels. */
function symbolRight(drawn: SvgNode): number {
  const symbol = part(drawn, "symbol");
  const match = /translate\(([\d.]+) ([\d.]+)\) scale\(([\d.]+)\)/.exec(String(symbol?.attrs.transform));
  assert.ok(match, renderToString(drawn));
  return Number(match[1]) + 48 * Number(match[3]);
}

suite("HMI drawing: instruments", () => {
  test("a temperature sensor is an ISA bubble with its letters; its tag and value stand beside it", () => {
    const drawn = drawObject(
      instrument("temperature-sensor", ["value"], { tag: "TT-101", unit: "°C", decimals: 1 }),
      valuesOf({ value: 62.43 }),
    );
    assert.strictEqual(words(part(drawn, "letters")), "TT", renderToString(drawn));
    assert.strictEqual(stateWord(drawn), "62.4 °C");
    const tag = part(drawn, "tag");
    assert.ok(Number(tag?.attrs.x) >= symbolRight(drawn), "the tag stands beside the bubble, not under it");
    assert.strictEqual(tag?.attrs["text-anchor"], "start");
  });

  test("each instrument carries the letters of what it measures (ISA-5.1)", () => {
    const letters: Array<[string, string]> = [
      ["level-sensor", "LT"],
      ["flow-sensor", "FT"],
      ["pressure-sensor", "PT"],
      ["position-sensor", "ZT"],
      ["speed-sensor", "ST"],
    ];
    for (const [type, expected] of letters) {
      assert.strictEqual(
        words(part(drawObject(instrument(type, ["value"]), valuesOf({ value: 1 })), "letters")),
        expected,
        type,
      );
    }
  });

  test("a loop controller is the ISA shared-display bubble with its tag's letters and loop number; it gives its measurement, its setpoint while they differ (the unit once), and manual as a sign", () => {
    // Its catalogue size leaves room beside the bubble for both numbers.
    const controller = instrument("controller", ["pv", "sp", "manual"], {
      tag: "TIC-101",
      unit: "°C",
      decimals: 1,
      size: [256, 56],
    });
    const drawn = drawObject(controller, valuesOf({ pv: 42, sp: 45, manual: false }));
    assert.ok(part(drawn, "frame"), `the square of a shared display: ${renderToString(drawn)}`);
    assert.strictEqual(words(part(drawn, "letters")), "TIC");
    assert.strictEqual(words(part(drawn, "loop")), "101");
    assert.strictEqual(stateWord(drawn), "42.0 → 45.0 °C", "one unit for the two");
    assert.strictEqual(
      stateWord(drawObject(controller, valuesOf({ pv: 45, sp: 45, manual: false }))),
      "45.0 °C",
      "on its setpoint, the measurement alone",
    );
    assert.strictEqual(sign(drawn, "manual"), undefined);
    assert.ok(
      sign(drawObject(controller, valuesOf({ pv: 42, sp: 45, manual: true })), "manual"),
      "manual is the M sign",
    );
  });

  test("a sensor fault says Fault with its sign; an unknown value is ? on a hatched bubble; a stale one keeps its value with its age", () => {
    const sensor = instrument("pressure-sensor", ["value", "fault"], { unit: "bar", decimals: 1 });
    const fault = drawObject(sensor, valuesOf({ value: 3.2, fault: true }));
    assert.strictEqual(stateWord(fault), "Fault");
    assert.ok(sign(fault, "fault"), renderToString(fault));
    const unknown = drawObject(sensor, valuesOf({ value: { value: null, state: "unknown" }, fault: false }));
    assert.strictEqual(stateWord(unknown), "?");
    assert.match(String(part(unknown, "bubble")?.attrs.fill), /url\(#hmi-unknown-hatch\)/);
    const stale = drawObject(
      sensor,
      valuesOf({ value: { value: 3.2, state: "confirmed", staleMs: 12_000 }, fault: false }),
    );
    assert.strictEqual(stateWord(stale), "3.2 bar");
    assert.ok(sign(stale, "stale"));
  });

  test("a redundant sensor pair shows both values, and its disagreement as a fault: Disagree with its sign and a mark between the bubbles", () => {
    const pair = instrument("sensor-pair", ["a", "b", "disagree"], { unit: "°C", decimals: 1, size: [280, 56] });
    assert.strictEqual(
      stateWord(drawObject(pair, valuesOf({ a: 61.2, b: 61.4, disagree: false }))),
      "61.2 °C · 61.4 °C",
    );
    const disagreeing = drawObject(pair, valuesOf({ a: 61.2, b: 68.9, disagree: true }));
    assert.strictEqual(stateWord(disagreeing), "Disagree");
    assert.ok(sign(disagreeing, "fault"), renderToString(disagreeing));
    assert.ok(part(disagreeing, "disagreement"));
  });

  test("a loop controller's signal lines run from its instrument and to its equipment, dashed and around the other objects (§6 item 5)", () => {
    const valve: HmiObjectModel = {
      id: "tv-101",
      type: "control-valve",
      at: [700, 300],
      size: [128, 108],
      rotation: 0,
      bind: {},
    };
    const sensor = instrument("temperature-sensor", [], { id: "tt-101", at: [100, 420] });
    // A pump stands between the controller and the valve: the line goes round it.
    const pump: HmiObjectModel = { id: "p-101", type: "pump", at: [430, 110], size: [128, 108], rotation: 0, bind: {} };
    const controller: HmiObjectModel = {
      id: "tic-101",
      type: "controller",
      at: [100, 120],
      size: [256, 56],
      rotation: 0,
      bind: {},
      measures: "tt-101",
      acts_on: "tv-101",
    };
    const svg = drawPage(
      { title: "Loop", object: [valve, sensor, pump, controller], pipe: [] },
      { size: [1280, 720], values: {} },
    );
    const line = (id: string): Array<[number, number]> => {
      const node = findAll(svg, (candidate) => candidate.attrs["data-signal"] === id)[0];
      assert.ok(node, `the ${id} line is drawn`);
      assert.strictEqual(node.attrs["stroke-dasharray"], "6 5", `${id} is a signal line`);
      return String(node.attrs.points)
        .trim()
        .split(/\s+/)
        .map((pair) => pair.split(",").map(Number) as [number, number]);
    };
    const orthogonal = (points: Array<[number, number]>) =>
      points.slice(1).every(([x, y], index) => x === points[index][0] || y === points[index][1]);
    const measured = line("tic-101:measures");
    assert.ok(orthogonal(measured), JSON.stringify(measured));
    const bubble = attachOrigin(sensor);
    assert.deepStrictEqual(
      measured[0],
      [bubble.centre[0], bubble.centre[1] - bubble.radius],
      "it leaves the transmitter's bubble at its top, facing the controller",
    );
    const driven = line("tic-101:acts_on");
    assert.ok(orthogonal(driven), JSON.stringify(driven));
    const last = driven[driven.length - 1];
    assert.ok(
      last[0] === 732 || last[0] === 796 || last[1] === 300,
      `it ends on the valve's side: ${JSON.stringify(last)}`,
    );
    const crossesPump = driven.slice(1).some(([x, y], index) => {
      const [px, py] = driven[index];
      const [left, right] = [Math.min(px, x), Math.max(px, x)];
      const [top, bottom] = [Math.min(py, y), Math.max(py, y)];
      return right > 430 && left < 558 && bottom > 110 && top < 218;
    });
    assert.ok(!crossesPump, `the line goes round the pump: ${JSON.stringify(driven)}`);
  });

  test("an instrument attached to equipment or a pipe is joined to it by an orthogonal line from its bubble, and follows it", () => {
    const pump: HmiObjectModel = { id: "p-101", type: "pump", at: [400, 200], size: [128, 108], rotation: 0, bind: {} };
    const sensor = instrument("temperature-sensor", [], { id: "tt-101", at: [180, 40], attach: "p-101" });
    const line = (page: HmiPageModel): SvgNode | undefined =>
      findAll(
        drawPage(page, { size: [1280, 720], values: {} }),
        (node) => node.attrs["data-attachment"] === "tt-101",
      )[0];
    const points = (node: SvgNode | undefined): Array<[number, number]> =>
      String(node?.attrs.points ?? "")
        .trim()
        .split(/\s+/)
        .map((pair) => pair.split(",").map(Number) as [number, number]);
    const end = (node: SvgNode | undefined): [number, number] => points(node)[points(node).length - 1];
    const orthogonal = (node: SvgNode | undefined): boolean =>
      points(node)
        .slice(1)
        .every((point, index) => point[0] === points(node)[index][0] || point[1] === points(node)[index][1]);
    const drawn = line({ title: "A", object: [pump, sensor], pipe: [] });
    assert.ok(drawn, "the attachment is drawn");
    // The bubble (208, 68) is above and left of the pump's symbol, (432, 200) to (496, 264): the
    // line goes down to the symbol's middle, then across into the middle of its left side.
    assert.deepStrictEqual(points(drawn), [
      [208, 68],
      [208, 232],
      [432, 232],
    ]);
    const moved = line({ title: "A", object: [{ ...pump, at: [400, 400] }, sensor], pipe: [] });
    assert.deepStrictEqual(end(moved), [432, 432], "it follows the moved pump");
    // Above the symbol, the line drops straight onto its top.
    const above = line({ title: "A", object: [pump, { ...sensor, at: [420, 40] }], pipe: [] });
    assert.ok(orthogonal(above) && points(above).length === 2, JSON.stringify(points(above)));
    assert.strictEqual(end(above)[1], 200);
    const valve: HmiObjectModel = {
      id: "v-101",
      type: "valve",
      at: [880, 176],
      size: [128, 108],
      rotation: 0,
      bind: {},
    };
    const piped: HmiPageModel = {
      title: "A",
      object: [pump, valve, { ...sensor, at: [600, 60], attach: "pipe:0" }],
      pipe: [{ from: "p-101.outlet", to: "v-101.inlet" }],
    };
    const onPipe = end(line(piped));
    const route = pipeRoutes(piped, [1280, 720])[0] ?? [];
    const onRoute = route.slice(1).some((point, index) => {
      const from = route[index];
      return (
        (from[0] === point[0] &&
          onPipe[0] === from[0] &&
          onPipe[1] >= Math.min(from[1], point[1]) &&
          onPipe[1] <= Math.max(from[1], point[1])) ||
        (from[1] === point[1] &&
          onPipe[1] === from[1] &&
          onPipe[0] >= Math.min(from[0], point[0]) &&
          onPipe[0] <= Math.max(from[0], point[0]))
      );
    });
    assert.ok(onRoute, `the line ends on the pipe: ${JSON.stringify(onPipe)} on ${JSON.stringify(route)}`);
  });
});

type Spot = [number, number];
type Piece = [Spot, Spot];

/** Where an attachment line ends on the page. */
function attachmentEnd(svg: SvgNode, id: string): Spot {
  const line = findAll(svg, (node) => node.attrs["data-attachment"] === id)[0];
  assert.ok(line, `the attachment of ${id} is drawn`);
  const points = String(line.attrs.points)
    .trim()
    .split(/\s+/)
    .map((pair) => pair.split(",").map(Number) as Spot);
  return points[points.length - 1];
}

/** How an object's drawn symbol is placed on the page: the object's and the symbol's transforms. */
function symbolOnPage(svg: SvgNode, id: string): { object: SvgNode; onPage: (point: Spot) => Spot; scale: number } {
  const object = findAll(svg, (node) => node.attrs["data-object"] === id)[0];
  assert.ok(object, `${id} is drawn`);
  const placed = /translate\(([-\d.]+) ([-\d.]+)\)/.exec(object.attrs.transform ?? "");
  const symbol = /translate\(([-\d.]+) ([-\d.]+)\) scale\(([-\d.]+)\)/.exec(
    part(object, "symbol")?.attrs.transform ?? "",
  );
  assert.ok(placed && symbol, renderToString(object));
  const [x, y, left, top, scale] = [placed[1], placed[2], symbol[1], symbol[2], symbol[3]].map(Number);
  return { object, scale, onPage: ([px, py]) => [x + left + px * scale, y + top + py * scale] };
}

/** A drawn part's path on the page as short straight pieces: its moves, lines and quadratic curves. */
function drawnPieces(svg: SvgNode, id: string, name: string): Piece[] {
  const { object, onPage } = symbolOnPage(svg, id);
  const d = part(object, name)?.attrs.d;
  assert.ok(d, `${id} draws its ${name}`);
  const tokens = d.match(/[A-Za-z]|-?\d+(?:\.\d+)?/g) ?? [];
  const pieces: Piece[] = [];
  let at: Spot = [0, 0];
  const to = (next: Spot): void => {
    pieces.push([onPage(at), onPage(next)]);
    at = next;
  };
  for (let index = 0; index < tokens.length;) {
    const command = tokens[index++];
    const take = (): number => Number(tokens[index++]);
    if (command === "M") at = [take(), take()];
    else if (command === "L") to([take(), take()]);
    else if (command === "H") to([take(), at[1]]);
    else if (command === "V") to([at[0], take()]);
    else if (command === "Q") {
      const [control, end, from]: [Spot, Spot, Spot] = [[take(), take()], [take(), take()], at];
      for (let step = 1; step <= 64; step += 1) {
        const t = step / 64;
        to([
          (1 - t) ** 2 * from[0] + 2 * (1 - t) * t * control[0] + t * t * end[0],
          (1 - t) ** 2 * from[1] + 2 * (1 - t) * t * control[1] + t * t * end[1],
        ]);
      }
    } else assert.fail(`the test reads no "${command}" in ${d}`);
  }
  return pieces;
}

function distanceTo(point: Spot, pieces: Piece[]): number {
  return Math.min(
    ...pieces.map(([a, b]) => {
      const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
      const length = dx * dx + dy * dy;
      const t = length === 0 ? 0 : Math.max(0, Math.min(1, ((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length));
      return Math.hypot(point[0] - (a[0] + t * dx), point[1] - (a[1] + t * dy));
    }),
  );
}

suite("HMI drawing: an attached instrument meets the drawn outline (§21.6)", () => {
  const sensor = (at: Spot, attach: string): HmiObjectModel =>
    instrument("level-sensor", [], { id: "lt-101", tag: "LT-101", at, attach });

  test("a level sensor's line ends on the tank's drawn wall, not on the box around it, for two tank sizes", () => {
    for (const size of [
      [200, 260],
      [128, 172],
    ] as Spot[]) {
      const tank: HmiObjectModel = { id: "tk-101", type: "tank", at: [400, 200], size, rotation: 0, bind: {} };
      // Beside the tank, above and to the left of it, and under it.
      for (const at of [
        [150, 240],
        [150, 20],
        [440, 520],
      ] as Spot[]) {
        const svg = drawPage(
          { title: "A", object: [tank, sensor(at, "tk-101")], pipe: [] },
          { size: [1280, 720], values: {} },
        );
        const end = attachmentEnd(svg, "lt-101");
        const away = distanceTo(end, drawnPieces(svg, "tk-101", "walls"));
        assert.ok(
          away <= 0.5,
          `a ${size.join(" × ")} tank, the sensor at ${at.join(", ")}: the line ends ${away.toFixed(2)} px from the drawn wall, at ${end.join(", ")}`,
        );
      }
    }
  });

  test("on a closed vessel the line meets its dished head where it is drawn", () => {
    const vessel: HmiObjectModel = {
      id: "v-101",
      type: "vessel",
      at: [400, 200],
      size: [128, 172],
      rotation: 0,
      bind: {},
    };
    const svg = drawPage(
      { title: "A", object: [vessel, sensor([420, 20], "v-101")], pipe: [] },
      { size: [1280, 720], values: {} },
    );
    const end = attachmentEnd(svg, "lt-101");
    // The top head is a half ellipse over the shell's top, as the drawn outline's first arc says.
    const { object, onPage, scale } = symbolOnPage(svg, "v-101");
    const head = /^M([\d.]+) ([\d.]+) A([\d.]+) ([\d.]+) 0 0 1 ([\d.]+) ([\d.]+)/.exec(
      part(object, "vessel")?.attrs.d ?? "",
    );
    assert.ok(head, renderToString(object));
    const [startX, shellY, rx, ry, endX] = [head[1], head[2], head[3], head[4], head[5]].map(Number);
    const [cx, cy] = onPage([(startX + endX) / 2, shellY]);
    const across = (end[0] - cx) / (rx * scale);
    assert.ok(Math.abs(across) < 1, `the line comes down onto the head: ${end.join(", ")}`);
    const drawnY = cy - ry * scale * Math.sqrt(1 - across * across);
    assert.ok(
      Math.abs(end[1] - drawnY) <= 0.5,
      `the line ends at ${end[1]}, the head is drawn at ${drawnY.toFixed(2)}`,
    );
  });
});
