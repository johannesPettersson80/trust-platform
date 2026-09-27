import * as assert from "assert";

suite("Plant starting conditions", () => {
  test("publishes canonical labels and state meanings for supported dynamic models", () => {
    let api: {
      plantStartingStateFields?: (model: string) => ReadonlyArray<{ label: string; description: string }>;
    } | undefined;
    try {
      api = require("../../networkCanvas/plant/startingConditions") as typeof api;
    } catch {
      api = undefined;
    }
    const expected = {
      first_order: [
        { label: "Starting output", description: "Absolute ideal output, in the configured model's output units" },
      ],
      integrator: [
        { label: "Starting accumulated value", description: "Stored state before output bias; output equals this value plus output bias" },
      ],
      gravity_tank: [
        { label: "Starting level (m)", description: "Tank liquid level in metres" },
      ],
      second_order: [
        { label: "Starting displacement", description: "Output displacement before output bias" },
        { label: "Starting rate of change", description: "Time derivative in output units per second" },
      ],
      robot_joint: [
        { label: "Starting angle (rad)", description: "Joint angle when simulation starts or resets. Enter radians, not degrees; 2π radians is one complete turn." },
        { label: "Starting angular speed (rad/s)", description: "Joint speed when simulation starts or resets. Zero starts at rest; the sign gives the direction of motion." },
      ],
    };
    for (const [model, fields] of Object.entries(expected)) {
      assert.deepStrictEqual(api?.plantStartingStateFields?.(model), fields, model);
    }
    for (const model of ["first_order_dead_time", "state_space", "static_gain", "piecewise_linear"]) {
      assert.deepStrictEqual(api?.plantStartingStateFields?.(model), [], model);
    }
  });
});
