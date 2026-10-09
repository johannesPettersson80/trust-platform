import * as assert from "assert";

import { renderToString } from "../../hmiBuilder/drawing/svg";
import { SYMBOLS } from "../../hmiBuilder/drawing/symbols";
import { drawObject, part, stateWord, symbolObject, valuesOf } from "./symbolTestKit";

// Specification 39, §5.2 phase 2 (thermal) and §5.3 items 4 and 5: the cold room with its door,
// the refrigeration unit, the electric heater with its water vessel, the cooler, the jacketed
// vessel, the coupled vessels with their heat-transfer wall, and the heat draw-off.

suite("HMI drawing: thermal equipment", () => {
  test("a cold room's door stands open or shut as its binding says, and its word gives the room's temperature", () => {
    const room = symbolObject("cold-room", ["door-open", "temperature"], { unit: "°C", decimals: 1, size: [160, 108] });
    const shut = drawObject(room, valuesOf({ "door-open": false, temperature: 3.24 }));
    assert.strictEqual(part(shut, "door")?.attrs["data-open"], "false", renderToString(shut));
    assert.strictEqual(stateWord(shut), "3.2 °C");
    const open = drawObject(room, valuesOf({ "door-open": true, temperature: 6.5 }));
    assert.strictEqual(part(open, "door")?.attrs["data-open"], "true");
    assert.ok(part(open, "swing"), "an open door shows its swing");
  });

  test("a refrigeration unit's fan and a cooler's fan turn while they run", () => {
    for (const type of ["refrigeration-unit", "cooler"] as const) {
      const unit = symbolObject(type, ["running"], { size: [160, 108] });
      assert.strictEqual(part(drawObject(unit, valuesOf({ running: true })), "fan")?.attrs.class, "hmi-spin", type);
      const stopped = drawObject(unit, valuesOf({ running: false }));
      assert.strictEqual(part(stopped, "fan")?.attrs.class, undefined, type);
      assert.strictEqual(stateWord(stopped), "Stopped", type);
    }
  });

  test("a heater's element works and its heat rises while it has power; its word gives the water's temperature", () => {
    const heater = symbolObject("heater", ["power", "temperature", "level"], {
      unit: "°C",
      decimals: 1,
      size: [128, 108],
    });
    const heating = drawObject(heater, valuesOf({ power: 40, temperature: 55.04, level: 80 }));
    assert.strictEqual(part(heating, "element")?.attrs["data-active"], "true", renderToString(heating));
    assert.ok(part(heating, "heat"), "the heat rises");
    assert.strictEqual(stateWord(heating), "55.0 °C");
    assert.strictEqual(Number(part(heating, "liquid")?.attrs["data-level"]), 0.8);
    const cold = drawObject(heater, valuesOf({ power: 0, temperature: 20, level: 80 }));
    assert.strictEqual(part(cold, "element")?.attrs["data-active"], "false");
    assert.strictEqual(part(cold, "heat"), undefined);
  });

  test("a jacketed vessel has its jacket's inlet and outlet besides its own, and is filled to its level", () => {
    assert.deepStrictEqual(Object.keys(SYMBOLS["jacketed-vessel"].ports).sort(), [
      "inlet",
      "jacket-inlet",
      "jacket-outlet",
      "outlet",
    ]);
    const vessel = drawObject(
      symbolObject("jacketed-vessel", ["level", "temperature"], { unit: "°C", decimals: 1 }),
      valuesOf({ level: 50, temperature: 41.25 }),
    );
    assert.ok(part(vessel, "jacket"), renderToString(vessel));
    assert.strictEqual(Number(part(vessel, "liquid")?.attrs["data-level"]), 0.5);
  });

  test("coupled vessels share a heat-transfer wall, show the heat passing it from the warmer side, and give both temperatures", () => {
    const vessels = symbolObject("coupled-vessels", ["temperature-1", "temperature-2"], {
      unit: "°C",
      decimals: 1,
      size: [192, 108],
    });
    const drawn = drawObject(vessels, valuesOf({ "temperature-1": 45.21, "temperature-2": 38.1 }));
    assert.ok(part(drawn, "wall"), renderToString(drawn));
    assert.strictEqual(stateWord(drawn), "45.2 °C · 38.1 °C");
    assert.strictEqual(
      part(drawn, "heat-flow")?.attrs["data-direction"],
      "right",
      "from the warmer left vessel to the right",
    );
    assert.strictEqual(
      part(drawObject(vessels, valuesOf({ "temperature-1": 30, "temperature-2": 41 })), "heat-flow")?.attrs[
        "data-direction"
      ],
      "left",
    );
    assert.strictEqual(
      part(drawObject(vessels, valuesOf({ "temperature-1": 40, "temperature-2": 40.2 })), "heat-flow"),
      undefined,
      "no flow worth showing between even vessels",
    );
  });

  test("a heat draw-off shows the heat leaving while it draws, and gives its power", () => {
    const draw = symbolObject("heat-draw-off", ["power"], { unit: "kW", decimals: 1, size: [128, 108] });
    const drawing = drawObject(draw, valuesOf({ power: 1.24 }));
    assert.ok(part(drawing, "heat"), renderToString(drawing));
    assert.strictEqual(stateWord(drawing), "1.2 kW");
    assert.strictEqual(part(drawObject(draw, valuesOf({ power: 0 })), "heat"), undefined);
  });
});
