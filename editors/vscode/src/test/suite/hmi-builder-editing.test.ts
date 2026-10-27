import * as assert from "assert";

import type { HmiObjectModel, HmiPageModel } from "../../hmiBuilder/drawing/model";
import {
  addObject,
  deleteObjects,
  freeCorner,
  moveObjects,
  nextId,
  nudge,
  objectAt,
  resizeObject,
  selectFromClick,
  updateObject,
  visiblePageCentre,
} from "../../hmiBuilder/editor/editing";

// Specification 39, §3.3: editing on the 8 px grid. A loop controller's signal lines are the
// language server's rule (§6 item 5), proven in
// crates/trust-lsp/src/handlers/commands/hmi_builder/equipment_tests.rs.

function object(
  id: string,
  type: string,
  at: [number, number],
  size: [number, number],
  extra: Partial<HmiObjectModel> = {},
): HmiObjectModel {
  return { id, type, at, size, rotation: 0, bind: {}, ...extra };
}

function page(...objects: HmiObjectModel[]): HmiPageModel {
  return { title: "Edit", object: objects, pipe: [] };
}

function frozen(model: HmiPageModel): string {
  return JSON.stringify(model);
}

function item(model: HmiPageModel, id: string): HmiObjectModel {
  const found = model.object.find((candidate) => candidate.id === id);
  assert.ok(found, `object ${id} in ${JSON.stringify(model.object.map((candidate) => candidate.id))}`);
  return found;
}

const at = (model: HmiPageModel, id: string): [number, number] => item(model, id).at;
const size = (model: HmiPageModel, id: string): [number, number] => item(model, id).size;

