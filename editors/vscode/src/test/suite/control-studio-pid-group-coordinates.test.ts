import * as assert from "assert";
import { initializationFormulas } from "../../controlStudio/controllerInitializationFormulas";
import { initializationValues } from "../../controlStudio/controllerInitialState";
import { projectPidGroupLoop } from "./control-studio-pid-group-fixtures";

suite("Control Studio PID group operating coordinates", () => {
  test("initializes each member from its explicit model identity after model channel reordering", () => {
    const loop = projectPidGroupLoop();
    const model: any = {
      kind: "continuous-state-space", inputs: [{ id: "u2", unit: "%" }, { id: "u1", unit: "%" }],
      outputs: [{ id: "y2", unit: "°C" }, { id: "y1", unit: "°C" }],
      operatingPoint: { inputs: [30, 20], outputs: [40, 10] },
    };
    const values = initializationValues(loop, model, loop.controller.project.initialization);
    assert.strictEqual(values["P1.Controller.StateValue.NominalOutput"], 20);
    assert.strictEqual(values["P1.Secondary.StateValue.NominalOutput"], 30);
    assert.strictEqual(values["P1.Controller.StateValue.ProcessValue"], 10);
    assert.strictEqual(values["P1.Secondary.StateValue.ProcessValue"], 40);
    const formulas = initializationFormulas(loop, model, loop.controller.project.initialization)!;
    assert.deepStrictEqual(formulas["P1.Controller.StateValue.NominalOutput"], { kind: "model-input", output_symbol: "P1.ManipulatedValue" });
    assert.deepStrictEqual(formulas["P1.Secondary.StateValue.ProcessValue"], { kind: "model-output", process_variable: "P1.SecondaryProcessValue" });
  });
});
