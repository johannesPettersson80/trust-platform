import * as assert from "assert";

import { renderToString } from "../../hmiBuilder/drawing/svg";
import { SYMBOLS } from "../../hmiBuilder/drawing/symbols";
import { drawObject, part, stateWord, symbolObject, valuesOf, words } from "./symbolTestKit";

// Specification 39, §5.2 and §8.8; specification 41, §7 and §8: the symbols of the equipment PLC
// programmers need: fan, blower, agitator and conveyor (machines that turn while they run),
// damper, cylinder and motor valve (actuators), weigh scale, analyzer and the level and pressure
// switches.

suite("HMI drawing: plant equipment", () => {
  test("a fan, a blower, an agitator and a conveyor turn while they run and say Running or Stopped", () => {
    for (const type of ["fan", "blower", "agitator", "conveyor"]) {
      const object = symbolObject(type as never, ["running"]);
      const running = drawObject(object, valuesOf({ running: true }));
      assert.strictEqual(part(running, "rotor")?.attrs.class, "hmi-spin", `${type}: ${renderToString(running)}`);
      assert.strictEqual(stateWord(running), "Running", type);
      const stopped = drawObject(object, valuesOf({ running: false }));
      assert.ok(part(stopped, "rotor"), type);
      assert.strictEqual(part(stopped, "rotor")?.attrs.class, undefined, type);
      assert.strictEqual(stateWord(stopped), "Stopped", type);
    }
  });

  test("a variable-speed fan carries the arrow of variability and gives its speed while it runs", () => {
    const fan = symbolObject("fan" as never, ["running", "speed"], { variant: "variable-speed" });
    const drawn = drawObject(fan, valuesOf({ running: true, speed: 62 }));
    assert.ok(part(drawn, "variable"), renderToString(drawn));
    assert.strictEqual(stateWord(drawn), "62 %");
  });

  test("a damper's blade lies along the duct when open and across it when closed, and turns with its position", () => {
    const onOff = symbolObject("damper" as never, ["open", "closed"]);
    assert.strictEqual(
      Number(part(drawObject(onOff, valuesOf({ open: true, closed: false })), "blade")?.attrs["data-angle"]),
      0,
    );
    const closed = drawObject(onOff, valuesOf({ open: false, closed: true }));
    assert.strictEqual(Number(part(closed, "blade")?.attrs["data-angle"]), 90);
    assert.strictEqual(stateWord(closed), "Closed");
    const moving = SYMBOLS.damper.states.find((state) => state.id === "moving");
    assert.deepStrictEqual(moving?.values.position, { share: 0.5 }, "the Moving preview stands half open");
    const modulating = drawObject(symbolObject("damper" as never, ["position"]), valuesOf({ position: 25 }));
    assert.strictEqual(Number(part(modulating, "blade")?.attrs["data-angle"]), 67.5, renderToString(modulating));
    assert.strictEqual(stateWord(modulating), "25 %");
  });

  test("a cylinder's rod is out when extended and in when retracted, and says so", () => {
    const cylinder = symbolObject("cylinder" as never, ["open", "closed"]);
    const extended = drawObject(cylinder, valuesOf({ open: true, closed: false }));
    assert.strictEqual(Number(part(extended, "rod")?.attrs["data-travel"]), 1, renderToString(extended));
    assert.strictEqual(stateWord(extended), "Extended");
    const retracted = drawObject(cylinder, valuesOf({ open: false, closed: true }));
    assert.strictEqual(Number(part(retracted, "rod")?.attrs["data-travel"]), 0);
    assert.strictEqual(stateWord(retracted), "Retracted");
  });

  test("a motor valve has its motor on the stem; its body is filled when closed", () => {
    const valve = symbolObject("motor-valve" as never, ["open", "closed"]);
    const closed = drawObject(valve, valuesOf({ open: false, closed: true }));
    assert.strictEqual(words(part(closed, "motor")), "M", renderToString(closed));
    assert.strictEqual(part(closed, "body")?.attrs["data-filled"], "true");
    assert.strictEqual(stateWord(drawObject(valve, valuesOf({ open: true, closed: false }))), "Open");
  });

  test("a weigh scale gives its weight; the analyzer and the switches carry their ISA letters", () => {
    const scale = drawObject(
      symbolObject("weigh-scale" as never, ["value"], { unit: "kg", decimals: 1 }),
      valuesOf({ value: 50 }),
    );
    assert.strictEqual(stateWord(scale), "50.0 kg", renderToString(scale));
    assert.ok(part(scale, "platform"));
    const analyzer = drawObject(
      symbolObject("analyzer" as never, ["value"], { size: [176, 56] }),
      valuesOf({ value: 7.2 }),
    );
    assert.strictEqual(words(part(analyzer, "letters")), "AT");
    for (const [type, letters] of [
      ["level-switch", "LS"],
      ["pressure-switch", "PS"],
    ]) {
      const object = symbolObject(type as never, ["state"], { size: [176, 56] });
      const high = drawObject(object, valuesOf({ state: true }));
      assert.strictEqual(words(part(high, "letters")), letters, type);
      assert.strictEqual(stateWord(high), "On", type);
      assert.strictEqual(stateWord(drawObject(object, valuesOf({ state: false }))), "Off", type);
    }
  });
});
