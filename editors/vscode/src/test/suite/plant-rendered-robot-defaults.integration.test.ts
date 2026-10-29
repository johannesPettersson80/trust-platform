import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { beginPlantDraft, captureRenderedWorkbench, openRenderedPlantSession, pollSave, pollValue, sleep } from "./plant-rendered-harness";

if (process.env.TRUST_PLANT_RENDERED_TEST === "1") {
  suite("Rendered plant editor", function () {
    this.timeout(120_000);

    test("CPFIX-006 rendered robot optional defaults preserve blank parameters", async function () {
      const session = await openRenderedPlantSession();
      try {
        const { inner, setValue } = session;
        await beginPlantDraft(session);
        await setValue("select[name=plant-model]", "robot_joint");
        await pollValue("robot model", () => inner("return d.querySelector('select[name=plant-model]')?.value||'';"), (value) => value === "robot_joint", 8_000);
        const command = await pollValue("robot command option", () => inner("var s=d.querySelector('select[name=plant-command-source]');return [...(s?.options||[])].find((option)=>option.value)?.value||'';"), Boolean, 20_000);
        const feedback = await pollValue("robot feedback option", () => inner("var s=d.querySelector('select[name=plant-feedback-target]');return [...(s?.options||[])].find((option)=>option.value)?.value||'';"), Boolean, 20_000);
        const feedback2Options = await pollValue("robot second feedback options", () => inner("var s=d.querySelector('select[name=plant-feedback-target-1]');return JSON.stringify([...(s?.options||[])].map((option)=>option.value));"), (value) => { try { return JSON.parse(value).some((candidate: string) => candidate && candidate !== feedback); } catch { return false; } }, 20_000);
        const feedback2 = JSON.parse(feedback2Options).find((candidate: string) => candidate && candidate !== feedback);
        await setValue('select[name="plant-command-source"]', command);
        await setValue('select[name="plant-feedback-target"]', feedback);
        await setValue('select[name="plant-feedback-target-1"]', feedback2);
        for (const [name, value] of [["plant-command-source", command], ["plant-feedback-target", feedback], ["plant-feedback-target-1", feedback2]]) {
          await pollValue(`${name} selected`, () => inner(`return d.querySelector('select[name=${name}]')?.value||'';`), (current) => current === value, 8_000);
        }
        await pollSave(inner, false, "bound robot joint");

        const optional = ["gravity_torque_nm", "viscous_friction_nm_s_per_rad", "coulomb_friction_nm"];
        // Typed text reaches the draft when Rust has read it (`set_dynamics_text`, specification 47
        // §20.1); until then Save waits. React commits the typed text in a microtask and Rust's answer
        // comes later in a message, so Save is disabled here. Catches a Save that stores the value the
        // field held before (the guard of 29 October 2026 saw 4 for a blanked gravity torque).
        const saveWhileRead = await inner("var el=d.querySelector('[data-plant-dynamics-field=\"gravity_torque_nm\"]');if(!el)return 'NO_FIELD';var w=el.ownerDocument.defaultView;Object.getOwnPropertyDescriptor(w.HTMLInputElement.prototype,'value').set.call(el,'');el.dispatchEvent(new w.Event('input',{bubbles:true}));await Promise.resolve();await Promise.resolve();var panel=[...d.querySelectorAll('aside')].find((x)=>/Plant properties/i.test(x.getAttribute('aria-label')||''));var save=panel&&[...panel.querySelectorAll('button')].find((x)=>(x.textContent||'').trim()==='Save');return String(Boolean(save&&save.disabled));");
        assert.strictEqual(saveWhileRead, "true", "Save waits until Rust has read the typed text");
        for (const field of optional) await setValue(`[data-plant-dynamics-field="${field}"]`, "");
        const state = JSON.parse(await inner("var keys=['gravity_torque_nm','viscous_friction_nm_s_per_rad','coulomb_friction_nm'];return JSON.stringify(keys.map((key)=>{var e=d.querySelector('[data-plant-dynamics-field='+key+']');return {key,value:e?.value??null,placeholder:e?.getAttribute('placeholder'),invalid:e?.getAttribute('aria-invalid')};}));"));
        assert.deepStrictEqual(state, [
          { key: "gravity_torque_nm", value: "", placeholder: "Default: 0", invalid: "false" },
          { key: "viscous_friction_nm_s_per_rad", value: "", placeholder: "Default: 0", invalid: "false" },
          { key: "coulomb_friction_nm", value: "", placeholder: "Default: 0", invalid: "false" },
        ]);
        await pollSave(inner, false, "blank robot optional parameters");
        if (process.env.TRUST_PLANT_RENDERED_SCREENSHOT) {
          await vscode.commands.executeCommand("notifications.clearAll");
          await inner("d.querySelector('[data-plant-dynamics-field=gravity_torque_nm]')?.scrollIntoView({block:'center'});return 'SCROLLED';");
          await sleep(500);
          captureRenderedWorkbench(process.env.TRUST_PLANT_RENDERED_SCREENSHOT);
        }
        const workspace = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        assert.ok(workspace);
        await inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');var save=panel&&[...panel.querySelectorAll('button')].find((candidate)=>(candidate.textContent||'').trim()==='Save');if(!save)return 'NO_SAVE';save.click();return 'SAVED';");
        await pollValue("robot editor closes after save", () => inner("return String(Boolean(d.querySelector('aside[aria-label=\"Plant properties\"]')));"), (value) => value === "false", 20_000);
        let simulation = fs.readFileSync(path.join(workspace, "simulation.toml"), "utf8");
        // In any form: a table line or an inline table (the saved plant's parameters can be either), so
        // that a Save of the value the field held before is caught here and not only after reopening.
        for (const field of optional) assert.doesNotMatch(simulation, new RegExp(`\\b${field}\\s*=`), `${field} omission must persist`);
        await pollValue("saved robot node", () => inner("var node=[...d.querySelectorAll('.react-flow__node')].find((candidate)=>(candidate.textContent||'').includes('New simulated plant'));if(!node)return 'WAIT';node.dispatchEvent(new node.ownerDocument.defaultView.MouseEvent('click',{bubbles:true}));return 'OPEN';"), (value) => value === "OPEN", 20_000);
        await pollValue("reopened robot editor", () => inner("return String(Boolean(d.querySelector('select[name=plant-model]')));"), (value) => value === "true", 20_000);
        for (const field of optional) {
          assert.strictEqual(await inner(`return d.querySelector('[data-plant-dynamics-field="${field}"]')?.value||'';`), "", `${field} remains blank after reopen`);
          assert.strictEqual(await inner(`return d.querySelector('[data-plant-dynamics-field="${field}"]')?.getAttribute('placeholder')||'';`), "Default: 0");
        }
      } finally {
        session.close();
      }
    });
  });
}
