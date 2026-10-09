import * as assert from "assert";

import { renderToString } from "../../hmiBuilder/drawing/svg";
import { drawObject, part, stateWord, symbolObject, valuesOf } from "./symbolTestKit";

// Specification 39, §5.2 phase 2 (motion) and §5.3 items 4 and 5: the servo motor with its drive,
// the shaft, the drum with its hanging load, the flexible coupling with its load, the encoder,
// the rotary arm and the axis position indicator; what turns stands at its angle.

function angleOf(drawn: ReturnType<typeof drawObject>, name: string): number {
  return Number(part(drawn, name)?.attrs["data-angle"]);
}

suite("HMI drawing: motion equipment", () => {
  test("a motor is the IEC circle with M; its rotor turns while it runs, and its word says Running or Stopped", () => {
    const motor = symbolObject("motor" as never, ["running"]);
    const running = drawObject(motor, valuesOf({ running: true }));
    assert.strictEqual(part(running, "rotor")?.attrs.class, "hmi-spin", renderToString(running));
    assert.strictEqual(stateWord(running), "Running");
    const stopped = drawObject(motor, valuesOf({ running: false }));
    assert.ok(part(stopped, "rotor"), renderToString(stopped));
    assert.strictEqual(part(stopped, "rotor")?.attrs.class, undefined, "still while stopped");
    assert.strictEqual(stateWord(stopped), "Stopped");
  });

  test("a servo motor's rotor stands at its angle, or turns while it runs; its word gives its speed while it runs", () => {
    const motor = symbolObject("servo-motor", ["running", "speed", "angle"], {
      unit: "rpm",
      decimals: 0,
      size: [160, 108],
    });
    const placed = drawObject(motor, valuesOf({ running: true, speed: 1500, angle: 30 }));
    assert.strictEqual(angleOf(placed, "rotor"), 30, renderToString(placed));
    assert.strictEqual(stateWord(placed), "1500 rpm");
    const spinning = drawObject(
      symbolObject("servo-motor", ["running", "speed"], { unit: "rpm", size: [160, 108] }),
      valuesOf({ running: true, speed: 900 }),
    );
    assert.strictEqual(part(spinning, "rotor")?.attrs.class, "hmi-spin");
    assert.strictEqual(stateWord(drawObject(motor, valuesOf({ running: false, speed: 0, angle: 30 }))), "Stopped");
  });

  test("a shaft, an encoder and a rotary arm stand at their angles and give them", () => {
    for (const [type, name] of [
      ["shaft", "marker"],
      ["encoder", "disc"],
      ["rotary-arm", "arm"],
    ] as const) {
      const drawn = drawObject(
        symbolObject(type, ["angle"], { unit: "°", decimals: 1, size: [128, 108] }),
        valuesOf({ angle: 123.45 }),
      );
      assert.strictEqual(angleOf(drawn, name), 123.45, `${type}: ${renderToString(drawn)}`);
      assert.strictEqual(stateWord(drawn), "123.5 °", type);
    }
  });

  test("a drum turns to its angle and its load hangs by its position", () => {
    const drum = symbolObject("drum", ["angle", "position"], {
      unit: "m",
      decimals: 2,
      min: 0,
      max: 2,
      size: [128, 172],
    });
    const drawn = drawObject(drum, valuesOf({ angle: 90, position: 0.5 }));
    assert.strictEqual(angleOf(drawn, "spokes"), 90, renderToString(drawn));
    assert.strictEqual(Number(part(drawn, "load")?.attrs["data-travel"]), 0.25);
    assert.strictEqual(stateWord(drawn), "0.50 m");
  });

  test("a flexible coupling shows its motor and load sides at their angles", () => {
    const coupling = symbolObject("coupling", ["motor-angle", "load-angle"], {
      unit: "°",
      decimals: 1,
      size: [192, 108],
    });
    const drawn = drawObject(coupling, valuesOf({ "motor-angle": 40, "load-angle": 32.5 }));
    assert.strictEqual(angleOf(drawn, "motor-marker"), 40, renderToString(drawn));
    assert.strictEqual(angleOf(drawn, "load-marker"), 32.5);
    assert.strictEqual(stateWord(drawn), "40.0 ° · 32.5 °");
  });

  test("an axis position indicator shows the actual and the target angle, and both in its word while they differ", () => {
    const axis = symbolObject("axis-indicator", ["actual", "target"], { unit: "°", decimals: 1, size: [160, 172] });
    const following = drawObject(axis, valuesOf({ actual: 87.5, target: 90 }));
    assert.strictEqual(angleOf(following, "actual"), 87.5, renderToString(following));
    assert.strictEqual(angleOf(following, "target"), 90);
    assert.strictEqual(stateWord(following), "87.5 ° → 90.0 °");
    assert.strictEqual(
      stateWord(drawObject(axis, valuesOf({ actual: 90, target: 90 }))),
      "90.0 °",
      "on target, the angle alone",
    );
  });
});
