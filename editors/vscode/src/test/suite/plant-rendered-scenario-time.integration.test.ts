import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import {
  beginPlantDraft,
  bindFirstAddressedSignals,
  captureRenderedWorkbench,
  openRenderedPlantSession,
  pollSave,
  pollValue,
  sleep,
  type RenderedPlantSession,
} from "./plant-rendered-harness";

const RENDERED_TEST_ENABLED = process.env.TRUST_PLANT_RENDERED_TEST === "1";

if (RENDERED_TEST_ENABLED) {
  suite("Rendered plant editor", function () {
    this.timeout(120_000);

    test("CPUI-019 scenario seconds preserve exact backend milliseconds", async function () {
      if (process.env.TRUST_PLANT_RENDERED_THEME) {
        await vscode.workspace.getConfiguration("workbench").update("colorTheme", process.env.TRUST_PLANT_RENDERED_THEME, vscode.ConfigurationTarget.Global);
      }
      const session = await openRenderedPlantSession();
      try {
        const { inner } = session;
        await beginPlantDraft(session);
        await bindFirstAddressedSignals(session);
        await inner("d.querySelector('[data-plant-optional-open=scenarios]')?.click();return 'OPEN';");
        await inner("d.querySelector('[data-plant-scenario-event-add]')?.click();return 'ADDED';");

        const timeField = '[data-plant-scenario-event-field="at_s"]';
        // The event row appears once the language server has added it to the draft.
        await pollValue(
          "scenario time must be a named seconds quantity in the rendered editor",
          async () => inner("var field=d.querySelector('[data-plant-scenario-event-field=at_s]');return JSON.stringify({exists:Boolean(field),label:(field?.closest('label')?.textContent||'').replace(/\\s+/g,' ').trim()});"),
          (value) => value === JSON.stringify({ exists: true, label: "Time (s)" }),
          8_000
        );

        await session.setValue(timeField, "1.001");
        await pollValue(
          "exact millisecond scenario time",
          async () => inner("var field=d.querySelector('[data-plant-scenario-event-field=at_s]');return JSON.stringify({value:field?.value,invalid:field?.getAttribute('aria-invalid')});"),
          (value) => value === JSON.stringify({ value: "1.001", invalid: "false" }),
          8_000
        );
        await pollSave(inner, false, "exact millisecond scenario time");
        await saveAndReopen(session);
        const simulationToml = fs.readFileSync(path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "simulation.toml"), "utf8");
        assert.match(simulationToml, /at_ms\s*=\s*1001/, "1.001 s must be persisted as exactly 1001 backend milliseconds");
        await pollValue(
          "saved 1.001 s scenario time must reopen without losing its 1001 ms representation",
          async () => inner("return d.querySelector('[data-plant-scenario-event-field=at_s]')?.value||'';"),
          (value) => value === "1.001",
          8_000
        );

        await session.setValue(timeField, "");
        await pollValue(
          "blank scenario time",
          async () => inner("var field=d.querySelector('[data-plant-scenario-event-field=at_s]');return JSON.stringify({value:field?.value,invalid:field?.getAttribute('aria-invalid')});"),
          (value) => value === JSON.stringify({ value: "", invalid: "true" }),
          8_000
        );
        await pollSave(inner, true, "blank scenario time");

        await session.setValue(timeField, "0.0001");
        await pollValue(
          "sub-millisecond scenario time",
          async () => inner("var field=d.querySelector('[data-plant-scenario-event-field=at_s]');return JSON.stringify({value:field?.value,invalid:field?.getAttribute('aria-invalid')});"),
          (value) => value === JSON.stringify({ value: "0.0001", invalid: "true" }),
          8_000
        );
        await inner("d.querySelector('[data-plant-optional-open=scenarios]')?.click();return 'CLOSED';");
        await pollValue(
          "closed scenario editor after sub-millisecond input",
          async () => inner("return String(d.querySelector('[data-plant-optional-editor=scenarios]'));"),
          (value) => value === "null",
          8_000
        );
        await inner("d.querySelector('[data-plant-optional-open=scenarios]')?.click();return 'OPEN';");
        await pollValue(
          "reopened sub-millisecond scenario time",
          async () => inner("var field=d.querySelector('[data-plant-scenario-event-field=at_s]');return JSON.stringify({value:field?.value,invalid:field?.getAttribute('aria-invalid')});"),
          (value) => value === JSON.stringify({ value: "0.0001", invalid: "true" }),
          8_000
        );
        await pollSave(inner, true, "sub-millisecond scenario time");
        assert.match(await inner("return d.querySelector('[data-plant-optional-summary=scenarios]')?.textContent||'';"),
          /Correct event time/, "invalid scenario time must have an actionable closed summary, not NaN seconds");
        if (process.env.TRUST_PLANT_RENDERED_SCREENSHOT) {
          await vscode.commands.executeCommand("notifications.clearAll");
          await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
          await inner("d.querySelector('[data-plant-scenario-event-field=at_s]')?.scrollIntoView({block:'center'});return 'VISIBLE';");
          await sleep(500);
          captureRenderedWorkbench(process.env.TRUST_PLANT_RENDERED_SCREENSHOT);
        }

        await session.setValue(timeField, "1.001");
        await pollSave(inner, false, "corrected scenario time");
        await saveAndReopen(session);
        await pollValue(
          "corrected scenario time must reopen as the exact seconds rendering of 1001 ms",
          async () => inner("return d.querySelector('[data-plant-scenario-event-field=at_s]')?.value||'';"),
          (value) => value === "1.001",
          8_000
        );
      } finally {
        session.close();
      }
    });
  });
}

async function saveAndReopen(session: RenderedPlantSession): Promise<void> {
  const clicked = await session.inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');var save=panel&&[...panel.querySelectorAll('button')].find((candidate)=>(candidate.textContent||'').trim()==='Save');if(!save||save.disabled)return 'NOT_READY';save.click();return 'SAVED';");
  assert.strictEqual(clicked, "SAVED", "scenario draft must be ready for a real save");
  await pollValue("saved scenario draft to close", async () => session.inner("return d.querySelector('aside[aria-label=\"Plant properties\"]')?'OPEN':'CLOSED';"), (value) => value === "CLOSED", 20_000);
  await pollValue("persisted plant node to reopen", async () => session.inner("var node=[...d.querySelectorAll('.react-flow__node')].find((candidate)=>(candidate.textContent||'').includes('New simulated plant')&&!(candidate.textContent||'').includes('DRAFT'));if(!node)return 'WAIT';node.dispatchEvent(new node.ownerDocument.defaultView.MouseEvent('click',{bubbles:true}));return 'CLICKED';"), (value) => value === "CLICKED", 20_000);
  await pollValue("persisted plant editor", async () => session.inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');return (panel?.textContent||'').includes('Configure plant')?'OPEN':'WAIT';"), (value) => value === "OPEN", 20_000);
  // The reopened editor shows its fields once Rust has opened the saved plant.
  await pollValue("reopened plant fields", async () => session.inner("return String(Boolean(d.querySelector('[data-plant-optional-open=scenarios]')));"), (value) => value === "true", 20_000);
  await session.inner("d.querySelector('[data-plant-optional-open=scenarios]')?.click();return 'OPEN';");
  await pollValue("reopened scenario editor", async () => session.inner("return String(Boolean(d.querySelector('[data-plant-optional-editor=scenarios]')));"), (value) => value === "true", 8_000);
}
