import * as assert from "assert";

suite("Plant robot-joint defaults", () => {
  test("optional gravity and friction parameters may be omitted", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      validatePlantDynamics: (input: { model: string; parameters: Record<string, number>; initialState: number[] }) => Array<unknown>;
    };
    assert.deepStrictEqual(api.validatePlantDynamics({
      model: "robot_joint",
      parameters: { inertia_kg_m2: 2, friction_smoothing_rad_s: 0.01 },
      initialState: [0, 0],
    }), []);
  });
});
