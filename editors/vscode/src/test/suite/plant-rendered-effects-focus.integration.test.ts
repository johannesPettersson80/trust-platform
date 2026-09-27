import * as assert from "assert";
import * as vscode from "vscode";

import {
  beginPlantDraft,
  bindFirstAddressedSignals,
  captureRenderedWorkbench,
  openRenderedPlantSession,
  pollSave,
  pollValue,
  sleep,
} from "./plant-rendered-harness";

const RENDERED_TEST_ENABLED = process.env.TRUST_PLANT_RENDERED_TEST === "1";

if (RENDERED_TEST_ENABLED) {
  suite("Rendered plant editor", function () {
    this.timeout(120_000);

    test("CPUI-017/018 focused effect cards preserve order and applicable controls", async function () {
      const session = await openRenderedPlantSession();
      try {
        const { inner } = session;
        await beginPlantDraft(session);
        await bindFirstAddressedSignals(session);
        await inner("d.querySelector('[data-plant-optional-open=realism]')?.click();return 'OPEN';");
        await inner("d.querySelector('[data-plant-uncertainty-add]')?.click();return 'ADD';");

        assert.strictEqual(
          await inner("return String(Boolean(d.querySelector('[data-plant-uncertainty-add-type]')));"),
          "true",
          "Add effect must offer the existing effect types before creating a row"
        );
        await inner("d.querySelector('[data-plant-uncertainty-add-type=gaussian_noise]')?.click();return 'GAUSSIAN';");
        await inner("d.querySelector('[data-plant-uncertainty-add]')?.click();return 'ADD';");
        await inner("d.querySelector('[data-plant-uncertainty-add-type=bias]')?.click();return 'BIAS';");

        await pollValue(
          "two focused effect cards",
          async () => inner("return JSON.stringify([...d.querySelectorAll('[data-plant-uncertainty-card]')].map((card)=>({kind:card.getAttribute('data-plant-uncertainty-kind'),boundary:card.getAttribute('data-plant-uncertainty-boundary')})));"),
          (value) => value === JSON.stringify([
            { kind: "gaussian_noise", boundary: "Sensor" },
            { kind: "bias", boundary: "Sensor" },
          ]),
          8_000
        );
        assert.strictEqual(await inner("var focused=[...d.querySelectorAll('[data-plant-uncertainty-focused]')];var visible=focused.filter((card)=>{var rect=card.getBoundingClientRect();return rect.width>0&&rect.height>0&&[...card.querySelectorAll('input,select,button')].some((field)=>{var box=field.getBoundingClientRect();return box.width>0&&box.height>0})});return JSON.stringify({focused:focused.length,visible:visible.length});"), JSON.stringify({ focused: 1, visible: 1 }), "exactly one effect editor must be visibly open at a time");
        await inner("d.querySelector('[data-plant-uncertainty-edit=\\\"0\\\"]')?.click();return 'EDIT_GAUSSIAN';");
        await pollValue("focused Gaussian effect", async () => inner("return String(d.querySelector('[data-plant-uncertainty-focused]')?.getAttribute('data-plant-uncertainty-focused')||'');"), (value) => value === "0", 8_000);
        assert.strictEqual(await inner("var card=d.querySelector('[data-plant-uncertainty-card=\\\"0\\\"]');return JSON.stringify({noise:Boolean(card?.querySelector('[data-plant-uncertainty-field=standard_deviation]')),value:Boolean(card?.querySelector('[data-plant-uncertainty-field=value]'))});"), JSON.stringify({ noise: true, value: false }), "Gaussian focus must show noise controls and hide bias-only controls");
        await inner("d.querySelector('[data-plant-uncertainty-edit=\\\"1\\\"]')?.click();return 'EDIT_BIAS';");
        await pollValue("focused Bias effect", async () => inner("return String(d.querySelector('[data-plant-uncertainty-focused]')?.getAttribute('data-plant-uncertainty-focused')||'');"), (value) => value === "1", 8_000);
        assert.strictEqual(await inner("var card=d.querySelector('[data-plant-uncertainty-card=\\\"1\\\"]');return JSON.stringify({noise:Boolean(card?.querySelector('[data-plant-uncertainty-field=standard_deviation]')),value:Boolean(card?.querySelector('[data-plant-uncertainty-field=value]'))});"), JSON.stringify({ noise: false, value: true }), "Bias focus must show bias controls and hide noise-only controls");
        await inner("var include=d.querySelector('[data-plant-uncertainty-row=\\\"0\\\"] input[type=checkbox]');include?.click();return 'DISABLED';");
        await inner("d.querySelector('[data-plant-uncertainty-row=\\\"0\\\"] [data-plant-uncertainty-down]')?.click();return 'MOVED';");
        await pollValue("moved effect order", async () => inner("return JSON.stringify([...d.querySelectorAll('[data-plant-uncertainty-card]')].map((card)=>card.getAttribute('data-plant-uncertainty-kind')));"), (value) => value === JSON.stringify(["bias", "gaussian_noise"]), 8_000);
        await inner("d.querySelector('[data-plant-uncertainty-row=\\\"1\\\"] [data-plant-uncertainty-up]')?.click();return 'RESTORED';");
        await inner("var include=d.querySelector('[data-plant-uncertainty-row=\\\"0\\\"] input[type=checkbox]');include?.click();return 'ENABLED';");
        await inner("d.querySelector('[data-plant-uncertainty-edit=\\\"0\\\"]')?.click();return 'EDIT_GAUSSIAN_AGAIN';");

        await session.setValue('[data-plant-uncertainty-row="0"] [data-plant-uncertainty-field="standard_deviation"]', "-1");
        await pollValue(
          "invalid Gaussian standard deviation",
          async () => inner("var field=d.querySelector('[data-plant-uncertainty-row=\\\"0\\\"] [data-plant-uncertainty-field=\\\"standard_deviation\\\"]');return JSON.stringify({value:field?.value,invalid:field?.getAttribute('aria-invalid')});"),
          (value) => value === JSON.stringify({ value: "-1", invalid: "true" }),
          8_000
        );
        await pollSave(inner, true, "invalid Gaussian standard deviation");
        await inner("d.querySelector('[data-plant-optional-open=realism]')?.click();return 'CLOSED';");
        await pollValue("closed realism editor", async () => inner("return String(d.querySelector('[data-plant-optional-editor=realism]'));"), (value) => value === "null", 8_000);
        const beforeFocus = JSON.parse(await inner("var summary=d.querySelector('[data-plant-validation-summary]');var button=d.querySelector('[data-plant-focus-first-error]');return JSON.stringify({summary:(summary?.textContent||'').replace(/\\s+/g,' ').trim(),button:Boolean(button),buttonText:(button?.textContent||'').replace(/\\s+/g,' ').trim(),editors:[...d.querySelectorAll('[data-plant-optional-editor]')].map((editor)=>editor.getAttribute('data-plant-optional-editor'))});"));
        assert.strictEqual(beforeFocus.button, true, "validation summary must expose a first-error action");
        assert.match(beforeFocus.buttonText, /standard deviation/i, "first-error action must identify standard deviation");
        assert.deepStrictEqual(beforeFocus.editors, [], "realism must be closed before first-error navigation");
        const clicked = await inner("var button=d.querySelector('[data-plant-focus-first-error]');if(!button)return 'NO_BUTTON';button.click();return 'CLICKED';");
        assert.strictEqual(clicked, "CLICKED", "first-error navigation must click an existing action");
        await pollValue(
          "validation opens and focuses first effect error",
          async () => inner("var summary=d.querySelector('[data-plant-validation-summary]');var active=d.activeElement;return JSON.stringify({summary:(summary?.textContent||'').replace(/\\s+/g,' ').trim(),editors:[...d.querySelectorAll('[data-plant-optional-editor]')].map((editor)=>editor.getAttribute('data-plant-optional-editor')),field:active?.getAttribute('data-plant-uncertainty-field'),row:active?.closest('[data-plant-uncertainty-row]')?.getAttribute('data-plant-uncertainty-row')});"),
          (value) => {
            try {
              const state = JSON.parse(value);
              return state.editors?.length === 1 && state.editors[0] === "realism" &&
                state.field === "standard_deviation" && state.row === "0";
            } catch {
              return false;
            }
          },
          8_000
        );
        if (process.env.TRUST_PLANT_RENDERED_SCREENSHOT) {
          await vscode.commands.executeCommand("notifications.clearAll");
          await inner("d.activeElement?.scrollIntoView({block:'center'});return 'SCROLLED';");
          await sleep(500);
          captureRenderedWorkbench(process.env.TRUST_PLANT_RENDERED_SCREENSHOT);
        }
      } finally {
        session.close();
      }
    });
  });
}
