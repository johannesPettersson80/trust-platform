import * as assert from "assert";
import { editAgentPage } from "../../hmiBuilder/agent/edits";
import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
const catalogue = {
  format: 1,
  components: [
    {
      type: "tank",
      category: "equipment",
      phase: 2,
      size: [100, 100] as [number, number],
      ports: ["inlet", "outlet"],
      properties: { level: { accepts: ["number"], directions: ["read"] } },
    },
  ],
};
const base = (): HmiPageModel => ({
  title: "Process",
  object: [
    {
      id: "a",
      type: "tank",
      at: [40, 40],
      size: [100, 100],
      rotation: 0,
      bind: { level: { path: "Main.Level", direction: "read" } },
      label: "Keep me",
    },
    { id: "b", type: "tank", at: [240, 80], size: [100, 100], rotation: 0, bind: {} },
  ],
  pipe: [{ from: "a.outlet", to: "b.inlet" }],
});
suite("HMI agent semantic editing", () => {
  test("targeted moves preserve bindings, labels, identities and other objects", () => {
    const original = base();
    const changed = editAgentPage(original, [{ op: "move", ids: ["a"], delta: [16, 24] }], catalogue);
    assert.deepStrictEqual(changed.object[0].at, [56, 64]);
    assert.deepStrictEqual(changed.object[0].bind, original.object[0].bind);
    assert.strictEqual(changed.object[0].label, "Keep me");
    assert.deepStrictEqual(changed.object[1], original.object[1]);
    assert.deepStrictEqual(original, base());
  });
  test("batch failure leaves the source unchanged and pinned objects cannot move", () => {
    const original = base();
    assert.throws(
      () =>
        editAgentPage(
          original,
          [
            { op: "move", ids: ["a"], delta: [8, 8] },
            { op: "move", ids: ["missing"], delta: [8, 8] },
          ],
          catalogue,
        ),
      /unknown object/i,
    );
    assert.deepStrictEqual(original, base());
    original.object[0].locked = true;
    assert.throws(() => editAgentPage(original, [{ op: "resize", id: "a", size: [80, 80] }], catalogue), /locked/i);
  });
  test("deleting connected equipment requires explicit dependent removal", () => {
    assert.throws(() => editAgentPage(base(), [{ op: "remove", ids: ["a"] }], catalogue), /dependent/i);
    const changed = editAgentPage(base(), [{ op: "remove", ids: ["a"], removeDependents: true }], catalogue);
    assert.deepStrictEqual(
      changed.object.map((object) => object.id),
      ["b"],
    );
    assert.deepStrictEqual(changed.pipe, []);
  });
  test("port connections and binding directions respect the catalogue", () => {
    assert.throws(() => editAgentPage(base(), [{ op: "connect", from: "a.fake", to: "b.inlet" }], catalogue), /port/i);
    assert.throws(
      () =>
        editAgentPage(
          base(),
          [{ op: "bind", id: "a", property: "level", binding: { path: "Main.Level", direction: "write" } }],
          catalogue,
        ),
      /direction/i,
    );
    const changed = editAgentPage(
      base(),
      [{ op: "bind", id: "a", property: "level", binding: { path: "Main.Other", direction: "read" } }],
      catalogue,
    );
    assert.strictEqual(changed.object[0].bind.level.path, "Main.Other");
  });
  test("align and group reuse editor geometry without changing pipes", () => {
    const changed = editAgentPage(
      base(),
      [
        { op: "align", ids: ["a", "b"], edge: "top" },
        { op: "group", ids: ["a", "b"], group: "process" },
      ],
      catalogue,
    );
    assert.strictEqual(changed.object[1].at[1], 40);
    assert.strictEqual(changed.object[0].group, "process");
    assert.deepStrictEqual(changed.pipe, base().pipe);
  });
  test("duplicate carries internal pipes and rejects duplicate ids", () => {
    const changed = editAgentPage(
      base(),
      [{ op: "duplicate", ids: ["a", "b"], newIds: ["c", "d"], delta: [0, 200] }],
      catalogue,
    );
    assert.strictEqual(changed.object.length, 4);
    assert.ok(changed.pipe.some((pipe) => pipe.from === "c.outlet" && pipe.to === "d.inlet"));
    assert.throws(
      () => editAgentPage(base(), [{ op: "duplicate", ids: ["a"], newIds: ["b"], delta: [0, 200] }], catalogue),
      /id/i,
    );
  });
  test("equipment and pipe deletion keep surviving instrument attachments on their original pipes", () => {
    const original = base();
    original.object.push(
      { id: "c", type: "tank", at: [440, 80], size: [100, 100], rotation: 0, bind: {} },
      { id: "sensor", type: "tank", at: [400, 240], size: [80, 80], rotation: 0, attach: "pipe:1", bind: {} },
    );
    original.pipe.push({ from: "b.outlet", to: "c.inlet" });
    const removed = editAgentPage(original, [{ op: "remove", ids: ["a"], removeDependents: true }], catalogue);
    assert.strictEqual(removed.object.find((object) => object.id === "sensor")?.attach, "pipe:0");
    const disconnected = editAgentPage(original, [{ op: "disconnect", from: "a.outlet", to: "b.inlet" }], catalogue);
    assert.strictEqual(disconnected.object.find((object) => object.id === "sensor")?.attach, "pipe:0");
    assert.strictEqual(original.object.find((object) => object.id === "sensor")?.attach, "pipe:1");
  });
  test("duplicating a connected group rebinds its pipe-attached instruments to the copied pipe", () => {
    const original = base();
    original.object.push({
      id: "sensor",
      type: "tank",
      at: [180, 240],
      size: [80, 80],
      rotation: 0,
      attach: "pipe:0",
      bind: {},
    });
    const copied = editAgentPage(
      original,
      [{ op: "duplicate", ids: ["a", "b", "sensor"], newIds: ["c", "d", "sensor-copy"], delta: [0, 320] }],
      catalogue,
    );
    assert.strictEqual(copied.object.find((object) => object.id === "sensor-copy")?.attach, "pipe:1");
    assert.strictEqual(copied.object.find((object) => object.id === "sensor")?.attach, "pipe:0");
  });

  test("pipe reroute and reconnection preserve bindings, style and attachment identity", () => {
    const original = base();
    original.pipe[0].style = "signal";
    original.pipe[0].bind = { flow: { path: "Main.Flow", direction: "read" } };
    original.object.push(
      { id: "c", type: "tank", at: [440, 80], size: [100, 100], rotation: 0, bind: {} },
      { id: "sensor", type: "tank", at: [400, 240], size: [80, 80], rotation: 0, attach: "pipe:0", bind: {} },
    );
    const changed = editAgentPage(
      original,
      [
        {
          op: "reroute",
          from: "a.outlet",
          to: "b.inlet",
          waypoints: [
            [200, 200],
            [320, 200],
          ],
        },
        { op: "reconnect", from: "a.outlet", to: "b.inlet", newFrom: "a.outlet", newTo: "c.inlet" },
      ],
      catalogue,
    );
    assert.deepStrictEqual(changed.pipe[0], {
      ...original.pipe[0],
      to: "c.inlet",
      waypoints: [
        [200, 200],
        [320, 200],
      ],
    });
    assert.strictEqual(changed.object.find((object) => object.id === "sensor")?.attach, "pipe:0");
    const automatic = editAgentPage(
      changed,
      [{ op: "reroute", from: "a.outlet", to: "c.inlet", waypoints: [] }],
      catalogue,
    );
    assert.strictEqual(automatic.pipe[0].waypoints, undefined);
    assert.throws(
      () => editAgentPage(original, [{ op: "connect", from: "a.outlet", to: "a.outlet" }], catalogue),
      /same port|self/i,
    );
    assert.throws(
      () =>
        editAgentPage(
          original,
          [{ op: "reconnect", from: "a.outlet", to: "b.inlet", newFrom: "a.nope", newTo: "c.inlet" }],
          catalogue,
        ),
      /port/i,
    );
    assert.strictEqual(original.pipe[0].to, "b.inlet");
  });
  test("equal sizing uses the reference and refuses locked or incompatible faceplate geometry", () => {
    const original = base();
    original.object[1].size = [160, 80];
    const changed = editAgentPage(
      original,
      [{ op: "equal-size", ids: ["a"], reference: "b", axis: "both" }],
      catalogue,
    );
    assert.deepStrictEqual(changed.object[0].size, [160, 80]);
    assert.deepStrictEqual(changed.object[0].bind, original.object[0].bind);
    original.object[0].type = "faceplate";
    assert.throws(
      () => editAgentPage(original, [{ op: "equal-size", ids: ["a"], reference: "b", axis: "both" }], catalogue),
      /proportion/i,
    );
    const width = editAgentPage(original, [{ op: "equal-size", ids: ["a"], reference: "b", axis: "width" }], catalogue);
    assert.deepStrictEqual(width.object[0].size, [160, 160]);
    original.object[0].locked = true;
    assert.throws(
      () => editAgentPage(original, [{ op: "equal-size", ids: ["a"], reference: "b", axis: "width" }], catalogue),
      /locked/i,
    );
  });
  test("regional arrangement preserves pinned and unselected objects and avoids their space", () => {
    const original = base();
    original.object[0].at = [32, 88];
    original.object[0].size = [96, 96];
    original.object[0].locked = true;
    original.object[1].size = [96, 96];
    original.object.push({ id: "obstacle", type: "tank", at: [152, 88], size: [96, 96], rotation: 0, bind: {} });
    const changed = Reflect.apply(editAgentPage, undefined, [
      original,
      [
        {
          op: "arrange",
          ids: ["a", "b"],
          region: { x: 32, y: 88, width: 600, height: 480 },
          gap: 24,
          flow: "horizontal",
        },
      ],
      catalogue,
      [1280, 800],
    ]) as HmiPageModel;
    assert.deepStrictEqual(changed.object[0], original.object[0]);
    assert.deepStrictEqual(changed.object[1].at, [272, 88]);
    assert.deepStrictEqual(changed.object[2], original.object[2]);
    assert.deepStrictEqual(changed.pipe, original.pipe);
    assert.throws(
      () =>
        Reflect.apply(editAgentPage, undefined, [
          original,
          [{ op: "arrange", ids: ["a", "b"], region: { x: 1200, y: 88, width: 600, height: 480 } }],
          catalogue,
          [1280, 800],
        ]),
      /fit|bounds/i,
    );
  });
});
