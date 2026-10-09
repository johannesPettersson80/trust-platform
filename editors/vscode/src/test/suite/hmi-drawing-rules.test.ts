import * as assert from "assert";

import type { HmiObjectModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { applyRules, Rule } from "../../hmiBuilder/drawing/symbols/rules";

// Specification 39, §5.3 items 3 and 5: visualization rules map bindings to a symbol's parts:
// turn a part, stand it at an angle, fill a level, move it along its travel, blink while an
// alarm is unacknowledged, show a value, paint it in the alarm colour. Motion and blinking show
// a current observation only.

const OBJECT: HmiObjectModel = {
  id: "t",
  type: "tank",
  at: [0, 0],
  size: [128, 128],
  rotation: 0,
  bind: {},
  min: 0,
  max: 4,
  unit: "m",
  decimals: 2,
};

function frame(values: Record<string, LiveValue | undefined>) {
  return {
    value: (name: string) => values[name],
    frame: { size: [1280, 720] as [number, number], values: {}, nowMs: 2_500 },
  };
}

const confirmed = (value: LiveValue["value"]): LiveValue => ({ value, state: "confirmed" });

suite("HMI drawing: visualization rules", () => {
  test("a part turns while its binding is TRUE and a current observation, continuing its turn across redraws", () => {
    const rules: Rule[] = [{ part: "fan", spin: "running" }];
    const at = (running: LiveValue | undefined) => {
      const { value, frame: shown } = frame({ running });
      return applyRules(rules, value, shown, OBJECT).fan?.spin;
    };
    assert.deepStrictEqual(at(confirmed(true)), { delayMs: 500 });
    assert.strictEqual(at(confirmed(false)), undefined);
    assert.strictEqual(at({ value: true, state: "confirmed", staleMs: 9_000 }), undefined, "not while stale");
    assert.strictEqual(at({ value: true, state: "commanded", target: true }), undefined, "not while only commanded");
  });

  test("a part stands at its angle, fills its level and moves along its travel by the object's range", () => {
    const rules: Rule[] = [
      { part: "needle", angle: "actual" },
      { part: "liquid", level: "level" },
      { part: "load", travel: "position" },
    ];
    const { value, frame: shown } = frame({ actual: confirmed(135), level: confirmed(1), position: confirmed(5) });
    const parts = applyRules(rules, value, shown, OBJECT);
    assert.strictEqual(parts.needle?.angle, 135);
    assert.strictEqual(parts.liquid?.level, 0.25, "1 m of 0 to 4 m");
    assert.strictEqual(parts.load?.travel, 1, "beyond the range, at its end");
    const unbound = applyRules(rules, frame({}).value, shown, OBJECT);
    assert.deepStrictEqual(unbound, {}, "an unbound rule leaves its part as drawn");
  });

  test("a part works while its binding is TRUE or above zero, as a heater's element heats", () => {
    const rules: Rule[] = [{ part: "element", active: "power" }];
    const at = (power: LiveValue | undefined) => {
      const { value, frame: shown } = frame({ power });
      return applyRules(rules, value, shown, OBJECT).element?.active;
    };
    assert.strictEqual(at(confirmed(35)), true);
    assert.strictEqual(at(confirmed(true)), true);
    assert.strictEqual(at(confirmed(0)), undefined);
    assert.strictEqual(at(confirmed(false)), undefined);
    assert.strictEqual(at({ value: null, state: "unknown" }), undefined);
  });

  test("a part blinks while its alarm is unacknowledged and fresh, shows a value, and takes the alarm colour while its binding is TRUE", () => {
    const rules: Rule[] = [
      { part: "body", blink: "unacknowledged" },
      { part: "readout", show: "level" },
      { part: "link", alarm: "lost" },
    ];
    const fresh = frame({ unacknowledged: confirmed(true), level: confirmed(1.234), lost: confirmed(true) });
    const parts = applyRules(rules, fresh.value, fresh.frame, OBJECT);
    assert.strictEqual(parts.body?.blink, true);
    assert.strictEqual(parts.readout?.text, "1.23 m");
    assert.strictEqual(parts.link?.alarm, true);
    const stale = frame({ unacknowledged: { value: true, state: "confirmed", staleMs: 9_000 } });
    assert.strictEqual(
      applyRules(rules, stale.value, stale.frame, OBJECT).body?.blink,
      undefined,
      "no blinking on old data",
    );
  });
});