suite("HMI Builder editor: editing", () => {
  test("a new object gets its type's default size, the first free id and a position on the grid", () => {
    const before = page(
      object("value-1", "value", [0, 0], [224, 96]),
      object("value-3", "value", [0, 0], [224, 96]),
      object("bar-1", "bar", [0, 0], [80, 240]),
    );
    assert.strictEqual(nextId(before, "value"), "value-2");
    assert.strictEqual(nextId(page(), "gauge"), "gauge-1");
    const original = frozen(before);
    const added = addObject(before, { type: "value", size: [224, 96] }, [103, 45]);
    assert.strictEqual(added.id, "value-2");
    assert.deepStrictEqual(
      added.page.object[added.page.object.length - 1],
      object("value-2", "value", [104, 48], [224, 96]),
    );
    assert.strictEqual(frozen(before), original, "the old page is left as it was");
    assert.deepStrictEqual(
      at(addObject(page(), { type: "text", size: [160, 32] }, [103, 45], true).page, "text-1"),
      [103, 45],
      "Alt places freely",
    );
  });

  test("dragging puts the first object on the grid and keeps the others' offsets; Alt moves freely", () => {
    const before = page(
      object("a", "value", [40, 40], [80, 40]),
      object("b", "value", [100, 60], [80, 40]),
      object("c", "value", [0, 0], [80, 40]),
    );
    const moved = moveObjects(before, ["a", "b"], [13, 6]);
    assert.deepStrictEqual(
      [at(moved, "a"), at(moved, "b"), at(moved, "c")],
      [
        [56, 48],
        [116, 68],
        [0, 0],
      ],
    );
    const free = moveObjects(before, ["a", "b"], [13, 6], true);
    assert.deepStrictEqual(
      [at(free, "a"), at(free, "b")],
      [
        [53, 46],
        [113, 66],
      ],
    );
  });

  test("the arrow keys move one grid step, with Alt one pixel", () => {
    const before = page(object("a", "value", [40, 40], [80, 40]));
    assert.deepStrictEqual(at(nudge(before, ["a"], "right"), "a"), [48, 40]);
    assert.deepStrictEqual(at(nudge(before, ["a"], "up"), "a"), [40, 32]);
    assert.deepStrictEqual(at(nudge(before, ["a"], "left", true), "a"), [39, 40]);
    assert.deepStrictEqual(at(nudge(before, ["a"], "down", true), "a"), [40, 41]);
  });

  test("resizing moves the dragged edges onto the grid and never below one step", () => {
    const before = page(object("a", "value", [40, 40], [80, 40]));
    const corner = resizeObject(before, "a", "se", [13, 3]);
    assert.deepStrictEqual(
      [at(corner, "a"), size(corner, "a")],
      [
        [40, 40],
        [96, 40],
      ],
    );
    const left = resizeObject(before, "a", "w", [-13, 0]);
    assert.deepStrictEqual(
      [at(left, "a"), size(left, "a")],
      [
        [24, 40],
        [96, 40],
      ],
    );
    const crushed = resizeObject(before, "a", "nw", [100, 100]);
    assert.deepStrictEqual(
      [at(crushed, "a"), size(crushed, "a")],
      [
        [112, 72],
        [8, 8],
      ],
    );
    const free = resizeObject(before, "a", "se", [13, 3], true);
    assert.deepStrictEqual(size(free, "a"), [93, 43]);
  });

  test("deleting removes the selected objects and keeps the rest in order", () => {
    const before = page(
      object("a", "value", [0, 0], [8, 8]),
      object("b", "value", [0, 0], [8, 8]),
      object("c", "value", [0, 0], [8, 8]),
    );
    assert.deepStrictEqual(
      deleteObjects(before, ["b"]).object.map((item) => item.id),
      ["a", "c"],
    );
  });

  test("the topmost object under the pointer is found, rotated objects by their drawn box", () => {
    const below = object("below", "panel", [0, 0], [200, 200]);
    const above = object("above", "value", [40, 40], [80, 40]);
    const turned = object("turned", "bar", [300, 0], [100, 20], { rotation: 90 });
    const model = page(below, above, turned);
    assert.strictEqual(objectAt(model, [50, 50]), "above");
    assert.strictEqual(objectAt(model, [10, 10]), "below");
    assert.strictEqual(objectAt(model, [500, 500]), undefined);
    assert.strictEqual(objectAt(model, [350, 50]), "turned", "inside the turned box");
    assert.strictEqual(objectAt(model, [390, 10]), undefined, "inside the unturned box only");
  });

  test("a click selects, Shift-click adds or removes, and a click on empty canvas clears", () => {
    assert.deepStrictEqual(selectFromClick(["a"], "b", false), ["b"]);
    assert.deepStrictEqual(selectFromClick(["a"], "b", true), ["a", "b"]);
    assert.deepStrictEqual(selectFromClick(["a", "b"], "a", true), ["b"]);
    assert.deepStrictEqual(selectFromClick(["a"], undefined, false), []);
    assert.deepStrictEqual(selectFromClick(["a"], undefined, true), ["a"]);
  });

  test("a clicked component goes to the centre of the visible part of the page", () => {
    const size: [number, number] = [1280, 720];
    const whole = visiblePageCentre(
      { left: 0, top: 0, width: 800, height: 2000 },
      { left: 48, top: 32, width: 704, height: 396 },
      0.55,
      size,
    );
    assert.deepStrictEqual(whole.map(Math.round), [640, 360], "all of the page is visible: its centre");
    const zoomed = visiblePageCentre(
      { left: 0, top: 0, width: 800, height: 600 },
      { left: -400, top: -300, width: 2560, height: 1440 },
      2,
      size,
    );
    assert.deepStrictEqual(zoomed, [400, 300], "zoomed in: the centre of what shows");
    assert.deepStrictEqual(
      visiblePageCentre({ left: 0, top: 0, width: 0, height: 0 }, { left: 0, top: 0, width: 0, height: 0 }, 1, size),
      [640, 360],
    );
  });

  test("the inspector's edits keep whole pixels and the one-step minimum", () => {
    const before = page(object("a", "value", [40, 40], [80, 40]));
    const edited = updateObject(before, "a", { at: [10.4, 20.6], size: [3, 100.2], label: "Feed" });
    const changed = edited.object[0];
    assert.deepStrictEqual([changed.at, changed.size, changed.label], [[10, 21], [8, 100], "Feed"]);
    assert.strictEqual(before.object[0].label, undefined);
  });
  test("objects placed one after another step aside instead of piling up", () => {
    const at = (x: number, y: number) => ({
      id: `o${x}-${y}`,
      type: "value",
      at: [x, y] as [number, number],
      size: [160, 48] as [number, number],
      bind: {},
    });
    const page = { title: "P", object: [at(400, 300), at(424, 324)], pipe: [] } as any;
    assert.deepStrictEqual(freeCorner(page, [400, 300], [160, 48], [1280, 720]), [448, 348]);
    assert.deepStrictEqual(freeCorner(page, [100, 100], [160, 48], [1280, 720]), [100, 100], "a free spot stays");
  });
});
