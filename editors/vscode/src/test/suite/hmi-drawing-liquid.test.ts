import * as assert from "assert";

import { renderToString } from "../../hmiBuilder/drawing/svg";
import { SYMBOLS } from "../../hmiBuilder/drawing/symbols";
import { drawObject, part, stateWord, symbolObject, valuesOf } from "./symbolTestKit";

// Specification 39, §5.2 phase 2 (liquid) and §5.3 items 4 and 5: tanks filled to their level, a
// tank cascade, the transfer pump in fixed and variable speed, the dosing pump, the control valve
// with positioner and stem position, the supply line and the mixing point.

suite("HMI drawing: liquid equipment", () => {
  test("an open tank is filled to its level by the object's range, and its word gives the level", () => {
    const tank = symbolObject("tank", ["level"], { min: 0, max: 4, unit: "m", decimals: 2 });
    const drawn = drawObject(tank, valuesOf({ level: 1 }));
    assert.strictEqual(Number(part(drawn, "liquid")?.attrs["data-level"]), 0.25, renderToString(drawn));
    assert.strictEqual(stateWord(drawn), "1.00 m");
    assert.deepStrictEqual(Object.keys(SYMBOLS.tank.ports).sort(), ["inlet", "outlet"]);
    assert.strictEqual(SYMBOLS.tank.ports.inlet?.direction, "up", "it is filled from above");
    const unknown = drawObject(tank, valuesOf({ level: { value: null, state: "unknown" } }));
    assert.strictEqual(part(unknown, "liquid"), undefined, "no liquid while the level is unknown");
    assert.strictEqual(stateWord(unknown), "?");
  });

  test("a gravity-drained tank empties through its bottom outlet, and a cascade's two tanks each show their level", () => {
    assert.strictEqual(SYMBOLS["gravity-tank"].ports.outlet?.direction, "down");
    const gravity = drawObject(symbolObject("gravity-tank", ["level"]), valuesOf({ level: 80 }));
    assert.strictEqual(Number(part(gravity, "liquid")?.attrs["data-level"]), 0.8);
    const cascade = drawObject(
      symbolObject("tank-cascade", ["upper-level", "lower-level"]),
      valuesOf({ "upper-level": 30, "lower-level": 70 }),
    );
    assert.strictEqual(Number(part(cascade, "upper-liquid")?.attrs["data-level"]), 0.3);
    assert.strictEqual(Number(part(cascade, "lower-liquid")?.attrs["data-level"]), 0.7);
  });

  test("a variable-speed pump carries the variability arrow and gives its speed while it runs; a fixed-speed one has no arrow", () => {
    const variable = symbolObject("pump", ["running", "speed"], { variant: "variable-speed" });
    const running = drawObject(variable, valuesOf({ running: true, speed: 62 }));
    assert.ok(part(running, "variable"), renderToString(running));
    assert.strictEqual(stateWord(running), "62 %");
    assert.strictEqual(part(running, "impeller")?.attrs.class, "hmi-spin");
    assert.strictEqual(stateWord(drawObject(variable, valuesOf({ running: false, speed: 0 }))), "Stopped");
    assert.strictEqual(
      part(drawObject(symbolObject("pump", ["running"]), valuesOf({ running: true })), "variable"),
      undefined,
    );
  });

  test("a control valve with a positioner shows where the positioner sends the stem, and the word gives position and demand while they differ", () => {
    // At the catalogue's width, which has room for both numbers.
    const valve = symbolObject("control-valve", ["position", "demand"], { variant: "positioner", size: [128, 108] });
    const moving = drawObject(valve, valuesOf({ position: 62, demand: 70 }));
    assert.ok(part(moving, "positioner"), renderToString(moving));
    assert.strictEqual(Number(part(moving, "demand")?.attrs["data-travel"]), 0.7);
    assert.strictEqual(stateWord(moving), "62 % → 70 %");
    assert.strictEqual(
      stateWord(drawObject(valve, valuesOf({ position: 70, demand: 70 }))),
      "70 %",
      "at its demand, the position alone",
    );
  });

  test("a dosing pump's piston strokes while it runs and stands still when stopped, and it gives its rate while running", () => {
    const dosing = symbolObject("dosing-pump", ["running", "rate"], { unit: "L/h", decimals: 1 });
    const running = drawObject(dosing, valuesOf({ running: true, rate: 1.25 }));
    assert.strictEqual(part(running, "piston")?.attrs.class, "hmi-stroke", renderToString(running));
    assert.strictEqual(stateWord(running), "1.3 L/h");
    const stopped = drawObject(dosing, valuesOf({ running: false, rate: 0 }));
    assert.strictEqual(part(stopped, "piston")?.attrs.class, undefined);
    assert.strictEqual(stateWord(stopped), "Stopped");
  });

  test("a supply line gives its pressure, a regulated one draws its regulator, and a mixing point joins two inlets into one outlet", () => {
    const fixed = drawObject(
      symbolObject("supply", ["pressure"], { unit: "bar", decimals: 1 }),
      valuesOf({ pressure: 3.24 }),
    );
    assert.strictEqual(stateWord(fixed), "3.2 bar");
    assert.strictEqual(part(fixed, "regulator"), undefined);
    assert.ok(
      part(
        drawObject(symbolObject("supply", ["pressure"], { variant: "regulated" }), valuesOf({ pressure: 3 })),
        "regulator",
      ),
    );
    assert.strictEqual(SYMBOLS.supply.ports.outlet?.direction, "right");
    assert.deepStrictEqual(Object.keys(SYMBOLS.mixer.ports).sort(), ["inlet", "inlet-2", "outlet"]);
  });
});
