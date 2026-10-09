import * as assert from "assert";
import { composeAgentPage } from "../../hmiBuilder/agent/compose";
import { editAgentPage } from "../../hmiBuilder/agent/edits";
import type { HmiObjectModel, HmiPageModel } from "../../hmiBuilder/drawing/model";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import { addPipe } from "../../hmiBuilder/editor/pipes";

// Specification 43, "Repairs — 7 October 2026", item 4: the agent edits pipes with the builder's
// own pipe editing (specification 39, §6), so it refuses what the canvas refuses, for its reason.

const catalogue: Catalogue = {
  format: 1,
  components: [
    { type: "pump", category: "equipment", phase: 2, size: [128, 108], ports: ["inlet", "outlet"] },
    { type: "valve", category: "equipment", phase: 2, size: [128, 108], ports: ["inlet", "outlet"] },
    { type: "junction", category: "equipment", phase: 2, size: [16, 16], ports: ["left", "right", "top", "bottom"] },
    // The catalogue names a "drain" the tank symbol does not draw, so the canvas never offers it.
    { type: "tank", category: "equipment", phase: 2, size: [128, 172], ports: ["inlet", "outlet", "drain"] },
    { type: "flow-sensor", category: "instrument", phase: 2, size: [64, 64], attaches: true },
  ],
};

function object(id: string, type: string, at: [number, number], size: [number, number] = [128, 108]): HmiObjectModel {
  return { id, type, at, size, rotation: 0, bind: {} };
}

function page(pipe: HmiPageModel["pipe"] = []): HmiPageModel {
  return {
    title: "Pipes",
    pipe,
    object: [
      object("p-101", "pump", [40, 40]),
      object("p-102", "pump", [40, 320]),
      object("v-101", "valve", [480, 40]),
      object("j-1", "junction", [400, 200], [16, 16]),
      object("tk-1", "tank", [700, 300], [128, 172]),
    ],
  };
}

const refusal =
  (pattern: RegExp) =>
  (error: unknown): boolean => {
    assert.ok(error instanceof Error, String(error));
    assert.match(error.message, pattern);
    return true;
  };

