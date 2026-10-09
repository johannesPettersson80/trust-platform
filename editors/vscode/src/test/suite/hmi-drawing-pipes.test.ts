import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { drawPage } from "../../hmiBuilder/drawing";
import type { DrawFrame, HmiObjectModel, HmiPageModel, HmiPipeModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { portPoint, routePipe } from "../../hmiBuilder/drawing/pipes";
import { findAll, SvgNode } from "../../hmiBuilder/drawing/svg";
import { SYMBOLS } from "../../hmiBuilder/drawing/symbols";

// Specification 39, §6: pipes join the ports of the symbols. The route is computed, orthogonal,
// around objects and with few bends, and follows the objects when they move; a pipe shows flow
// only from a binding that reports it.

const ROOT = path.resolve(__dirname, "../../../../..");

function equipment(
  id: string,
  type: string,
  at: [number, number],
  extra: Partial<HmiObjectModel> = {},
): HmiObjectModel {
  return { id, type, at, size: [128, 108], rotation: 0, bind: {}, ...extra };
}

type Point = [number, number];

function segments(points: Point[]): Array<[Point, Point]> {
  return points.slice(1).map((point, index) => [points[index], point]);
}

function crosses([a, b]: [Point, Point], box: { x: number; y: number; width: number; height: number }): boolean {
  const [x1, x2] = [Math.min(a[0], b[0]), Math.max(a[0], b[0])];
  const [y1, y2] = [Math.min(a[1], b[1]), Math.max(a[1], b[1])];
  return x1 < box.x + box.width && x2 > box.x && y1 < box.y + box.height && y2 > box.y;
}

function pathPoints(node: SvgNode): Point[] {
  return [...(node.attrs.d ?? "").matchAll(/(-?[\d.]+) (-?[\d.]+)/g)].map(
    (match) => [Number(match[1]), Number(match[2])] as Point,
  );
}

function page(objects: HmiObjectModel[], pipe: HmiPipeModel[]): HmiPageModel {
  return { title: "Pipes", object: objects, pipe };
}

function drawn(model: HmiPageModel, values: Record<string, LiveValue> = {}, frame: Partial<DrawFrame> = {}): SvgNode {
  return drawPage(model, { size: [1280, 720], values, ...frame });
}

suite("HMI drawing: pipes", () => {
  test("a symbol's ports lie on the symbol where the page places it, and the catalogue names the same ports", () => {
    const pump = equipment("p", "pump", [40, 40]);
    const inlet = portPoint(pump, "inlet");
    const symbol = SYMBOLS.pump;
    // A 128 × 108 object draws the 64-pixel symbol at scale 1, centred above its label.
    assert.deepStrictEqual(inlet, {
      at: [40 + 32 + symbol.ports.inlet.at[0], 40 + symbol.ports.inlet.at[1]],
      direction: "left",
    });
    assert.strictEqual(portPoint(pump, "outlet")?.direction, "right");
    assert.strictEqual(portPoint(pump, "shaft"), undefined);
    const catalogue = JSON.parse(
      fs.readFileSync(path.join(ROOT, "crates/trust-runtime/src/hmi/catalogue/components.json"), "utf8"),
    ) as {
      components: Array<{ type: string; ports?: string[] }>;
    };
    for (const component of catalogue.components.filter((entry) => (entry.ports ?? []).length > 0)) {
      const object = equipment("x", component.type, [0, 0], component.type === "junction" ? { size: [16, 16] } : {});
      for (const port of component.ports ?? [])
        assert.ok(portPoint(object, port), `${component.type}.${port} is drawn`);
    }
  });

  test("a tank inlet is approached from above without routing through its own body", () => {
    const valve = equipment("valve", "control-valve", [440, 380]);
    const tank = equipment("tank", "tank", [760, 272], { size: [160, 236] });
    const group = equipment("area", "group", [40, 112], { size: [1196, 452] });
    const svg = drawn(page([group, valve, tank], [{ from: "valve.outlet", to: "tank.inlet" }]));
    const line = findAll(svg, (node) => node.attrs["data-part"] === "line")[0];
    const route = pathPoints(line);
    const end = portPoint(tank, "inlet")!.at;
    assert.deepStrictEqual(route[route.length - 1], end);
    assert.ok(route[route.length - 2][1] < end[1], "the final inlet segment comes from above the tank");
    assert.ok(
      !segments(route).some((segment) => crosses(segment, { x: 760, y: 288, width: 160, height: 160 })),
      "the tank body remains an obstacle even though it owns the destination port",
    );
  });

  test("two ports facing each other on one line are joined by a straight pipe", () => {
    const route = routePipe(
      { at: [200, 100], direction: "right" },
      { at: [500, 100], direction: "left" },
      [],
      [],
      [1280, 720],
    );
    assert.deepStrictEqual(route, [
      [200, 100],
      [500, 100],
    ]);
  });

  test("a pipe goes around an object between its ends, orthogonally and with few bends, leaving each port in its direction", () => {
    const obstacle = { x: 300, y: 40, width: 120, height: 140 };
    const route = routePipe(
      { at: [200, 100], direction: "right" },
      { at: [560, 100], direction: "left" },
      [obstacle],
      [],
      [1280, 720],
    );
    assert.ok(route.length >= 2, `a route: ${JSON.stringify(route)}`);
    for (const segment of segments(route)) {
      assert.ok(
        segment[0][0] === segment[1][0] || segment[0][1] === segment[1][1],
        `orthogonal: ${JSON.stringify(segment)}`,
      );
      assert.ok(!crosses(segment, obstacle), `around the object: ${JSON.stringify(route)}`);
    }
    assert.ok(route.length - 2 <= 4, `few bends: ${JSON.stringify(route)}`);
    assert.ok(route[1][0] > route[0][0] && route[1][1] === route[0][1], "it leaves the start to the right");
    const [before, end] = route.slice(-2);
    assert.ok(before[0] < end[0] && before[1] === end[1], "it reaches the end from the left, the way that port faces");
  });

  test("a pipe passes through the bends the user placed, in order", () => {
    const route = routePipe(
      { at: [200, 100], direction: "right" },
      { at: [600, 300], direction: "left" },
      [],
      [
        [400, 100],
        [400, 300],
      ],
      [1280, 720],
    );
    const at = (point: Point): number =>
      route.findIndex((candidate) => candidate[0] === point[0] && candidate[1] === point[1]);
    assert.ok(at([400, 100]) > 0 && at([400, 300]) > at([400, 100]), JSON.stringify(route));
  });

  test("pipes are drawn under the objects and follow them when they move; a signal line is dashed", () => {
    const pump = equipment("p", "pump", [40, 40]);
    const valve = equipment("v", "valve", [400, 40]);
    const model = page(
      [pump, valve],
      [
        { from: "p.outlet", to: "v.inlet" },
        { from: "v.outlet", to: "p.inlet", style: "signal" },
      ],
    );
    const svg = drawn(model);
    const order = findAll(
      svg,
      (node) => node.attrs["data-pipe"] !== undefined || node.attrs["data-object"] !== undefined,
    ).map((node) => node.attrs["data-pipe"] ?? node.attrs["data-object"]);
    assert.deepStrictEqual(order, ["0", "1", "p", "v"], "pipes first, so the symbols lie over their ends");
    const line = (svg: SvgNode, index: number): SvgNode => {
      const group = findAll(svg, (node) => node.attrs["data-pipe"] === String(index))[0];
      assert.ok(group, `pipe ${index} is drawn`);
      const found = group.children.find((node) => node.attrs["data-part"] === "line");
      assert.ok(found, `pipe ${index} has its line`);
      return found;
    };
    assert.strictEqual(line(svg, 0).attrs["stroke-dasharray"], undefined, "a process line is solid");
    assert.ok(line(svg, 1).attrs["stroke-dasharray"], "a signal line is dashed");
    const last = (points: Point[]): Point | undefined => points[points.length - 1];
    const end = last(pathPoints(line(svg, 0)));
    assert.deepStrictEqual(end, portPoint(valve, "inlet")?.at);
    const moved = drawn(page([pump, { ...valve, at: [400, 240] }], model.pipe));
    assert.deepStrictEqual(
      last(pathPoints(line(moved, 0))),
      portPoint({ ...valve, at: [400, 240] }, "inlet")?.at,
      "the route follows the moved valve",
    );
  });

  test("a pipe shows flow only from a flow binding that is a current observation, in the flow's direction", () => {
    const pump = equipment("p", "pump", [40, 40], { bind: { running: { path: "P.Running", direction: "read" } } });
    const valve = equipment("v", "valve", [400, 40]);
    const bound = (flow?: string): HmiPageModel =>
      page(
        [pump, valve],
        [
          {
            from: "p.outlet",
            to: "v.inlet",
            ...(flow ? { bind: { flow: { path: flow, direction: "read" as const } } } : {}),
          },
        ],
      );
    const flow = (model: HmiPageModel, values: Record<string, LiveValue>): SvgNode | undefined =>
      findAll(drawn(model, values), (node) => node.attrs["data-part"] === "flow")[0];
    const running = { "P.Running": { value: true, state: "confirmed" as const } };
    assert.strictEqual(flow(bound(), running), undefined, "a running pump does not make flow");
    assert.match(flow(bound("F"), { F: { value: 4.2, state: "confirmed" } })?.attrs.class ?? "", /\bhmi-flow\b/);
    assert.match(
      flow(bound("F"), { F: { value: -4.2, state: "confirmed" } })?.attrs.class ?? "",
      /\bhmi-flow-back\b/,
      "a negative flow runs back",
    );
    assert.ok(flow(bound("F"), { F: { value: true, state: "confirmed" } }), "a TRUE flow signal flows");
    assert.strictEqual(flow(bound("F"), { F: { value: 0, state: "confirmed" } }), undefined, "no flow at zero");
    assert.strictEqual(
      flow(bound("F"), { F: { value: 4.2, state: "confirmed", staleMs: 9000 } }),
      undefined,
      "nothing moves on a stale value",
    );
    assert.strictEqual(flow(bound("F"), { F: { value: null, state: "unknown" } }), undefined);
    assert.strictEqual(
      flow(bound("F"), { F: { value: 4.2, state: "commanded", target: 4.2 } }),
      undefined,
      "nor on a value merely commanded",
    );
    const estimated = flow(bound("F"), { F: { value: 4.2, state: "estimated" } });
    assert.strictEqual(estimated?.attrs["data-estimated"], "true", "an estimated flow is drawn as estimated");
  });
});
