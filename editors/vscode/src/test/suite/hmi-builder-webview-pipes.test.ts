import * as assert from "assert";

import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import { portPoint } from "../../hmiBuilder/drawing/pipes";
import { object, Webview } from "./hmiBuilderWebview";

// Specification 39, §6: pipes in the builder's canvas: a symbol's ports appear when the pointer is
// on it, a drag from one port to another draws a pipe, a click selects a pipe for the inspector,
// a drag bends it, and a valve dropped onto it splits it.

const PUMP = object("p-101", "pump", [40, 40], [128, 108]);
// The valve's inlet lies on the pump outlet's line, so a pipe between them runs straight.
const VALVE = object("v-101", "valve", [480, 16], [128, 108]);

function page(pipe: HmiPageModel["pipe"] = [], extra: HmiPageModel["object"] = []): HmiPageModel {
  return { title: "Pipes", object: [PUMP, VALVE, ...extra], pipe };
}

const OUTLET = portPoint(PUMP, "outlet")!.at;

function click(view: Webview, at: [number, number]): void {
  view.mouse("mousedown", view.layer(), at[0], at[1]);
  view.mouse("mouseup", view.window, at[0], at[1]);
}

suite("HMI Builder editor: pipes in the webview", function () {
  this.timeout(20000);

  test("pointing at a symbol shows its ports, and a drag from a port to another draws a pipe", async () => {
    const view = new Webview();
    await view.loaded(page());
    view.mouse("mousemove", view.layer(), 100, 80);
    const port = await view.until("the pump's ports", () => view.query('[data-port="p-101.outlet"]'));
    assert.strictEqual(view.query('[data-port="v-101.inlet"]'), null, "only the symbol pointed at shows its ports");
    const before = view.edits().length;
    view.mouse("mousedown", port, OUTLET[0], OUTLET[1]);
    const inlet = portPoint(VALVE, "inlet")!.at;
    view.mouse("mousemove", view.window, inlet[0] - 30, inlet[1]);
    await view.until("every port as a target while drawing", () => view.query('[data-port="v-101.inlet"]'));
    assert.ok(view.query('[data-part="drawing-pipe"]'), "the pipe follows the pointer while drawn");
    view.mouse("mouseup", view.window, inlet[0] + 2, inlet[1] - 1);
    const edit = await view.nextEdit(before);
    assert.deepStrictEqual(edit.pipe, [{ from: "p-101.outlet", to: "v-101.inlet" }]);
    await view.until("the drawn pipe selected, for its inspector", () => view.query('[data-selection="pipe:0"]'));
  });

  test("a pipe released on another pipe branches into it through a new junction", async () => {
    const view = new Webview();
    const far = object("v-101", "valve", [880, 16], [128, 108]);
    const lower = object("p-102", "pump", [400, 300], [128, 108]);
    await view.loaded({
      title: "Pipes",
      object: [PUMP, far, lower],
      pipe: [{ from: "p-101.outlet", to: "v-101.inlet" }],
    });
    view.mouse("mousemove", view.layer(), 460, 340);
    const port = await view.until("the lower pump's ports", () => view.query('[data-port="p-102.outlet"]'));
    const outlet = portPoint(lower, "outlet")!.at;
    const before = view.edits().length;
    view.mouse("mousedown", port, outlet[0], outlet[1]);
    view.mouse("mousemove", view.window, 600, 120);
    view.mouse("mouseup", view.window, 600, 63);
    const edit = await view.nextEdit(before);
    assert.deepStrictEqual(edit.pipe, [
      { from: "p-101.outlet", to: "junction-1.left" },
      { from: "junction-1.right", to: "v-101.inlet" },
      { from: "p-102.outlet", to: "junction-1.bottom" },
    ]);
    await view.until("the branch selected, for its inspector", () => view.query('[data-selection="pipe:2"]'));
  });

  test("a click on a pipe selects it, the inspector sets its line style, and Delete removes it", async () => {
    const view = new Webview();
    await view.loaded(page([{ from: "p-101.outlet", to: "v-101.inlet" }]));
    click(view, [OUTLET[0] + 150, OUTLET[1]]);
    await view.until("the pipe selected", () => view.query('[data-selection="pipe:0"]'));
    const style = await view.until("the pipe's line style", () =>
      view.query('[data-part="pipe-section"] select[name="pipe-style"]'),
    );
    const before = view.edits().length;
    view.choose(style, "signal");
    assert.strictEqual((await view.nextEdit(before)).pipe[0].style, "signal");
    const deleting = view.edits().length;
    view.key("Delete");
    assert.deepStrictEqual((await view.nextEdit(deleting)).pipe, []);
  });

  test("a selected pipe stays selected when the edited page comes back from the document", async () => {
    const view = new Webview();
    const model = page([{ from: "p-101.outlet", to: "v-101.inlet" }]);
    await view.loaded(model);
    click(view, [OUTLET[0] + 150, OUTLET[1]]);
    await view.until("the pipe selected", () => view.query('[data-selection="pipe:0"]'));
    view.send({ type: "page", model: { ...model, pipe: [{ ...model.pipe[0], style: "signal" }] } });
    await view.until(
      "the pipe still selected, with its new line style",
      () => view.query('[data-part="pipe-section"] select[name="pipe-style"]')?.value === "signal" || undefined,
    );
  });

  test("the inspector binds a pipe's flow to a variable, and an empty field removes it", async () => {
    const view = new Webview();
    await view.loaded(page([{ from: "p-101.outlet", to: "v-101.inlet" }]));
    click(view, [OUTLET[0] + 150, OUTLET[1]]);
    const field = await view.until("the flow field", () =>
      view.query('[data-part="pipe-section"] input[name="bind.flow"]'),
    );
    const before = view.edits().length;
    view.type(field, "Plant.FT101");
    assert.deepStrictEqual((await view.nextEdit(before)).pipe[0].bind, {
      flow: { path: "Plant.FT101", direction: "read" },
    });
    const bound = view.edits().length;
    view.type(view.query('[data-part="pipe-section"] input[name="bind.flow"]'), "");
    assert.deepStrictEqual((await view.nextEdit(bound)).pipe[0], { from: "p-101.outlet", to: "v-101.inlet" });
  });

  test("Straighten removes a pipe's bends", async () => {
    const view = new Webview();
    await view.loaded(
      page([
        {
          from: "p-101.outlet",
          to: "v-101.inlet",
          waypoints: [
            [176, 304],
            [472, 304],
          ],
        },
      ]),
    );
    click(view, [OUTLET[0] + 100, 304]);
    const straighten = await view.until("Straighten", () =>
      view.query('[data-part="pipe-section"] [data-action="straighten"]'),
    );
    const before = view.edits().length;
    straighten.click();
    assert.deepStrictEqual((await view.nextEdit(before)).pipe[0], { from: "p-101.outlet", to: "v-101.inlet" });
  });

  test("dragging a selected pipe's run bends it", async () => {
    const view = new Webview();
    await view.loaded(page([{ from: "p-101.outlet", to: "v-101.inlet" }]));
    const middle: [number, number] = [OUTLET[0] + 150, OUTLET[1]];
    click(view, middle);
    await view.until("the pipe selected", () => view.query('[data-selection="pipe:0"]'));
    const before = view.edits().length;
    view.drag(view.layer(), middle, [middle[0], 302]);
    const edit = await view.nextEdit(before);
    // The run's ends lie a clearance beyond the pump's box and before the valve's.
    assert.deepStrictEqual(edit.pipe[0].waypoints, [
      [40 + 128 + 8, 304],
      [480 - 8, 304],
    ]);
  });

  test("a valve dropped onto a pipe splits it into two pipes through the valve", async () => {
    const view = new Webview();
    const far = object("v-101", "valve", [880, 16], [128, 108]);
    const inline = object("v-102", "valve", [400, 400], [128, 108]);
    await view.loaded({
      title: "Pipes",
      object: [PUMP, far, inline],
      pipe: [{ from: "p-101.outlet", to: "v-101.inlet" }],
    });
    const before = view.edits().length;
    view.drag(view.layer(), [464, 454], [464, 70]);
    const edit = await view.nextEdit(before);
    assert.deepStrictEqual(edit.object.find((candidate) => candidate.id === "v-102")?.at, [400, 16]);
    assert.deepStrictEqual(edit.pipe, [
      { from: "p-101.outlet", to: "v-102.inlet" },
      { from: "v-102.outlet", to: "v-101.inlet" },
    ]);
  });
});
