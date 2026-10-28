import * as assert from "assert";

import type { HmiObjectModel, HmiPageModel } from "../../hmiBuilder/drawing/model";
import { pipeRoutes, portPoint } from "../../hmiBuilder/drawing/pipes";
import { deleteObjects } from "../../hmiBuilder/editor/editing";
import { addPipe, bendPipe, branchPipe, deletePipes, pipeAt, portAt, splitPipe } from "../../hmiBuilder/editor/pipes";

// Specification 39, §6: pipes are drawn between ports in the editor, bent by hand, split by a
// valve dropped onto them, and go with the objects they join.

const PORTS: Record<string, string[]> = {
  pump: ["inlet", "outlet"],
  valve: ["inlet", "outlet"],
  "control-valve": ["inlet", "outlet"],
  junction: ["left", "right", "top", "bottom"],
};

function equipment(id: string, type: string, at: [number, number]): HmiObjectModel {
  return { id, type, at, size: [128, 108], rotation: 0, bind: {} };
}

function page(objects: HmiObjectModel[], pipe: HmiPageModel["pipe"] = []): HmiPageModel {
  return { title: "Pipes", object: objects, pipe };
}

const PUMP = equipment("p-101", "pump", [40, 40]);
const VALVE = equipment("v-101", "valve", [480, 40]);

