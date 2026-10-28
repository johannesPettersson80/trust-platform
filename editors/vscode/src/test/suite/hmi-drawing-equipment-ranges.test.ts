import * as assert from "assert";
import { LiveValueStore, StreamMessage } from "../../hmiBuilder/display/liveValues";
import { drawPage } from "../../hmiBuilder/drawing";
import type { HmiObjectModel } from "../../hmiBuilder/drawing/model";
import { findAll, textsOf } from "../../hmiBuilder/drawing/svg";

suite("HMI drawing: live equipment presentation", () => {
  for (const type of ["bar", "gauge"]) {
    test(`${type} follows live equipment units and actual limits without moving the value or editing the page`, () => {
      const object: HmiObjectModel = {
        id: "reading",
        type,
        at: [0, 0],
        size: [320, 320],
        rotation: 0,
        limits: { low: 1, high: 2 },
        bind: { value: { path: "Main.TT.Value", direction: "read" } },
      };
      const original = JSON.stringify(object);
      const store = new LiveValueStore();
      const update = (unit: string, high: number, cycle: number) =>
        store.receive(
          {
            cycle,
            values: {
              "Main.TT.Value": {
                value: 100,
                presentation: {
                  unit,
                  range: [0, 200],
                  limits: { "low-low": 5, low: 10, high, "high-high": 150 },
                  priorities: { "low-low": "high", low: "low", high: "medium", "high-high": "high" },
                },
              },
            },
          } as StreamMessage,
          cycle * 250,
        );
      const draw = (time: number) =>
        drawPage(
          { title: "Measured", object: [object], pipe: [] },
          { size: [640, 480], ...store.frame(time), formats: { "main.tt.value": { unit: "fallback", decimals: 2 } } },
        );
      update("°C", 120, 1);
      let svg = draw(250);
      assert.ok(
        textsOf(svg).some((word) => word === "°C"),
        "the block's unit beats the inherited project fallback",
      );
      assert.ok(textsOf(svg).includes("100.00"));
      let ticks = findAll(svg, (node) => node.attrs["data-limit"] !== undefined);
      assert.strictEqual(ticks.length, 4, "actual equipment limits replace manual fallback marks");
      assert.strictEqual(
        ticks.find((tick) => tick.attrs["data-limit"] === "low")?.attrs.stroke,
        "var(--hmi-alarm-low)",
      );
      assert.strictEqual(
        Number(ticks.find((tick) => tick.attrs["data-limit"] === "high")?.attrs["data-fraction"]),
        0.6,
      );
      update("K", 140, 2);
      svg = draw(500);
      ticks = findAll(svg, (node) => node.attrs["data-limit"] !== undefined);
      assert.ok(textsOf(svg).includes("K"));
      assert.ok(textsOf(svg).includes("100.00"));
      assert.strictEqual(
        Number(ticks.find((tick) => tick.attrs["data-limit"] === "high")?.attrs["data-fraction"]),
        0.7,
      );
      assert.strictEqual(JSON.stringify(object), original, "the authored page is untouched");
    });
  }
});
