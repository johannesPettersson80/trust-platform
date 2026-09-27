import * as assert from "assert";
import { plantModelDescriptor } from "../../networkCanvas/plant/modelDescriptors";
import { plantDynamicsFields, plantPortLabels, proposePlantStartingPoint, type PlantModelKind } from "../../networkCanvas/plant/editorModel";
import { plantStartingStateFields } from "../../networkCanvas/plant/startingConditions";
suite("Cascaded gravity tank editor contract", () => {
  test("preserves two physical level states and ports instead of a scalar fallback", () => {
    const descriptor = plantModelDescriptor("gravity_tank_cascade");
    assert.strictEqual(descriptor.states, 2);
    assert.strictEqual(descriptor.inputs, 1);
    assert.strictEqual(descriptor.outputs, 2);
    assert.deepStrictEqual(plantPortLabels({ model: "gravity_tank_cascade" as PlantModelKind, parameters: {} }), {
      commands: ["Pump flow (m³/s)"], feedbacks: ["Upstream level (m)", "Downstream level (m)"] });
    assert.deepStrictEqual(plantStartingStateFields("gravity_tank_cascade").map(field => field.label), ["Starting upstream level (m)", "Starting downstream level (m)"]);
    assert.deepStrictEqual(plantDynamicsFields("gravity_tank_cascade").map(field => field.key), ["area_m2", "outflow_coefficient_m2_sqrt_m_per_s", "initial_state"]);
    const starting = proposePlantStartingPoint("gravity_tank_cascade");
    assert.deepStrictEqual(starting?.initialState, [0.25, 0.25]);
    assert.strictEqual(starting?.parameters.area_m2, 0.5);
  });
});
