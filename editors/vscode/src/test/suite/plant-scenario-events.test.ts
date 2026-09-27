import * as assert from "assert";

suite("Plant scenario event editor", () => {
  test("converts decimal seconds without rounding sub-millisecond edits", () => {
    const api = require("../../networkCanvas/plant/scenarioEventEditor");
    assert.strictEqual(typeof api.scenarioMillisecondsFromSeconds, "function");
    for (const [raw, expected] of [["0", 0], ["1.001", 1001], ["1.0010", 1001], ["0.029", 29], ["2.5", 2500]]) {
      assert.strictEqual(api.scenarioMillisecondsFromSeconds(raw), expected, String(raw));
    }
    for (const raw of ["", " ", "0.0001", "-1", "Infinity", "NaN", "9007199254740992", "1e-4", "abc"]) {
      assert.ok(Number.isNaN(api.scenarioMillisecondsFromSeconds(raw)), raw);
    }
  });
  test("keeps scenario time inside the runtime duration boundary", () => {
    const api = require("../../networkCanvas/plant/scenarioEventEditor");
    const limit = 9_223_372_036_854;
    assert.strictEqual(api.scenarioMillisecondsFromSeconds(String(limit / 1000)), limit);
    assert.ok(Number.isNaN(api.scenarioMillisecondsFromSeconds("9223372036.855")), "one millisecond beyond the runtime duration must be invalid");
    const row = { enabled: true, atMs: 1001, timeSecondsText: "1.0010", kind: "reset_plant" };
    assert.deepStrictEqual(api.persistedPlantScenarioEvents([row]), [{ at_ms: 1001, kind: "reset_plant" }]);
  });
  test("owns conditional fields, validation, equal-time order, and exact serialization", () => {
    let api: any;
    try {
      api = require("../../networkCanvas/plant/scenarioEventEditor");
    } catch {
      api = undefined;
    }

    const first = api?.createPlantScenarioEvent?.(1);
    assert.deepStrictEqual(first, {
      enabled: true,
      atMs: 0,
      kind: "set_state",
      channel: 0,
      value: 0,
    });
    const parameter = api?.applyPlantScenarioEventKind?.(first, "set_parameter", ["gain"]);
    assert.deepStrictEqual(parameter, {
      enabled: true,
      atMs: 0,
      kind: "set_parameter",
      parameter: "gain",
      value: 0,
    });
    const reset = api?.applyPlantScenarioEventKind?.(parameter, "reset_plant", ["gain"]);
    assert.deepStrictEqual(reset, { enabled: true, atMs: 0, kind: "reset_plant" });

    const rows = [
      { enabled: true, atMs: 500, kind: "add_state", channel: 0, value: 1.5 },
      { enabled: true, atMs: 500, kind: "set_parameter", parameter: "gain", value: 0.8 },
      { enabled: false, atMs: 900, kind: "reset_plant" },
    ];
    assert.deepStrictEqual(api?.movePlantScenarioEvent?.(rows, 1, -1), [rows[1], rows[0], rows[2]]);
    assert.deepStrictEqual(api?.movePlantScenarioEvent?.(rows, 0, 1), [rows[1], rows[0], rows[2]]);
    assert.deepStrictEqual(api?.movePlantScenarioEvent?.(rows, 1, 1), rows, "different due times cannot be reordered");
    assert.deepStrictEqual(api?.persistedPlantScenarioEvents?.(rows), [
      { at_ms: 500, kind: "add_state", channel: 0, value: 1.5 },
      { at_ms: 500, kind: "set_parameter", parameter: "gain", value: 0.8 },
    ]);

    assert.deepStrictEqual(
      api?.validatePlantScenarioEvents?.(
        [
          { enabled: true, atMs: -1, kind: "set_state", channel: 2, value: 1 },
          { enabled: true, atMs: 10, kind: "set_parameter", parameter: "deleted", value: 1 },
          { enabled: true, atMs: 20, kind: "stick_actuator" },
        ],
        { enabled: true, stateCount: 2, outputCount: 1, scalarActuator: false, parameters: ["gain"] }
      ),
      [
        { row: 0, field: "at_ms", message: "Time must be a nonnegative whole number of milliseconds." },
        { row: 0, field: "channel", message: "State channel must be between 0 and 1." },
        { row: 1, field: "parameter", message: "Choose a parameter owned by the selected plant model." },
        { row: 2, field: "kind", message: "Actuator fault events require a plant with one command signal." },
      ]
    );

    assert.deepStrictEqual(
      api?.scenarioEventsForPlant?.(
        {
          plant_events: [
            { plant: "oven", at_ms: 1, kind: "reset_plant" },
            { plant: "tank", at_ms: 2, kind: "set_sensor_bias", channel: 0, value: 3 },
          ],
        },
        "tank"
      ),
      [{ enabled: true, atMs: 2, kind: "set_sensor_bias", channel: 0, value: 3 }]
    );
  });
});
