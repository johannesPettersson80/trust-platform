import * as assert from "assert";
import * as vscode from "vscode";

import { createPlantSessionRegistry } from "../../networkCanvas/plant/service";
import {
  captureRenderedWorkbench,
  openRenderedPlantSession,
  pollSave,
  pollValue,
  requiredEnvironment,
  sleep,
  type RenderedPlantSession,
} from "./plant-rendered-harness";

if (process.env.TRUST_PLANT_RENDERED_TEST === "1") {
  suite("Rendered plant editor", function () {
    this.timeout(120_000);

    test("CPFIX-003 rendered full candidate preservation", async function () {
      const session = await openRenderedPlantSession();
      try {
        const workspace = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        assert.ok(workspace, "rendered full-candidate fixture workspace must be open");
        const before = await readSavedConfiguration(workspace);
        await openPlant(session, "Full candidate tank");
        await assertProjectedCandidate(session, "Full candidate tank");

        await session.inner("var details=d.querySelector('[data-plant-technical-details]');if(details&&!details.open)details.querySelector('summary').click();return 'OPEN';");
        assert.strictEqual(
          await session.setValue('input[name="plant-display-name"]', "Renamed full candidate tank"),
          "Renamed full candidate tank",
        );
        await pollSave(session.inner, false, "full candidate label edit");
        await saveDraft(session);

        const after = await readSavedConfiguration(workspace);
        const expected = structuredClone(before);
        const expectedTank = expected.plants.find((plant) => plant.id === "tank");
        assert.ok(expectedTank, "fixture plant.read must include tank candidate");
        expectedTank.name = "Renamed full candidate tank";
        assert.deepStrictEqual(
          after,
          expected,
          "label-only Save must preserve all plant fields, events, couplings, simulation settings, and unrelated plants",
        );

        await openPlant(session, "Renamed full candidate tank");
        await assertProjectedCandidate(session, "Renamed full candidate tank");
        if (process.env.TRUST_PLANT_RENDERED_SCREENSHOT) {
          await vscode.commands.executeCommand("notifications.clearAll");
          await session.inner("d.querySelector('[data-plant-optional-settings]')?.scrollIntoView({block:'center'});return 'SCROLLED';");
          await sleep(400);
          captureRenderedWorkbench(process.env.TRUST_PLANT_RENDERED_SCREENSHOT);
        }
      } finally {
        session.close();
      }
    });
  });
}

type SavedConfiguration = {
  simulation: Record<string, unknown>;
  couplings: Record<string, unknown>[];
  plants: Record<string, unknown>[];
  plant_events: Record<string, unknown>[];
};

async function readSavedConfiguration(workspace: string): Promise<SavedConfiguration> {
  const registry = createPlantSessionRegistry(requiredEnvironment("ST_RUNTIME_TEST_BIN"));
  try {
    const response = await registry.forProject(workspace).request("plant.read", {
      contract: "trust.plant",
      schema_version: 1,
    });
    assert.ok(response.saved_configuration, "plant.read must return the saved configuration");
    return structuredClone(response.saved_configuration) as SavedConfiguration;
  } finally {
    registry.dispose();
  }
}

async function openPlant(session: RenderedPlantSession, label: string): Promise<void> {
  await pollValue(
    `saved plant ${label}`,
    async () => session.inner(`var node=[...d.querySelectorAll('.react-flow__node')].find((candidate)=>(candidate.textContent||'').includes(${JSON.stringify(label)}));if(!node)return 'WAIT';node.dispatchEvent(new node.ownerDocument.defaultView.MouseEvent('click',{bubbles:true}));return 'OPEN';`),
    (value) => value === "OPEN",
    20_000,
  );
  await pollValue(
    `editor for ${label}`,
    () => session.inner("return String(Boolean(d.querySelector('select[name=plant-model]')));"),
    (value) => value === "true",
    10_000,
  );
}

