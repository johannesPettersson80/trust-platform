import * as assert from "assert";

import type { LiveValue } from "../../hmiBuilder/drawing/model";
import { LiveValueStore } from "../../hmiBuilder/display/liveValues";

// Specification 39, §11.1 and §5.3: live values in the runtime display.

function valueAt(store: LiveValueStore, nowMs: number, path: string): LiveValue {
  const value = store.frame(nowMs).values[path];
  assert.ok(value, `a value for ${path} at ${nowMs} ms`);
  return value;
}

suite("HMI display: live values", () => {
  test("a received value is confirmed, with an array's lower bounds; a value that does not resolve is unknown", () => {
    const store = new LiveValueStore();
    store.receive({ period_ms: 250 }, 0);
    store.receive(
      {
        cycle: 1,
        values: { "Pump.Speed": { value: 1180 }, "Line.Data": { value: [1, 2], lowerBounds: [1] }, "Gone.Path": null },
      },
      100,
    );
    const { values } = store.frame(150);
    assert.deepStrictEqual(values["Pump.Speed"], { value: 1180, state: "confirmed" });
    assert.deepStrictEqual(values["Line.Data"], { value: [1, 2], state: "confirmed", lowerBounds: [1] });
    assert.deepStrictEqual(values["Gone.Path"], { value: null, state: "unknown" });
  });

  test("the paths the running program does not have come with the frame, kept until the stream sends a new list", () => {
    const store = new LiveValueStore();
    store.receive(
      { cycle: 1, values: { "Pump.Speed": { value: 1180 }, "Gone.Path": null }, missing: ["Gone.Path"] },
      100,
    );
    assert.deepStrictEqual(store.frame(150).missing, ["Gone.Path"]);
    store.receive({ cycle: 2, values: {} }, 350);
    assert.deepStrictEqual(store.frame(400).missing, ["Gone.Path"], "an update without the list keeps it");
    store.receive({ cycle: 3, values: { "Gone.Path": { value: 3 } }, missing: [] }, 600);
    assert.deepStrictEqual(store.frame(650).missing, [], "the program has it again");
  });

  test("an update carries only the values that changed: the others stay, and the cycle alone keeps them fresh (§18 phase 5)", () => {
    const store = new LiveValueStore();
    store.receive({ period_ms: 250 }, 0);
    store.receive({ cycle: 1, values: { A: { value: 1 }, B: { value: 2 } } }, 0);
    for (const [cycle, at] of [
      [2, 250],
      [3, 500],
      [4, 750],
      [5, 1000],
    ])
      store.receive({ cycle, values: cycle === 3 ? { A: { value: 5 } } : {} }, at);
    assert.deepStrictEqual(store.frame(1100).values.A, { value: 5, state: "confirmed" });
    assert.deepStrictEqual(store.frame(1100).values.B, { value: 2, state: "confirmed" }, "unchanged and fresh");
  });

  test("values turn stale with their age when the cycle count stops for three update periods", () => {
    const store = new LiveValueStore();
    store.receive({ period_ms: 250 }, 0);
    for (const at of [0, 250, 500, 750, 1000]) store.receive({ cycle: 7, values: { A: { value: 1 } } }, at);
    assert.deepStrictEqual(store.frame(700).values.A, { value: 1, state: "confirmed" });
    assert.deepStrictEqual(store.frame(800).values.A, { value: 1, state: "confirmed", staleMs: 800 });
  });

  test("values turn stale when no message arrives for three update periods, and at once on disconnect", () => {
    const store = new LiveValueStore();
    store.receive({ period_ms: 500 }, 0);
    store.receive({ cycle: 1, values: { A: { value: 1 } } }, 0);
    store.receive({ cycle: 2, values: { A: { value: 2 } } }, 500);
    assert.strictEqual(valueAt(store, 1900, "A").staleMs, undefined);
    assert.strictEqual(valueAt(store, 2100, "A").staleMs, 1600);
    store.receive({ cycle: 3, values: { A: { value: 3 } } }, 2200);
    assert.strictEqual(valueAt(store, 2300, "A").staleMs, undefined);
    store.disconnected(2400);
    assert.strictEqual(valueAt(store, 2400, "A").staleMs, 200);
  });

  test("the update period defaults to 250 ms until the runtime sends it", () => {
    const store = new LiveValueStore();
    store.receive({ cycle: 1, values: { A: { value: 1 } } }, 0);
    assert.strictEqual(valueAt(store, 700, "A").staleMs, undefined);
    assert.strictEqual(valueAt(store, 800, "A").staleMs, 800);
  });

  test("an unknown value stays unknown when the stream stalls", () => {
    const store = new LiveValueStore();
    store.receive({ cycle: 1, values: { A: null } }, 0);
    assert.deepStrictEqual(store.frame(5000).values.A, { value: null, state: "unknown" });
  });

  test("trend paths keep a sample per new cycle, inside the longest span plus the newest older one", () => {
    const store = new LiveValueStore({ trendSpansMs: { T: 1000, Run: 1000 } });
    store.receive({ period_ms: 250 }, 0);
    for (let step = 0; step <= 8; step += 1) {
      store.receive(
        { cycle: step + 1, values: { T: { value: step }, Run: { value: step % 2 === 0 }, Other: { value: step } } },
        step * 250,
      );
    }
    store.receive({ cycle: 9, values: { T: { value: 99 } } }, 2100);
    const { history, nowMs } = store.frame(2200);
    assert.strictEqual(nowMs, 2200);
    assert.deepStrictEqual(history?.T, [
      [750, 3],
      [1000, 4],
      [1250, 5],
      [1500, 6],
      [1750, 7],
      [2000, 8],
    ]);
    assert.deepStrictEqual(history?.Run?.slice(0, 2), [
      [750, 0],
      [1000, 1],
    ]);
    assert.strictEqual(history?.Other, undefined, "only trend paths keep samples");
  });

  test("a value the runtime marks stale shows its age while the stream is fresh (specification 42, §5 item 4)", () => {
    const store = new LiveValueStore();
    store.receive({ period_ms: 250 }, 0);
    store.receive({ cycle: 1, values: { Level: { value: 7, staleMs: 2000 }, Other: { value: 3 } } }, 0);
    assert.deepStrictEqual(store.frame(100).values.Level, { value: 7, state: "confirmed", staleMs: 2000 });
    assert.deepStrictEqual(store.frame(100).values.Other, { value: 3, state: "confirmed" }, "a fresh value");
    store.receive({ cycle: 2, values: { Level: { value: 7 } } }, 250);
    assert.deepStrictEqual(store.frame(300).values.Level, { value: 7, state: "confirmed" }, "data again");
  });

  test("a trend path keeps a sample per new cycle while its value does not change", () => {
    const store = new LiveValueStore({ trendSpansMs: { T: 1000 } });
    store.receive({ period_ms: 250 }, 0);
    store.receive({ cycle: 1, values: { T: { value: 4 } } }, 0);
    store.receive({ cycle: 2, values: {} }, 250);
    store.receive({ cycle: 3, values: { Other: { value: 1 } } }, 500);
    store.receive({ cycle: 3, values: {} }, 600);
    assert.deepStrictEqual(
      store.frame(700).history?.T,
      [
        [0, 4],
        [250, 4],
        [500, 4],
      ],
      "one sample per new cycle",
    );
  });
});
