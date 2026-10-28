import * as assert from "assert";

import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import { object, Webview } from "./hmiBuilderWebview";

// Specification 39, §8.7: an object's click opens a faceplate, for the instance its click names or
// its own; a faceplate's description and fields are edited in the inspector, and its objects bind
// its fields, offered first in the binding picker.

const PAGE: HmiPageModel = { title: "Pumps", object: [object("lvl", "value", [40, 40], [160, 48])], pipe: [] };

const FACEPLATE: HmiPageModel = {
  title: "Pump",
  description: "Transfer pump",
  size: [400, 300],
  fields: { Running: { accepts: ["boolean"] }, Spare: { accepts: ["number"] } },
  object: [
    object("state", "status", [24, 24], [160, 48], { bind: { state: { path: "$Running", direction: "read" } } }),
    object("speed", "value", [24, 100], [160, 48]),
  ],
  pipe: [],
};

function select(view: Webview, at: [number, number]): void {
  view.mouse("mousedown", view.layer(), at[0], at[1]);
  view.mouse("mouseup", view.window, at[0], at[1]);
}

suite("HMI Builder editor: faceplates in the webview", function () {
  this.timeout(20000);

  test("an object's click opens a faceplate of the project, for the instance the click names", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.send({ type: "targets", pages: ["overview"], popups: [], faceplates: ["pump", "valve"] });
    select(view, [100, 60]);
    const kind = await view.until("On click", () => view.query('select[name="click-kind"]'));
    assert.ok(
      [...kind.querySelectorAll("option")].some((option: { value: string }) => option.value === "faceplate"),
      "a faceplate can be opened",
    );
    let before = view.edits().length;
    view.choose(kind, "faceplate");
    assert.deepStrictEqual((await view.nextEdit(before)).object[0].click, { faceplate: "pump" });
    before = view.edits().length;
    view.choose(view.query('select[name="click-target"]'), "valve");
    assert.deepStrictEqual((await view.nextEdit(before)).object[0].click, { faceplate: "valve" });
    before = view.edits().length;
    view.type(await view.until("the instance", () => view.query('input[name="click-instance"]')), "Main.P101");
    assert.deepStrictEqual((await view.nextEdit(before)).object[0].click, {
      faceplate: "valve",
      instance: "Main.P101",
    });
  });

  test("a faceplate's description and fields are edited in the inspector; a field in use cannot be removed", async () => {
    const view = new Webview();
    await view.loaded(FACEPLATE);
    const description = await view.until("the description", () =>
      view.query('[data-part="faceplate"] input[name="description"]'),
    );
    assert.strictEqual(description.value, "Transfer pump");
    assert.strictEqual(
      view.query('[data-field="Running"] [data-action="remove-field"]').disabled,
      true,
      "$Running is bound",
    );
    let before = view.edits().length;
    view.type(view.query('input[name="new-field"]'), "Speed");
    view.choose(view.query('select[name="new-field-kind"]'), "number");
    view.query('[data-action="add-field"]').click();
    assert.deepStrictEqual((await view.nextEdit(before)).fields, {
      Running: { accepts: ["boolean"] },
      Spare: { accepts: ["number"] },
      Speed: { accepts: ["number"] },
    });
    before = view.edits().length;
    view.query('[data-field="Spare"] [data-action="remove-field"]').click();
    assert.deepStrictEqual(Object.keys((await view.nextEdit(before)).fields ?? {}), ["Running", "Speed"]);
  });

  test("the binding picker offers a faceplate's fields that fit the property, and picking one binds it", async () => {
    const view = new Webview();
    await view.loaded(FACEPLATE);
    select(view, [100, 120]);
    view.send({ type: "variables", variables: [] });
    const pick = await view.until("the value's picker button", () =>
      view.query('[data-binding="value"] [data-action="pick"]'),
    );
    pick.click();
    const offered = await view.until("the fields in the picker", () =>
      view.query('[data-part="picker"] [data-variable="$Spare"]'),
    );
    assert.strictEqual(
      view.query('[data-part="picker"] [data-variable="$Running"]') !== null,
      true,
      "a value takes a boolean too",
    );
    const before = view.edits().length;
    offered.click();
    assert.deepStrictEqual((await view.nextEdit(before)).object[1].bind.value, { path: "$Spare", direction: "read" });
  });
});