async function assertProjectedCandidate(session: RenderedPlantSession, name: string): Promise<void> {
  const { inner } = session;
  await inner("var details=d.querySelector('[data-plant-technical-details]');if(details&&!details.open)details.querySelector('summary').click();var model=d.querySelector('[data-plant-model-details]');if(model&&model.getAttribute('aria-expanded')!=='true')model.click();return 'OPEN';");
  await pollValue(
    "full candidate projected values",
    () => inner("var value=(s)=>d.querySelector(s)?.value??'';return JSON.stringify({name:value('input[name=\"plant-display-name\"]'),model:value('select[name=plant-model]'),command:value('select[name=plant-command-source]'),feedback:value('select[name=plant-feedback-target]'),state:value('[data-plant-dynamics-field=starting_value]'),gain:value('[data-plant-dynamics-field=gain]'),time:value('[data-plant-dynamics-field=time_constant_s]'),delay:value('[data-plant-dynamics-field=dead_time_s]'),history:value('[data-plant-dynamics-field=initial_delayed_input]')});"),
    (value) => {
      const projected = JSON.parse(value);
      return projected.name === name && projected.model === "first_order_dead_time" &&
        projected.command === "%QL0" && projected.feedback === "%IL0" &&
        projected.state === "1.5" && projected.gain === "1.2" && projected.time === "5" &&
        projected.delay === "0.5" && projected.history === "2";
    },
    10_000,
  );
  const summaries = JSON.parse(await inner("var text=(id)=>(d.querySelector('[data-plant-optional-summary=\"'+id+'\"]')?.textContent||'').trim();return JSON.stringify({actuator:text('actuator'),signals:text('signals'),realism:text('realism'),scenarios:text('scenarios')});"));
  assert.match(summaries.actuator, /Range 0–10.*7 configured settings/);
  assert.match(summaries.signals, /4 scaling or offset values changed.*1 nonzero initial command.*20 ms plant step/);
  assert.match(summaries.realism, /1 configured effect.*1 enabled effect.*Process 1/);
  assert.match(summaries.scenarios, /1 configured event.*1 included event.*first at 0\.1 s/);

  await inner("d.querySelector('[data-plant-optional-open=actuator]')?.click();return 'OPEN';");
  const actuator = JSON.parse(await inner("var value=(key)=>d.querySelector('[data-plant-actuator-field=\"'+key+'\"]')?.value??'';return JSON.stringify(Object.fromEntries(['minimum','maximum','rate_up_per_s','rate_down_per_s','deadband','lag_s','initial_applied'].map((key)=>[key,value(key)])));"));
  assert.deepStrictEqual(actuator, {
    minimum: "0", maximum: "10", rate_up_per_s: "3", rate_down_per_s: "4",
    deadband: "0.1", lag_s: "0.2", initial_applied: "2.5",
  });
  await inner("d.querySelector('[data-plant-optional-open=signals]')?.click();return 'OPEN';");
  const signals = JSON.parse(await inner("var value=(key)=>d.querySelector('[data-plant-engineering-field=\"'+key+'\"]')?.value??'';return JSON.stringify(Object.fromEntries(['step_ms','initial_input.0','input_scale.0','input_offset.0','output_scale.0','output_offset.0'].map((key)=>[key,value(key)])));"));
  assert.deepStrictEqual(signals, {
    step_ms: "20", "initial_input.0": "2.5", "input_scale.0": "0.5",
    "input_offset.0": "1", "output_scale.0": "2", "output_offset.0": "-3",
  });
}

async function saveDraft(session: RenderedPlantSession): Promise<void> {
  assert.strictEqual(await session.inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');var button=panel&&[...panel.querySelectorAll('button')].find((candidate)=>(candidate.textContent||'').trim()==='Save');if(!button||button.disabled)return 'NOT_READY';button.click();return 'SAVED';"), "SAVED");
  try {
    await pollValue("saved full candidate closes", () => session.inner("return String(Boolean(d.querySelector('aside[aria-label=\"Plant properties\"]')));"), (value) => value === "false", 20_000);
  } catch (error) {
    const diagnostic = await session.inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');return JSON.stringify({summary:(panel?.querySelector('[data-plant-validation-summary]')?.textContent||'').trim(),conflict:(panel?.querySelector('[data-plant-revision-conflict]')?.textContent||'').trim(),footer:(panel?.querySelector('footer')?.textContent||'').trim()});");
    if (process.env.TRUST_PLANT_RENDERED_SCREENSHOT) captureRenderedWorkbench(process.env.TRUST_PLANT_RENDERED_SCREENSHOT);
    throw new Error(`${error instanceof Error ? error.message : String(error)}; UI diagnostic: ${diagnostic}`);
  }
}
