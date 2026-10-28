import * as assert from "assert";

import { drawPage } from "../../hmiBuilder/drawing";
import type { HmiObjectModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { findAll, renderToString } from "../../hmiBuilder/drawing/svg";

// Specification 39, §18 phase 5 item 2: an object with a visibility binding is shown while its
// BOOL is TRUE and hidden while it is FALSE; while the value is unknown it stays shown, so nothing
// disappears on a lost connection.

const LEVEL: HmiObjectModel = {
  id: "level",
  type: "value",
  at: [40, 40],
  size: [240, 96],
  rotation: 0,
  label: "Level",
  bind: { value: { path: "Main.Level", direction: "read" }, visible: { path: "Main.Shown", direction: "read" } },
};

function shown(visible: LiveValue | undefined): boolean {
  const values: Record<string, LiveValue> = { "Main.Level": { value: 1.5, state: "confirmed" } };
  if (visible) values["Main.Shown"] = visible;
  const svg = drawPage({ title: "Page", object: [LEVEL], pipe: [] }, { size: [1280, 720], values });
  const found = findAll(svg, (node) => node.attrs["data-object"] === "level");
  assert.strictEqual(found.length, 1, renderToString(svg));
  return found[0].attrs["data-hidden"] !== "true" && found[0].children.length > 0;
}

suite("HMI drawing: visibility", () => {
  test("an object is shown while its visibility is TRUE, hidden while FALSE, and shown while unknown", () => {
    assert.strictEqual(shown({ value: true, state: "confirmed" }), true);
    assert.strictEqual(shown({ value: false, state: "confirmed" }), false);
    assert.strictEqual(shown({ value: null, state: "unknown" }), true, "unknown stays shown");
    assert.strictEqual(shown(undefined), true, "not read yet");
    assert.strictEqual(
      shown({ value: false, state: "confirmed", staleMs: 9000 }),
      false,
      "the last value counts, stale",
    );
  });
});
