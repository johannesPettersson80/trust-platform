import * as assert from "assert";

import type { HmiObjectModel, HmiPageModel } from "../../hmiBuilder/drawing/model";
import { deleteObjects } from "../../hmiBuilder/editor/editing";
import { branchPipe, deletePipes, splitPipe } from "../../hmiBuilder/editor/pipes";

// Specification 39, §6 item 5: an instrument's attachment names an equipment or a pipe by its
// place; it follows the pipes as they are deleted and split, and goes when its target does.

const PORTS: Record<string, string[]> = {
  pump: ["inlet", "outlet"],
  valve: ["inlet", "outlet"],
  junction: ["left", "right", "top", "bottom"],
};

function thing(id: string, type: string, at: [number, number], extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  return { id, type, at, size: type.endsWith("sensor") ? [176, 56] : [128, 108], rotation: 0, bind: {}, ...extra };
}

/** A pump feeding two valves, with a sensor on each pipe and one on the pump. */
function plant(): HmiPageModel {
  return {
    title: "Plant",
    object: [
      thing("p-101", "pump", [40, 40]),
      thing("v-101", "valve", [880, 16]),
      thing("v-102", "valve", [880, 400]),
      thing("ft-1", "flow-sensor", [300, 200], { attach: "pipe:0" }),
      thing("ft-2", "flow-sensor", [300, 500], { attach: "pipe:1" }),
      thing("tt-1", "temperature-sensor", [40, 300], { attach: "p-101" }),
    ],
    pipe: [
      { from: "p-101.outlet", to: "v-101.inlet" },
      { from: "v-101.outlet", to: "v-102.inlet" },
    ],
  };
}

function attached(page: HmiPageModel): Record<string, string | undefined> {
  return Object.fromEntries(
    page.object.filter((object) => object.type.endsWith("sensor")).map((object) => [object.id, object.attach]),
  );
}

suite("HMI Builder editor: attachments", () => {
  test("deleting a pipe drops the attachments to it and renumbers those to later pipes", () => {
    assert.deepStrictEqual(attached(deletePipes(plant(), ["pipe:0"])), {
      "ft-1": undefined,
      "ft-2": "pipe:0",
      "tt-1": "p-101",
    });
  });

  test("deleting an object drops the attachments to it and to its pipes", () => {
    assert.deepStrictEqual(attached(deleteObjects(plant(), ["p-101"])), {
      "ft-1": undefined,
      "ft-2": "pipe:0",
      "tt-1": undefined,
    });
  });

  test("splitting a pipe keeps an attachment to it on its first part and moves those to later pipes on by one", () => {
    const page = plant();
    page.object.push(thing("v-103", "valve", [400, 16]));
    const split = splitPipe(page, "v-103", PORTS, [1280, 720]);
    assert.strictEqual(split.pipe.length, 3, "the pipe was split");
    assert.deepStrictEqual(attached(split), { "ft-1": "pipe:0", "ft-2": "pipe:2", "tt-1": "p-101" });
    // A branch splits too; the new branch comes last.
    page.object.push(thing("p-102", "pump", [400, 300]));
    const branched = branchPipe(plant(), "v-102.outlet", [600, 63], PORTS, [1280, 720]);
    assert.deepStrictEqual(attached(branched), { "ft-1": "pipe:0", "ft-2": "pipe:2", "tt-1": "p-101" });
  });
});
