import * as assert from "assert";

import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import { object, Webview } from "./hmiBuilderWebview";

// Specification 39, §6 item 5: an instrument is attached to what it measures in the inspector,
// or by dragging from its handle onto the equipment or the pipe.

const PUMP = object("p-101", "pump", [40, 40], [128, 108], { tag: "P-101" });
const VALVE = object("v-101", "valve", [480, 16], [128, 108], { tag: "XV-101" });
const SENSOR = object("tt-101", "temperature-sensor", [300, 360], [176, 56], { tag: "TT-101" });

function page(): HmiPageModel {
  return { title: "Attach", object: [PUMP, VALVE, SENSOR], pipe: [{ from: "p-101.outlet", to: "v-101.inlet" }] };
}

async function selectSensor(view: Webview): Promise<void> {
  view.mouse("mousedown", view.layer(), 330, 380);
  view.mouse("mouseup", view.window, 330, 380);
  await view.until("the sensor selected", () => view.query('[data-selection="tt-101"]'));
}

suite("HMI Builder editor: attaching instruments", function () {
  this.timeout(20000);

  test("the inspector attaches a selected instrument to an equipment or a pipe, and detaches it", async () => {
    const view = new Webview();
    await view.loaded(page());
    await selectSensor(view);
    const choice = await view.until("Attached to", () => view.query('select[name="attach"]'));
    const offered = [...choice.querySelectorAll("option")].map((option: { value: string; textContent: string }) => [
      option.value,
      option.textContent,
    ]);
    assert.deepStrictEqual(offered, [
      ["", "Nothing"],
      ["p-101", "P-101"],
      ["v-101", "XV-101"],
      ["pipe:0", "Pipe P-101 outlet → XV-101 inlet"],
    ]);
    const before = view.edits().length;
    view.choose(choice, "pipe:0");
    const attached = await view.nextEdit(before);
    assert.strictEqual(attached.object.find((candidate) => candidate.id === "tt-101")?.attach, "pipe:0");
    const again = view.edits().length;
    view.choose(view.query('select[name="attach"]'), "");
    const detached = await view.nextEdit(again);
    assert.strictEqual("attach" in (detached.object.find((candidate) => candidate.id === "tt-101") ?? {}), false);
  });

  test("dragging from a selected instrument's handle onto a pump attaches it; onto empty canvas, nothing", async () => {
    const view = new Webview();
    await view.loaded(page());
    await selectSensor(view);
    const handle = await view.until("the attach handle", () => view.query('[data-part="attach-handle"]'));
    const at: [number, number] = [Number(handle.getAttribute("cx")), Number(handle.getAttribute("cy"))];
    const before = view.edits().length;
    view.mouse("mousedown", handle, at[0], at[1]);
    view.mouse("mousemove", view.window, 200, 150);
    await view.until("the line follows the pointer", () => view.query('[data-part="attaching"]'));
    view.mouse("mouseup", view.window, 104, 80);
    const edit = await view.nextEdit(before);
    assert.strictEqual(edit.object.find((candidate) => candidate.id === "tt-101")?.attach, "p-101");
    await selectSensor(view);
    const count = view.edits().length;
    const again = await view.until("the attach handle", () => view.query('[data-part="attach-handle"]'));
    view.mouse("mousedown", again, Number(again.getAttribute("cx")), Number(again.getAttribute("cy")));
    view.mouse("mouseup", view.window, 900, 650);
    await new Promise((resolve) => setTimeout(resolve, 300));
    assert.strictEqual(view.edits().length, count, "released on nothing, nothing changes");
  });
});
