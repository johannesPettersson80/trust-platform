import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import {
  beginPlantDraft,
  bindFirstAddressedSignals,
  openRenderedPlantSession,
  pollSave,
  pollValue,
} from "./plant-rendered-harness";

const RENDERED_TEST_ENABLED = process.env.TRUST_PLANT_RENDERED_TEST === "1";

if (RENDERED_TEST_ENABLED) {
  suite("Rendered plant editor", function () {
    this.timeout(120_000);

    test("CPUI-019 scenario events use named state and sensor quantities", async function () {
      const session = await openRenderedPlantSession();
      try {
        const { inner } = session;
        await beginPlantDraft(session);
        await session.setValue("select[name=plant-model]", "robot_joint");
        await bindFirstAddressedSignals(session);
        await session.setValue("select[name=plant-feedback-target-1]", "%IL8");
        await inner("d.querySelector('[data-plant-optional-open=scenarios]')?.click();return 'OPEN';");
        await inner("d.querySelector('[data-plant-scenario-event-add]')?.click();return 'ADDED';");

        const stateChannel = '[data-plant-scenario-event-row="0"] select[data-plant-scenario-event-field="channel"]';
        assert.deepStrictEqual(
          JSON.parse(await inner("var select=d.querySelector('[data-plant-scenario-event-row=\\\"0\\\"] select[data-plant-scenario-event-field=\\\"channel\\\"]');return JSON.stringify({tag:select?.tagName,options:[...(select?.options||[])].map((option)=>({value:option.value,text:option.textContent?.trim()}))});")),
          {
            tag: "SELECT",
            options: [
              { value: "0", text: "Angle (rad)" },
              { value: "1", text: "Angular speed (rad/s)" },
            ],
          },
          "state events must expose named robot-joint state quantities"
        );
        await session.setValue(stateChannel, "1");

        await inner("var action=d.querySelector('[data-plant-scenario-event-row=\\\"0\\\"] select[data-plant-scenario-event-field=\\\"kind\\\"]');if(!action)return 'NO_ACTION';action.value='set_sensor_bias';action.dispatchEvent(new Event('change',{bubbles:true}));return 'CHANGED';");
        await pollValue(
          "sensor event channel selector",
          async () => inner("var select=d.querySelector('[data-plant-scenario-event-row=\\\"0\\\"] select[data-plant-scenario-event-field=\\\"channel\\\"]');return JSON.stringify({options:[...(select?.options||[])].map((option)=>({value:option.value,text:option.textContent?.trim()}))});"),
          (value) => value === JSON.stringify({ options: [{ value: "0", text: "Joint angle (rad)" }, { value: "1", text: "Angular speed (rad/s)" }] }),
          8_000
        );
        await session.setValue('[data-plant-scenario-event-row="0"] select[data-plant-scenario-event-field="channel"]', "1");
        await pollSave(inner, false, "named sensor channel event");

        const clicked = await inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');var save=panel&&[...panel.querySelectorAll('button')].find((candidate)=>(candidate.textContent||'').trim()==='Save');if(!save||save.disabled)return 'NOT_READY';save.click();return 'SAVED';");
        assert.strictEqual(clicked, "SAVED");
        await pollValue("scenario editor closes after save", async () => inner("return d.querySelector('aside[aria-label=\"Plant properties\"]')?'OPEN':'CLOSED';"), (value) => value === "CLOSED", 20_000);
        const simulationToml = fs.readFileSync(path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "simulation.toml"), "utf8");
        const savedEvent = simulationToml.split(/\[\[plant_events\]\]/).find((table) => /kind\s*=\s*"set_sensor_bias"/.test(table));
        assert.ok(savedEvent, "the sensor event must be present in runtime configuration");
        assert.match(savedEvent, /channel\s*=\s*1\b/, "named sensor selection must persist numeric channel 1 independent of TOML key order");
      } finally {
        session.close();
      }
    });
  });
}
