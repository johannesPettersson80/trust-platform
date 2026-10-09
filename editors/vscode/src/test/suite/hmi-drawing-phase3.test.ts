import * as assert from "assert";

import type { HmiObjectModel } from "../../hmiBuilder/drawing/model";
import { renderToString } from "../../hmiBuilder/drawing/svg";
import { SYMBOLS, SymbolDefinition, SymbolType } from "../../hmiBuilder/drawing/symbols";
import { MOVING, STILL } from "../../hmiBuilder/drawing/symbols/style";
import { drawObject, part, stateWord, symbolObject, valuesOf } from "./symbolTestKit";

// Specification 39, §5.2 phase 3: the process equipment (vessel, heat exchanger, filter, cyclone,
// crusher, screen) and the electrical equipment (VFD, drive, contactor, breaker).

/** The size the catalogue gives a new electrical object, wide enough for its longest word. */
const CATALOGUE_SIZE: [number, number] = [128, 108];

function symbol(type: string): SymbolDefinition {
  const found = (SYMBOLS as Record<string, SymbolDefinition | undefined>)[type];
  assert.ok(found, `the ${type} symbol`);
  return found;
}

function objectOf(type: string, properties: string[], extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  symbol(type);
  return symbolObject(type as SymbolType, properties, extra);
}

