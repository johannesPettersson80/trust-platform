import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { captureRenderedWorkbench, openRenderedPlantSession, pollSave, pollValue, sleep, type RenderedPlantSession } from "./plant-rendered-harness";

if (process.env.TRUST_PLANT_RENDERED_TEST === "1") {
  suite("Rendered plant editor", function () {
    this.timeout(120_000);

    test("CPFIX-006 rendered omitted initial state remains editable and omission-preserving", async function () {
      const session = await openRenderedPlantSession();
      try {
        const { inner, setValue } = session;
        await pollValue(
          "saved omitted-state plant",
          async () => inner("var node=[...d.querySelectorAll('.react-flow__node')].find((candidate)=>(candidate.textContent||'').includes('Custom omitted state'));if(!node)return 'WAIT';node.dispatchEvent(new node.ownerDocument.defaultView.MouseEvent('click',{bubbles:true}));return 'OPEN';"),
          (value) => value === "OPEN",
          20_000
        );
        await sleep(500);
        assert.strictEqual(await inner("return String(Boolean(d.querySelector('select[name=plant-model]')));"),
          "true", "a runtime-valid saved plant with omitted initial_state must open its editor");
        assert.strictEqual(await inner("return d.querySelector('select[name=plant-model]')?.value||'';"), "first_order");
        assert.strictEqual(await inner("return d.querySelector('[data-plant-dynamics-field=time_constant_s]')?.value||'';"), "7");
        assert.strictEqual(await inner("return d.querySelector('[data-plant-starting-state=\"0\"]')?.value||'';"), "0");
        assert.strictEqual(await inner("return d.querySelector('[data-plant-dynamics-field=gain]')?.placeholder||'';"), "Default: 1",
          "an omitted gain must explain its effective value");
        assert.strictEqual(await inner("return d.querySelector('[data-plant-dynamics-field=input_bias]')?.placeholder||'';"), "Default: 0",
          "an omitted input bias is zero, not Unbounded");
        assert.strictEqual(await inner("return d.querySelector('[data-plant-dynamics-field=output_bias]')?.placeholder||'';"), "Default: 0");
        await setValue('[data-plant-dynamics-field=gain]', "2");
        await setValue('[data-plant-dynamics-field=gain]', "");
        await pollSave(inner, false, "clearing gain restores the valid runtime default");
        if (process.env.TRUST_PLANT_RENDERED_SCREENSHOT) {
          await vscode.commands.executeCommand("notifications.clearAll");
          await inner("d.querySelector('[data-plant-starting-state=\"0\"]')?.scrollIntoView({block:'center'});return 'SCROLLED';");
          await sleep(500);
          captureRenderedWorkbench(process.env.TRUST_PLANT_RENDERED_SCREENSHOT);
        }

        await setValue('input[name=plant-display-name]', "Renamed omitted state");
        await pollSave(inner, false, "label-only omitted-state edit");
        await saveDraft(session);
        const workspace = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        assert.ok(workspace, "rendered fixture workspace must be open");
        let simulation = fs.readFileSync(path.join(workspace, "simulation.toml"), "utf8");
        assert.match(simulation, /name\s*=\s*"Renamed omitted state"/);
        assert.doesNotMatch(simulation, /^initial_state\s*=/m, "label-only edit must preserve omitted initial_state");

        await pollValue(
          "renamed saved plant",
          async () => inner("var node=[...d.querySelectorAll('.react-flow__node')].find((candidate)=>(candidate.textContent||'').includes('Renamed omitted state'));if(!node)return 'WAIT';node.dispatchEvent(new node.ownerDocument.defaultView.MouseEvent('click',{bubbles:true}));return 'OPEN';"),
          (value) => value === "OPEN",
          20_000
        );
        // The editor shows its fields once Rust has opened the saved plant; an edit before then
        // finds no field.
        await pollValue(
          "reopened editor",
          async () => inner("return String(Boolean(d.querySelector('[data-plant-starting-state=\"0\"]')));"),
          (value) => value === "true",
          20_000
        );
        assert.strictEqual(await setValue('[data-plant-starting-state="0"]', "2.5"), "2.5");
        await pollSave(inner, false, "explicit nonzero initial-state edit");
        await saveDraft(session);
        simulation = fs.readFileSync(path.join(workspace, "simulation.toml"), "utf8");
        assert.match(simulation, /initial_state\s*=\s*\[2\.5\]/, "explicit starting-state edit must materialize initial_state");
      } finally {
        session.close();
      }
    });
  });
}

async function saveDraft(session: RenderedPlantSession): Promise<void> {
  assert.strictEqual(await session.inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');var button=panel&&[...panel.querySelectorAll('button')].find(x=>x.textContent.trim()==='Save');if(!button||button.disabled)return 'NOT_READY';button.click();return 'SAVED';"), "SAVED");
  await pollValue("saved draft closes", () => session.inner("return String(Boolean(d.querySelector('aside[aria-label=\"Plant properties\"]')));"), value => value === "false", 20_000);
}
