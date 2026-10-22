import * as assert from "assert";
import * as vscode from "vscode";
import { withStudio } from "./control-studio-three-page.integration.test";

/**
 * The reference selector of the Setpoint step (specification 35, B3.2 and B3.6). A loop that declares no setpoint of its own may move one
 * of its further setpoint signals, so the Identify page shows the selector while that method is chosen, and only then, and lists the signals
 * the engine offers (`setpointChoices` of the open answer). Whether a signal can be the reference is the engine's rule, which a Rust test
 * proves (`only_a_declared_reference_is_used_and_the_controller_is_not_changed`, acquisition_settings_tests.rs); this journey shows the
 * control in the rendered panel. The checks of the rule moved to Rust with the TypeScript they tested; the control stayed in the page, so
 * its journey stays too.
 */

/**
 * The journey's loop: an open-loop project with no setpoint of its own and one further signal that is a setpoint in the measured output's
 * unit, which the Setpoint step may move without changing the controller.
 */
function referenceLoop(loop: any): void {
  loop.id += "-reference-selection";
  loop.parameterBinding.loop_id = loop.id;
  loop.topology = "open-loop";
  loop.controller.design = "project";
  delete loop.controller.law;
  delete loop.controller.pidInitialization;
  delete loop.signals.setpoint;
  loop.signals.additional = [{
    id: "additional-reference", channel: "primary", role: "SP", symbol: "Main.Reference", address: "%ML64", iecType: "LREAL", direction: "read",
    unit: loop.signals.processVariable.unit, range: { ...loop.signals.processVariable.range },
  }];
}

suite("Control Studio conditional setpoint identification", () => {
  test("shows the reference selector only when Setpoint step needs a configured signal", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async (inner, _reopen, capture) => {
      const selectorVisible = () => inner("return String(!!d.querySelector('[data-identify-setpoint]')?.checkVisibility());");
      const chooseMethod = (method: string) => inner(`const m=d.querySelector('[data-engineer-acquisition-method]');m.value='${method}';m.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return '${method}';`);
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify';");
      assert.strictEqual(await selectorVisible(), "false", "Automatic needs no reference, so the selector is not shown");
      await chooseMethod("setpoint_step");
      assert.strictEqual(await selectorVisible(), "true", "the Setpoint step needs the signal it moves");
      assert.match(await inner("return d.querySelector('[data-identify-setpoint]').textContent;"), /Main.Reference/, "the selector lists the engine's offer");
      await inner("const s=d.querySelector('[data-identify-setpoint]');s.value='additional-reference';s.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'reference';");
      await capture("reference");
      await chooseMethod("automatic");
      assert.strictEqual(await selectorVisible(), "false", "another method hides the selector again");
      assert.strictEqual(vscode.debug.activeDebugSession, undefined, "Selecting a reference starts no runtime operation");
    }, { loop: referenceLoop });
  });
});