suite("HMI agent pipes follow the builder", () => {
  test("a connection the canvas accepts is joined exactly as the canvas joins it", () => {
    const source = page();
    const joined = editAgentPage(source, [{ op: "connect", from: "p-101.outlet", to: "v-101.inlet" }], catalogue);
    assert.deepStrictEqual(joined.pipe, addPipe(source, "p-101.outlet", "v-101.inlet").pipe);
    assert.deepStrictEqual(source.pipe, [], "the source page is unchanged");
  });

  test("a port that already has a pipe is refused, as the canvas refuses it, with the builder's reason", () => {
    const source = page([{ from: "p-101.outlet", to: "v-101.inlet" }]);
    assert.strictEqual(
      addPipe(source, "p-101.outlet", "v-101.outlet"),
      source,
      "fixture: the builder refuses this pipe",
    );
    assert.throws(
      () => editAgentPage(source, [{ op: "connect", from: "p-101.outlet", to: "v-101.outlet" }], catalogue),
      refusal(/^CONFLICT: the builder joins only free ports, and p-101\.outlet already has a pipe/),
    );
    assert.strictEqual(
      addPipe(source, "p-102.outlet", "v-101.inlet"),
      source,
      "fixture: one inlet fed twice is refused too",
    );
    assert.throws(
      () => editAgentPage(source, [{ op: "connect", from: "p-102.outlet", to: "v-101.inlet" }], catalogue),
      refusal(/^CONFLICT: .*v-101\.inlet already has a pipe/),
    );
    assert.throws(
      () => editAgentPage(source, [{ op: "connect", from: "p-101.outlet", to: "v-101.inlet" }], catalogue),
      refusal(/already has a pipe/),
      "the same pipe again is no second pipe",
    );
  });

  test("one port twice is refused with the builder's reason", () => {
    const source = page();
    assert.strictEqual(
      addPipe(source, "v-101.outlet", "v-101.outlet"),
      source,
      "fixture: the builder refuses this pipe",
    );
    assert.throws(
      () => editAgentPage(source, [{ op: "connect", from: "v-101.outlet", to: "v-101.outlet" }], catalogue),
      refusal(/^INVALID_ARGUMENT: the builder joins two different ports; v-101\.outlet cannot connect to itself/),
    );
  });

  test("only a port the canvas offers can be joined", () => {
    assert.throws(
      () => editAgentPage(page(), [{ op: "connect", from: "p-101.outlet", to: "tk-1.drain" }], catalogue),
      refusal(/^INVALID_ARGUMENT: unknown port tk-1\.drain/),
      "a catalogue port the shared drawing cannot place",
    );
    assert.throws(
      () => editAgentPage(page(), [{ op: "connect", from: "p-101.outlet", to: "missing.inlet" }], catalogue),
      refusal(/^INVALID_ARGUMENT: unknown port missing\.inlet/),
    );
    assert.throws(
      () => editAgentPage(page(), [{ op: "connect", from: "p-101.outlet", to: "v-101.side" }], catalogue),
      refusal(/^INVALID_ARGUMENT: unknown port v-101\.side/),
    );
  });

  test("a branch runs through a junction, as the canvas makes one", () => {
    const branched = editAgentPage(
      page(),
      [
        { op: "connect", from: "p-101.outlet", to: "j-1.left" },
        { op: "connect", from: "j-1.top", to: "v-101.inlet" },
        { op: "connect", from: "j-1.bottom", to: "tk-1.inlet" },
      ],
      catalogue,
    );
    assert.deepStrictEqual(branched.pipe, [
      { from: "p-101.outlet", to: "j-1.left" },
      { from: "j-1.top", to: "v-101.inlet" },
      { from: "j-1.bottom", to: "tk-1.inlet" },
    ]);
  });

  test("reconnection follows the same rule and keeps the pipe's place, style and attachments", () => {
    const source = page([
      { from: "p-101.outlet", to: "v-101.inlet", style: "signal" },
      { from: "p-102.outlet", to: "j-1.left" },
    ]);
    source.object.push({ ...object("ft-1", "flow-sensor", [240, 160], [64, 64]), attach: "pipe:0" });
    assert.throws(
      () =>
        editAgentPage(
          source,
          [{ op: "reconnect", from: "p-101.outlet", to: "v-101.inlet", newFrom: "p-101.outlet", newTo: "j-1.left" }],
          catalogue,
        ),
      refusal(/^CONFLICT: .*j-1\.left already has a pipe/),
    );
    const moved = editAgentPage(
      source,
      [{ op: "reconnect", from: "p-101.outlet", to: "v-101.inlet", newFrom: "p-101.outlet", newTo: "tk-1.inlet" }],
      catalogue,
    );
    assert.deepStrictEqual(
      moved.pipe[0],
      { from: "p-101.outlet", to: "tk-1.inlet", style: "signal" },
      "its own old ends are free once it moves",
    );
    assert.strictEqual(moved.object.find((candidate) => candidate.id === "ft-1")?.attach, "pipe:0");
  });

  test("a connection is named by its ends, even when one end is no longer on the page", () => {
    assert.throws(
      () => editAgentPage(page(), [{ op: "disconnect", from: "p-101.outlet", to: "v-101.inlet" }], catalogue),
      refusal(/^INVALID_ARGUMENT: unknown connection p-101\.outlet -> v-101\.inlet/),
    );
    const broken = page([{ from: "gone.outlet", to: "v-101.inlet" }]);
    assert.deepStrictEqual(
      editAgentPage(broken, [{ op: "disconnect", from: "gone.outlet", to: "v-101.inlet" }], catalogue).pipe,
      [],
    );
    const repaired = editAgentPage(
      broken,
      [{ op: "reconnect", from: "gone.outlet", to: "v-101.inlet", newFrom: "p-101.outlet", newTo: "v-101.inlet" }],
      catalogue,
    );
    assert.deepStrictEqual(repaired.pipe, [{ from: "p-101.outlet", to: "v-101.inlet" }]);
  });

  test("duplication uses the builder's copy and paste with the chosen ids", () => {
    const source = page([{ from: "p-101.outlet", to: "v-101.inlet" }]);
    source.object.push({ ...object("ft-1", "flow-sensor", [240, 0], [64, 64]), attach: "pipe:0" });
    const group = editAgentPage(
      source,
      [{ op: "duplicate", ids: ["p-101", "v-101", "ft-1"], newIds: ["p-201", "v-201", "ft-2"], delta: [0, 400] }],
      catalogue,
    );
    assert.deepStrictEqual(
      group.pipe[1],
      { from: "p-201.outlet", to: "v-201.inlet" },
      "the pipe between the copies comes along",
    );
    assert.strictEqual(
      group.object.find((candidate) => candidate.id === "ft-2")?.attach,
      "pipe:1",
      "the copied instrument measures the copied pipe",
    );
    assert.deepStrictEqual(group.object.find((candidate) => candidate.id === "p-201")?.at, [40, 440]);
    const alone = editAgentPage(
      source,
      [{ op: "duplicate", ids: ["ft-1"], newIds: ["ft-2"], delta: [0, 120] }],
      catalogue,
    );
    const copy = alone.object.find((candidate) => candidate.id === "ft-2");
    assert.ok(copy);
    assert.strictEqual(
      copy.attach,
      undefined,
      "an instrument copied without what it measures stands free, as in the builder",
    );
    assert.deepStrictEqual(copy.at, [240, 120]);
    assert.strictEqual(alone.pipe.length, 1);
  });

  test("composition refuses a connection the builder refuses", () => {
    const input = {
      title: "Transfer",
      pattern: "process-overview" as const,
      size: [1280, 800] as [number, number],
      items: [
        { id: "tank", type: "tank", region: "process" as const },
        { id: "pump-a", type: "pump", region: "process" as const },
        { id: "pump-b", type: "pump", region: "process" as const },
      ],
    };
    assert.throws(
      () =>
        composeAgentPage(
          {
            ...input,
            connections: [
              { from: "tank.outlet", to: "pump-a.inlet" },
              { from: "tank.outlet", to: "pump-b.inlet" },
            ],
          },
          catalogue,
        ),
      refusal(/^CONFLICT: .*tank\.outlet already has a pipe/),
    );
    const composed = composeAgentPage(
      { ...input, connections: [{ from: "tank.outlet", to: "pump-a.inlet" }] },
      catalogue,
    );
    assert.deepStrictEqual(composed.pipe, [{ from: "tank.outlet", to: "pump-a.inlet" }]);
  });
});
