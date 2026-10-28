import * as assert from "assert";
import { drawPage } from "../../hmiBuilder/drawing";
import type { DrawFrame, HmiObjectModel } from "../../hmiBuilder/drawing/model";
import { textsOf } from "../../hmiBuilder/drawing/svg";
import { applyRules } from "../../hmiBuilder/drawing/symbols/rules";

function draw(type: string, extra: Partial<HmiObjectModel> = {}): string[] {
  const bind =
    type === "trend"
      ? { Level: { path: "Main.Level", direction: "read" } }
      : type === "split-bar"
        ? { first: { path: "Main.Level", direction: "read" }, second: { path: "Main.Other", direction: "read" } }
        : type === "tank"
          ? { level: { path: "Main.Level", direction: "read" } }
          : type === "vfd"
            ? { frequency: { path: "Main.Level", direction: "read" }, running: { path: "Main.Run", direction: "read" } }
            : type === "heat-draw-off"
              ? { power: { path: "Main.Level", direction: "read" } }
              : type === "controller"
                ? { pv: { path: "Main.Level", direction: "read" }, sp: { path: "Main.Other", direction: "read" } }
                : type === "axis-indicator"
                  ? {
                      actual: { path: "Main.Level", direction: "read" },
                      target: { path: "Main.Other", direction: "read" },
                    }
                  : { value: { path: "Main.Level", direction: "read" } };
  const object = {
    id: "value",
    type,
    at: [0, 0],
    size: [320, 240],
    rotation: 0,
    min: 0,
    max: 3,
    bind,
    ...extra,
  } as HmiObjectModel;
  const frame = {
    size: [640, 400],
    values: {
      "Main.Run": { value: true, state: "confirmed" },
      "Main.Level": { value: 1.2345, state: "confirmed" },
      "Main.Other": { value: 2.345, state: "confirmed" },
    },
    formats: { "main.level": { unit: "m", decimals: 2 }, "main.other": { unit: "m", decimals: 1 } },
  } as DrawFrame;
  return textsOf(drawPage({ title: "Formats", object: [object], pipe: [] }, frame));
}

suite("HMI drawing: inherited value formats", () => {
  for (const type of [
    "value",
    "bar",
    "gauge",
    "setpoint",
    "numeric-input",
    "slider",
    "trend",
    "split-bar",
    "tank",
    "vfd",
    "heat-draw-off",
    "controller",
    "axis-indicator",
  ]) {
    test(`${type} uses the bound value's project format`, () => {
      const words = draw(type);
      assert.ok(
        words.some((word) => /(?:^|\s)1\.23(?:\s|$)/.test(word)),
        `the shared precision is shown: ${words.join(" | ")}`,
      );
      assert.ok(
        words.some((word) => /(?:^|\s)m(?:\s|$)/.test(word)),
        `the shared unit is shown: ${words.join(" | ")}`,
      );
      if (type === "split-bar") assert.ok(words.includes("2.3"), "each output uses its own path's precision");
    });
  }
  test("a symbol's show rule uses the bound property's format", () => {
    const object = {
      id: "custom",
      type: "my:custom",
      at: [0, 0],
      size: [160, 80],
      rotation: 0,
      bind: { reading: { path: "Main.Level", direction: "read" } },
    } as HmiObjectModel;
    const parts = applyRules(
      [{ part: "number", show: "reading" }],
      () => ({ value: 1.2345, state: "confirmed" }),
      { size: [160, 80], values: {}, formats: { "main.level": { unit: "m", decimals: 2 } } },
      object,
    );
    assert.strictEqual(parts.number.text, "1.23 m");
  });
  test("an object's own unit and decimals override the project format", () => {
    const words = draw("value", { unit: "custom", decimals: 0 });
    assert.ok(words.includes("1"));
    assert.ok(words.includes("custom"));
    assert.ok(!words.includes("m"));
  });
});