suite("HMI Builder editor: pipes", () => {
  test("a pipe joins two free ports; a port that has a pipe, or the same port twice, is refused", () => {
    const joined = addPipe(page([PUMP, VALVE]), "p-101.outlet", "v-101.inlet");
    assert.deepStrictEqual(joined.pipe, [{ from: "p-101.outlet", to: "v-101.inlet" }]);
    assert.strictEqual(addPipe(joined, "p-101.outlet", "v-101.outlet"), joined, "the outlet already has a pipe");
    assert.strictEqual(addPipe(joined, "v-101.outlet", "v-101.outlet"), joined, "a port to itself");
  });

  test("deleting an object deletes its pipes; deleting a pipe leaves the objects", () => {
    const joined = page(
      [PUMP, VALVE],
      [
        { from: "p-101.outlet", to: "v-101.inlet" },
        { from: "v-101.outlet", to: "p-101.inlet" },
      ],
    );
    assert.deepStrictEqual(deleteObjects(joined, ["v-101"]).pipe, []);
    const fewer = deletePipes(joined, ["pipe:0"]);
    assert.deepStrictEqual(fewer.pipe, [{ from: "v-101.outlet", to: "p-101.inlet" }]);
    assert.strictEqual(fewer.object.length, 2);
  });

  test("the port under the pointer is found within a few pixels", () => {
    const model = page([PUMP, VALVE]);
    const outlet = portPoint(PUMP, "outlet")!.at;
    assert.strictEqual(portAt(model, [outlet[0] + 3, outlet[1] - 2], PORTS), "p-101.outlet");
    assert.strictEqual(portAt(model, [outlet[0] + 30, outlet[1]], PORTS), undefined);
  });

  test("the pipe under the pointer is found along its route", () => {
    const model = page([PUMP, VALVE], [{ from: "p-101.outlet", to: "v-101.inlet" }]);
    const outlet = portPoint(PUMP, "outlet")!.at;
    const inlet = portPoint(VALVE, "inlet")!.at;
    // The route leaves the outlet to the right; a point a little along it is on the pipe.
    assert.strictEqual(pipeAt(model, [outlet[0] + 10, outlet[1] + 2], [1280, 720]), "pipe:0");
    assert.strictEqual(pipeAt(model, [(outlet[0] + inlet[0]) / 2, 600], [1280, 720]), undefined);
  });

  test("bending a pipe keeps the bend: the dragged run is stored as two waypoints, clear of the objects, and the pipe runs through them without detours", () => {
    // The valve's inlet lies on the pump outlet's line, so the pipe runs straight.
    const valve = equipment("v-101", "valve", [480, 16]);
    const model = page([PUMP, valve], [{ from: "p-101.outlet", to: "v-101.inlet" }]);
    const outlet = portPoint(PUMP, "outlet")!.at;
    const inlet = portPoint(valve, "inlet")!.at;
    const bent = bendPipe(model, "pipe:0", [outlet[0] + 100, outlet[1]], [outlet[0] + 100, 302], [1280, 720]);
    // The run's ends lie a clearance beyond the pump's box and before the valve's (§6).
    const [right, left] = [40 + 128 + 8, 480 - 8];
    assert.deepStrictEqual(
      bent.pipe[0].waypoints,
      [
        [right, 304],
        [left, 304],
      ],
      "the run moved to the grid line at y 304, its ends clear of the objects",
    );
    assert.deepStrictEqual(
      pipeRoutes(bent, [1280, 720])[0],
      [outlet, [right, outlet[1]], [right, 304], [left, 304], [left, inlet[1]], inlet],
      "down, along and up",
    );
  });

  test("a valve dropped onto a pipe splits it into two pipes through the valve", () => {
    // The valves' pipe axis lies on the pump outlet's line, so the pipe runs straight through.
    const pump = equipment("p-101", "pump", [40, 40]);
    const valve = equipment("v-101", "valve", [880, 16]);
    const inline = equipment("v-102", "valve", [400, 16]);
    const model = page(
      [pump, valve, inline],
      [
        {
          from: "p-101.outlet",
          to: "v-101.inlet",
          style: "signal",
          bind: { flow: { path: "Plant.FT101", direction: "read" } },
        },
      ],
    );
    const split = splitPipe(model, "v-102", PORTS, [1280, 720]);
    assert.deepStrictEqual(split.pipe, [
      {
        from: "p-101.outlet",
        to: "v-102.inlet",
        style: "signal",
        bind: { flow: { path: "Plant.FT101", direction: "read" } },
      },
      { from: "v-102.outlet", to: "v-101.inlet", style: "signal" },
    ]);
    const away = page([pump, valve, equipment("v-102", "valve", [400, 400])], model.pipe);
    assert.strictEqual(splitPipe(away, "v-102", PORTS, [1280, 720]), away, "a valve away from the pipe splits nothing");
  });

  // A pump's outlet and a valve's inlet on one line, at y 60: a straight pipe from x 136 to 912.
  const line = (objects: HmiObjectModel[] = []): HmiPageModel =>
    page(
      [PUMP, equipment("v-101", "valve", [880, 16]), ...objects],
      [{ from: "p-101.outlet", to: "v-101.inlet", bind: { flow: { path: "Plant.FT101", direction: "read" } } }],
    );

  test("a valve dropped a few pixels off a pipe is moved onto it, so the pipe stays straight", () => {
    const split = splitPipe(line([equipment("v-102", "valve", [400, 20])]), "v-102", PORTS, [1280, 720]);
    assert.deepStrictEqual(split.object.find((object) => object.id === "v-102")?.at, [400, 16]);
    assert.deepStrictEqual(
      split.pipe.map((pipe) => [pipe.from, pipe.to]),
      [
        ["p-101.outlet", "v-102.inlet"],
        ["v-102.outlet", "v-101.inlet"],
      ],
    );
  });

  test("a junction dropped onto a pipe splits it through the junction's two sides along the pipe", () => {
    const junction: HmiObjectModel = {
      id: "j-1",
      type: "junction",
      at: [400, 54],
      size: [16, 16],
      rotation: 0,
      bind: {},
    };
    const split = splitPipe(line([junction]), "j-1", PORTS, [1280, 720]);
    assert.deepStrictEqual(split.object.find((object) => object.id === "j-1")?.at, [400, 52], "centred on the pipe");
    assert.deepStrictEqual(split.pipe, [
      { from: "p-101.outlet", to: "j-1.left", bind: { flow: { path: "Plant.FT101", direction: "read" } } },
      { from: "j-1.right", to: "v-101.inlet" },
    ]);
  });

  test("splitting a bent pipe keeps each bend on the part it lies on", () => {
    // The pipe bent down to y 304, as the bend test leaves it; a junction dropped on that run.
    const valve = equipment("v-101", "valve", [480, 16]);
    const junction: HmiObjectModel = {
      id: "j-1",
      type: "junction",
      at: [312, 296],
      size: [16, 16],
      rotation: 0,
      bind: {},
    };
    const bent = page(
      [PUMP, valve, junction],
      [
        {
          from: "p-101.outlet",
          to: "v-101.inlet",
          waypoints: [
            [176, 304],
            [472, 304],
          ],
        },
      ],
    );
    assert.deepStrictEqual(splitPipe(bent, "j-1", PORTS, [1280, 720]).pipe, [
      { from: "p-101.outlet", to: "j-1.left", waypoints: [[176, 304]] },
      { from: "j-1.right", to: "v-101.inlet", waypoints: [[472, 304]] },
    ]);
  });

  test("a pipe drawn from a port onto another pipe ends on a new junction there, which splits that pipe", () => {
    // A second pump below the line; its pipe is released on the line at x 600.
    const model = line([equipment("p-102", "pump", [400, 300])]);
    const branched = branchPipe(model, "p-102.outlet", [600, 63], PORTS, [1280, 720]);
    assert.deepStrictEqual(
      branched.object.find((object) => object.type === "junction"),
      { id: "junction-1", type: "junction", at: [592, 52], size: [16, 16], rotation: 0, bind: {} },
    );
    assert.deepStrictEqual(
      branched.pipe,
      [
        { from: "p-101.outlet", to: "junction-1.left", bind: { flow: { path: "Plant.FT101", direction: "read" } } },
        { from: "junction-1.right", to: "v-101.inlet" },
        { from: "p-102.outlet", to: "junction-1.bottom" },
      ],
      "the branch enters from below, the side its pump is on",
    );
    assert.strictEqual(
      branchPipe(model, "p-102.outlet", [600, 400], PORTS, [1280, 720]),
      model,
      "released away from any pipe",
    );
    assert.strictEqual(
      branchPipe(model, "p-101.outlet", [600, 63], PORTS, [1280, 720]),
      model,
      "a port that already has a pipe",
    );
  });
});