suite("HMI drawing: phase 3 equipment", () => {
  test("a vessel is closed by its dished heads and filled to its level, which the word gives", () => {
    const drawn = drawObject(objectOf("vessel", ["level"], { unit: "%" }), valuesOf({ level: 40 }));
    assert.ok(part(drawn, "heads"), renderToString(drawn));
    assert.strictEqual(Number(part(drawn, "liquid")?.attrs["data-level"]), 0.4);
    assert.strictEqual(stateWord(drawn), "40 %");
    assert.deepStrictEqual(Object.keys(symbol("vessel").ports).sort(), ["inlet", "outlet"]);
  });

  test("a heat exchanger has its product and medium ports, and gives the product's temperature or Fault", () => {
    const exchanger = objectOf("heat-exchanger", ["temperature", "fault"], { unit: "°C", decimals: 1 });
    const drawn = drawObject(exchanger, valuesOf({ temperature: 62.5, fault: false }));
    assert.ok(part(drawn, "shell") && part(drawn, "coil"), renderToString(drawn));
    assert.strictEqual(stateWord(drawn), "62.5 °C");
    assert.strictEqual(stateWord(drawObject(exchanger, valuesOf({ temperature: 62.5, fault: true }))), "Fault");
    assert.deepStrictEqual(Object.keys(symbol("heat-exchanger").ports).sort(), [
      "inlet",
      "medium-inlet",
      "medium-outlet",
      "outlet",
    ]);
  });

  test("a filter's medium lies across its housing; clogged, it turns to the warning colour and says Clogged", () => {
    const filter = objectOf("filter", ["differential-pressure", "clogged"], { unit: "bar", decimals: 2 });
    const clean = drawObject(filter, valuesOf({ "differential-pressure": 0.25, clogged: false }));
    assert.ok(part(clean, "housing"), renderToString(clean));
    assert.notStrictEqual(part(clean, "medium")?.attrs.stroke, "var(--hmi-warning)");
    assert.strictEqual(stateWord(clean), "0.25 bar");
    const clogged = drawObject(filter, valuesOf({ "differential-pressure": 1.4, clogged: true }));
    assert.strictEqual(part(clogged, "medium")?.attrs.stroke, "var(--hmi-warning)", renderToString(clogged));
    assert.strictEqual(stateWord(clogged), "Clogged");
  });

  test("a cyclone takes its feed at the side, sends gas up and solids down, and gives its differential pressure", () => {
    const drawn = drawObject(
      objectOf("cyclone", ["differential-pressure", "fault"], { unit: "mbar", decimals: 0 }),
      valuesOf({ "differential-pressure": 12, fault: false }),
    );
    assert.ok(part(drawn, "cone"), renderToString(drawn));
    assert.strictEqual(stateWord(drawn), "12 mbar");
    const ports = symbol("cyclone").ports;
    assert.deepStrictEqual(
      [ports.inlet?.direction, ports["gas-outlet"]?.direction, ports["solids-outlet"]?.direction],
      ["left", "up", "down"],
    );
  });

  test("a crusher's rotor and a screen's exciter turn while they run; a running crusher gives its load", () => {
    for (const type of ["crusher", "screen"]) {
      const object = objectOf(type, ["running"]);
      const running = drawObject(object, valuesOf({ running: true }));
      assert.strictEqual(part(running, "rotor")?.attrs.class, "hmi-spin", `${type}: ${renderToString(running)}`);
      assert.strictEqual(stateWord(running), "Running", type);
      const stopped = drawObject(object, valuesOf({ running: false }));
      assert.ok(part(stopped, "rotor"), type);
      assert.strictEqual(part(stopped, "rotor")?.attrs.class, undefined, type);
      assert.strictEqual(stateWord(stopped), "Stopped", type);
    }
    assert.strictEqual(
      stateWord(drawObject(objectOf("crusher", ["running", "load"]), valuesOf({ running: true, load: 72.4 }))),
      "72 %",
    );
    assert.deepStrictEqual(Object.keys(symbol("screen").ports).sort(), ["inlet", "oversize", "undersize"]);
  });

  test("a VFD gives its output frequency while it runs, its output drawn in the moving colour", () => {
    const vfd = objectOf("vfd", ["running", "frequency"], { unit: "Hz", decimals: 1 });
    const running = drawObject(vfd, valuesOf({ running: true, frequency: 42.5 }));
    assert.strictEqual(stateWord(running), "42.5 Hz");
    assert.strictEqual(part(running, "output")?.attrs.stroke, MOVING, renderToString(running));
    const stopped = drawObject(vfd, valuesOf({ running: false, frequency: 0 }));
    assert.strictEqual(stateWord(stopped), "Stopped");
    assert.strictEqual(part(stopped, "output")?.attrs.stroke, STILL);
    assert.deepStrictEqual(symbol("vfd").ports, {});
  });

  test("a drive says Fault, Running with its speed, Ready, Not ready or Stopped, in that order", () => {
    const drive = objectOf("drive", ["ready", "running", "fault", "speed"], {
      unit: "rpm",
      decimals: 0,
      size: CATALOGUE_SIZE,
    });
    const word = (values: Record<string, boolean | number>): string => stateWord(drawObject(drive, valuesOf(values)));
    assert.strictEqual(word({ ready: true, running: true, fault: true, speed: 900 }), "Fault");
    assert.strictEqual(word({ ready: true, running: true, fault: false, speed: 900 }), "900 rpm");
    assert.strictEqual(word({ ready: true, running: false, fault: false, speed: 0 }), "Ready");
    assert.strictEqual(word({ ready: false, running: false, fault: false, speed: 0 }), "Not ready");
    assert.strictEqual(stateWord(drawObject(objectOf("drive", ["running"]), valuesOf({ running: false }))), "Stopped");
  });

  test("the plain device, a customer block's first picture, lights its lamp while it runs (§21.9)", () => {
    const device = objectOf("device", ["running", "fault"], { size: CATALOGUE_SIZE });
    const running = drawObject(device, valuesOf({ running: true, fault: false }));
    const stopped = drawObject(device, valuesOf({ running: false, fault: false }));
    assert.notStrictEqual(
      part(running, "lamp")?.attrs.fill,
      part(stopped, "lamp")?.attrs.fill,
      "the lamp shows it runs",
    );
    assert.strictEqual(stateWord(drawObject(device, valuesOf({ running: false, fault: true }))), "Fault");
  });

  test("a contactor's contact closes when closed, in the moving colour; a tripped breaker stands open and says Tripped as a fault", () => {
    const contactor = objectOf("contactor", ["closed"]);
    const closed = drawObject(contactor, valuesOf({ closed: true }));
    assert.strictEqual(part(closed, "contact")?.attrs["data-closed"], "true", renderToString(closed));
    assert.strictEqual(part(closed, "contact")?.attrs.stroke, MOVING);
    assert.ok(part(closed, "arc"), "the contactor's arc");
    assert.strictEqual(stateWord(closed), "Closed");
    const open = drawObject(contactor, valuesOf({ closed: false }));
    assert.strictEqual(part(open, "contact")?.attrs["data-closed"], "false");
    assert.strictEqual(stateWord(open), "Open");
    const breaker = objectOf("breaker", ["closed", "tripped"], { size: CATALOGUE_SIZE });
    const on = drawObject(breaker, valuesOf({ closed: true, tripped: false }));
    assert.ok(part(on, "cross"), "the breaker's cross");
    assert.strictEqual(stateWord(on), "Closed");
    const tripped = drawObject(breaker, valuesOf({ closed: false, tripped: true }));
    assert.strictEqual(part(tripped, "contact")?.attrs["data-closed"], "false");
    assert.strictEqual(stateWord(tripped), "Tripped");
    assert.strictEqual(part(tripped, "state")?.attrs.fill, "var(--hmi-alarm)");
  });
});
