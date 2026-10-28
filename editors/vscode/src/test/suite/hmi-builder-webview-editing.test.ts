import * as assert from "assert";

import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import type { VariableNode } from "../../hmiBuilder/editor/messages";
import { object, PAGE, Webview } from "./hmiBuilderWebview";

// Specification 39, §3.3 item 8 (Phase 5): editing like a design tool.

/** The test page with a third object that the box below covers only in part. */
const THREE: HmiPageModel = {
  ...PAGE,
  object: [...PAGE.object, object("wide", "value", [100, 250], [400, 96])],
};

const selected = (view: Webview, id: string) => view.query(`[data-selection="${id}"]`);

suite("HMI Builder editor: editing like a design tool", function () {
  this.timeout(20000);

  test("a box dragged on empty canvas selects the objects wholly inside it", async () => {
    const view = new Webview();
    await view.loaded(THREE);
    view.drag(view.layer(), [20, 20], [420, 300]);
    await view.until("both objects inside the box selected", () => selected(view, "speed") && selected(view, "level"));
    assert.ok(!selected(view, "wide"), "an object partly outside the box stays unselected");
    assert.ok(!view.query('[data-part="selection-box"]'), "the box goes once the button is released");
  });

  test("Ctrl+A selects every object and Escape clears the selection", async () => {
    const view = new Webview();
    await view.loaded(THREE);
    view.key("a", { ctrlKey: true });
    await view.until(
      "every object selected",
      () => selected(view, "speed") && selected(view, "level") && selected(view, "wide"),
    );
    view.key("Escape");
    await view.until(
      "nothing selected",
      () => !selected(view, "speed") && !selected(view, "level") && !selected(view, "wide"),
    );
  });

  test("Ctrl+C copies the selection to the clipboard and Ctrl+V pastes it with a new id, its bindings and next to the original", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.key("Tab");
    await view.until("speed selected", () => selected(view, "speed"));
    view.key("c", { ctrlKey: true });
    const copy = await view.until("the copy on the clipboard", () =>
      view.posted.find((message) => message.type === "copy"),
    );
    view.key("v", { ctrlKey: true });
    await view.until("the paste asks for the clipboard", () => view.posted.some((message) => message.type === "paste"));
    view.send({ type: "clipboard", text: String(copy.text) });
    const edit = await view.nextEdit(0);
    const pasted = edit.object.find((candidate) => candidate.id === "value-1");
    assert.ok(pasted, JSON.stringify(edit.object.map((candidate) => candidate.id)));
    assert.deepStrictEqual(
      [pasted.at, pasted.label, pasted.bind.value?.path],
      [[56, 56], "Pump speed", "Main.P101.Speed"],
    );
    await view.until("the copy selected", () => selected(view, "value-1") && !selected(view, "speed"));
  });

  test("a copied binding moves to the next number when that variable exists", async () => {
    const view = new Webview();
    const page: HmiPageModel = {
      ...PAGE,
      object: [
        object("tt", "value", [40, 40], [240, 96], { bind: { value: { path: "Main.TT101", direction: "read" } } }),
        object("pump", "pump", [40, 200], [128, 108], {
          bind: { running: { path: "Main.P101.Running", direction: "read" } },
        }),
      ],
    };
    await view.loaded(page);
    const variable = (path: string, children?: VariableNode[]): VariableNode => ({
      name: path.split(".").pop() ?? path,
      path,
      kind: children ? "function-block" : "value",
      type: children ? "Pump" : "REAL",
      kinds: children ? [] : ["number"],
      writable: false,
      ...(children ? { children } : {}),
    });
    view.send({
      type: "variables",
      variables: [
        variable("Main", [
          variable("Main.TT101"),
          variable("Main.TT102"),
          variable("Main.P101", [variable("Main.P101.Running")]),
        ]),
      ],
    });
    view.key("a", { ctrlKey: true });
    await view.until("both selected", () => selected(view, "tt") && selected(view, "pump"));
    view.key("c", { ctrlKey: true });
    const copy = await view.until("the copy", () => view.posted.find((message) => message.type === "copy"));
    view.key("v", { ctrlKey: true });
    view.send({ type: "clipboard", text: String(copy.text) });
    const edit = await view.nextEdit(0);
    const paths = edit.object.slice(2).map((candidate) => Object.values(candidate.bind)[0]?.path);
    assert.deepStrictEqual(paths, ["Main.TT102", "Main.P101.Running"], "TT102 exists, P102 does not");
  });

  test("pipes between copied objects come along; pipes to objects not copied are dropped", async () => {
    const view = new Webview();
    const page: HmiPageModel = {
      ...PAGE,
      object: [
        object("p1", "pump", [40, 40], [128, 108]),
        object("t1", "tank", [400, 40], [160, 200]),
        object("v1", "value", [700, 40], [200, 80]),
      ],
      pipe: [
        { from: "p1.outlet", to: "t1.inlet", waypoints: [[300, 94]] },
        { from: "t1.outlet", to: "v1.inlet" },
      ],
    };
    await view.loaded(page);
    view.drag(view.layer(), [20, 20], [600, 280]);
    await view.until(
      "pump and tank selected",
      () => selected(view, "p1") && selected(view, "t1") && !selected(view, "v1"),
    );
    view.key("c", { ctrlKey: true });
    const copy = await view.until("the copy", () => view.posted.find((message) => message.type === "copy"));
    view.key("v", { ctrlKey: true });
    view.send({ type: "clipboard", text: String(copy.text) });
    const edit = await view.nextEdit(0);
    assert.deepStrictEqual(edit.pipe.slice(2), [
      { from: "pump-1.outlet", to: "tank-1.inlet", waypoints: [[316, 110]] },
    ]);
  });

  test("Ctrl+X cuts the selection", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.key("Tab");
    await view.until("speed selected", () => selected(view, "speed"));
    view.key("x", { ctrlKey: true });
    await view.until("the copy", () => view.posted.some((message) => message.type === "copy"));
    const edit = await view.nextEdit(0);
    assert.deepStrictEqual(
      edit.object.map((candidate) => candidate.id),
      ["level"],
    );
  });

  test("Ctrl+D duplicates; after the copy is moved, the next Ctrl+D repeats that step", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.key("Tab");
    await view.until("speed selected", () => selected(view, "speed"));
    view.key("d", { ctrlKey: true });
    const first = await view.nextEdit(0);
    assert.deepStrictEqual(first.object.find((candidate) => candidate.id === "value-1")?.at, [56, 56]);
    view.send({ type: "page", model: first });
    await view.until("the copy selected", () => selected(view, "value-1"));
    // Four presses, each after the last one's edit, as a user's are.
    for (let step = 0; step < 4; step += 1) {
      view.key("ArrowRight");
      await view.nextEdit(1 + step);
    }
    const moved = view.edits()[4];
    view.send({ type: "page", model: moved });
    await view.until(
      "the copy moved",
      () => moved.object.find((candidate) => candidate.id === "value-1")?.at[0] === 88,
    );
    view.key("d", { ctrlKey: true });
    const second = await view.nextEdit(5);
    assert.deepStrictEqual(
      second.object.find((candidate) => candidate.id === "value-2")?.at,
      [136, 72],
      "the same step again: +48, +16",
    );
  });

  test("with two objects selected, Align left lines up their left edges at the selection's outer edge", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.key("a", { ctrlKey: true });
    await view.until("both selected", () => selected(view, "speed") && selected(view, "level"));
    const button = await view.until("the Align left button", () => view.query('[data-align="left"]'));
    assert.ok(!view.query('[data-distribute="horizontal"]'), "distributing needs three");
    button.click();
    const edit = await view.nextEdit(0);
    assert.deepStrictEqual(
      edit.object.map((candidate) => candidate.at),
      [
        [40, 40],
        [40, 40],
      ],
    );
  });

  test("with three objects selected, Distribute horizontally gives equal gaps", async () => {
    const view = new Webview();
    const page: HmiPageModel = {
      ...PAGE,
      object: [
        object("a", "value", [40, 40], [80, 48]),
        object("b", "value", [200, 40], [80, 48]),
        object("c", "value", [600, 40], [80, 48]),
      ],
    };
    await view.loaded(page);
    view.key("a", { ctrlKey: true });
    await view.until("all selected", () => selected(view, "a") && selected(view, "b") && selected(view, "c"));
    (await view.until("the Distribute button", () => view.query('[data-distribute="horizontal"]'))).click();
    const edit = await view.nextEdit(0);
    assert.deepStrictEqual(
      edit.object.map((candidate) => candidate.at[0]),
      [40, 320, 600],
    );
  });

  test("while dragging, an object snaps to another's edge, with a guide, off the grid", async () => {
    const view = new Webview();
    const page: HmiPageModel = {
      ...PAGE,
      object: [object("fixed", "value", [403, 40], [160, 96]), object("moving", "value", [40, 300], [160, 96])],
    };
    await view.loaded(page);
    view.mouse("mousedown", view.layer(), 100, 340);
    view.mouse("mousemove", view.window, 461, 342);
    await view.until("a guide at the fixed object's left edge", () =>
      view.query('[data-part="guide"][data-axis="x"][data-at="403"]'),
    );
    view.mouse("mouseup", view.window, 461, 342);
    const edit = await view.nextEdit(0);
    assert.deepStrictEqual(
      edit.object.find((candidate) => candidate.id === "moving")?.at,
      [403, 304],
      "x on the edge, y on the grid",
    );
    assert.ok(!view.query('[data-part="guide"]'), "the guide goes with the drag");
  });

  test("while dragging between two objects, equal gaps snap and show", async () => {
    const view = new Webview();
    const page: HmiPageModel = {
      ...PAGE,
      object: [
        object("left", "value", [40, 40], [100, 60]),
        object("right", "value", [500, 40], [100, 60]),
        object("middle", "value", [200, 300], [100, 60]),
      ],
    };
    await view.loaded(page);
    // Left ends at 140 and right starts at 500: with the middle object 100 wide, equal gaps put it at 270.
    view.mouse("mousedown", view.layer(), 250, 330);
    view.mouse("mousemove", view.window, 317, 77);
    await view.until("the equal gaps shown", () => view.document.querySelectorAll('[data-part="gap"]').length === 2);
    view.mouse("mouseup", view.window, 317, 77);
    const edit = await view.nextEdit(0);
    assert.strictEqual(edit.object.find((candidate) => candidate.id === "middle")?.at[0], 270);
  });

  test("Ctrl+G groups the selection; a click on a member selects the group, a double-click one member; Ctrl+Shift+G ungroups", async () => {
    const view = new Webview();
    await view.loaded(THREE);
    view.drag(view.layer(), [20, 20], [420, 300]);
    await view.until("speed and level selected", () => selected(view, "speed") && selected(view, "level"));
    view.key("g", { ctrlKey: true });
    const grouped = await view.nextEdit(0);
    assert.deepStrictEqual(
      grouped.object.map((candidate) => candidate.group),
      ["group-1", "group-1", undefined],
    );
    view.send({ type: "page", model: grouped });
    view.key("Escape");
    await view.until("nothing selected", () => !selected(view, "speed") && !selected(view, "level"));
    view.mouse("mousedown", view.layer(), 60, 60);
    view.mouse("mouseup", view.window, 60, 60);
    await view.until(
      "a click on a member selects its group",
      () => selected(view, "speed") && selected(view, "level") && !selected(view, "wide"),
    );
    view.mouse("dblclick", view.layer(), 60, 60);
    await view.until(
      "a double-click selects the one member",
      () => selected(view, "speed") && !selected(view, "level"),
    );
    view.key("g", { ctrlKey: true, shiftKey: true });
    const ungrouped = await view.nextEdit(1);
    assert.deepStrictEqual(
      ungrouped.object.map((candidate) => candidate.group),
      [undefined, undefined, undefined],
      "the whole group goes",
    );
  });

  test("Ctrl+] and Ctrl+[ move the selection one step in the layer order, with Shift to the front or the back", async () => {
    const view = new Webview();
    await view.loaded(THREE);
    view.key("Tab");
    await view.until("speed selected", () => selected(view, "speed"));
    const order = (model: HmiPageModel) => model.object.map((candidate) => candidate.id);
    view.key("]", { ctrlKey: true });
    const forward = await view.nextEdit(0);
    assert.deepStrictEqual(order(forward), ["level", "speed", "wide"], "one step forward");
    view.send({ type: "page", model: forward });
    view.key("]", { ctrlKey: true, shiftKey: true });
    const front = await view.nextEdit(1);
    assert.deepStrictEqual(order(front), ["level", "wide", "speed"], "to the front");
    view.send({ type: "page", model: front });
    view.key("[", { ctrlKey: true });
    const backward = await view.nextEdit(2);
    assert.deepStrictEqual(order(backward), ["level", "speed", "wide"], "one step back");
    view.send({ type: "page", model: backward });
    view.key("[", { ctrlKey: true, shiftKey: true });
    const back = await view.nextEdit(3);
    assert.deepStrictEqual(order(back), ["speed", "level", "wide"], "to the back");
  });

  test("a press on the selection keeps it, so a drag moves every selected object", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.key("a", { ctrlKey: true });
    await view.until("both selected", () => selected(view, "speed") && selected(view, "level"));
    view.drag(view.layer(), [50, 50], [66, 50]);
    const edit = await view.nextEdit(0);
    assert.deepStrictEqual(
      edit.object.map((candidate) => candidate.at),
      [
        [56, 40],
        [336, 40],
      ],
    );
  });
});
