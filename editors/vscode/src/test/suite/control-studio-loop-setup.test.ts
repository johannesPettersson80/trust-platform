import * as assert from "assert";
import { open } from "./control-studio-loop-setup-form-harness";

suite("Control Studio loop setup from compiled facts", () => {

  test("offers as the Auto/Manual signal the signals the engine names, and only those", () => {
    // The engine names the Auto/Manual signals it offers (SetupFormModel, setup/form.rs); the page lists them and adds none of its own.
    const view = open({ structure: "pid", modes: ["P1.Enabled"] });
    try {
      const offered = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="mode"] option')).map(option => option.value).filter(Boolean);
      assert.deepStrictEqual(offered, ["P1.Enabled"]);
    } finally { view.close(); }
  });

});