// Catalogue §7: later pipe edits must preserve unrelated bends and endpoint bindings.
suite("HMI Builder pipes: preserve manual routing", () => {
  test("moving a run keeps bends on both sides", () => {
    const model: HmiPageModel = {
      title: "Route",
      object: [equipment("p", "pump", [8, 28]), equipment("v", "valve", [880, 16])],
      pipe: [
        {
          from: "p.outlet",
          to: "v.inlet",
          waypoints: [
            [176, 160],
            [320, 160],
            [320, 304],
            [600, 304],
            [600, 160],
            [800, 160],
          ],
        },
      ],
    };
    const routes = pipeRoutes(model, [1280, 720]);
    const route = routes[0];
    assert.ok(route, "fixture has a routed pipe");
    const segment = route.findIndex(
      (point, index) => index + 1 < route.length && point[1] === 304 && route[index + 1][1] === 304,
    );
    assert.ok(segment >= 0, "fixture includes the authored middle run");
    const a = route[segment],
      b = route[segment + 1];
    const changed = bendPipe(model, "pipe:0", [(a[0] + b[0]) / 2, 304], [(a[0] + b[0]) / 2, 368], [1280, 720]);
    assert.deepStrictEqual(changed.pipe[0].waypoints?.slice(0, segment - 1), route.slice(1, segment));
    assert.ok(changed.pipe[0].waypoints?.some((point) => point[1] === 368));
    assert.strictEqual(changed.pipe[0].from, "p.outlet");
    assert.strictEqual(changed.pipe[0].to, "v.inlet");
    assert.deepStrictEqual(model.pipe[0].waypoints?.[2], [320, 304], "Undo base stays immutable");
  });
});
