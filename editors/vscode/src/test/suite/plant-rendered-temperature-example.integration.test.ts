import * as assert from "assert";
import * as fs from "fs";

import {
  beginPlantDraft,
  bindFirstAddressedSignals,
  captureRenderedWorkbench,
  openRenderedPlantSession,
  pollSave,
  pollValue,
  setWebviewChecked,
  sleep,
} from "./plant-rendered-harness";

const RENDERED_TEST_ENABLED = process.env.TRUST_PLANT_RENDERED_TEST === "1";

if (RENDERED_TEST_ENABLED) {
  suite("Rendered plant editor", function () {
    this.timeout(120_000);

    test("CPUI-005 rendered FOPDT temperature example", async function () {
      const session = await openRenderedPlantSession();
      try {
        const { inner, setValue } = session;
        await beginPlantDraft(session);
        await setValue("select[name=plant-model]", "first_order_dead_time");
        await pollValue(
          "selected FOPDT model",
          async () => inner("var select=d.querySelector('select[name=plant-model]');return select&&select.value;"),
          (value) => value === "first_order_dead_time",
          8_000
        );

        const entry = JSON.parse(await inner("var button=d.querySelector('button[data-plant-temperature-example]');var select=d.querySelector('select[name=plant-model]');return JSON.stringify({exists:Boolean(button),text:(button&&button.textContent||'').trim(),models:select?[...select.options].map((option)=>option.textContent.trim()):[],equipmentChooser:Boolean(d.querySelector('[data-plant-equipment-chooser],[name=plant-equipment]'))});"));
        assert.strictEqual(entry.exists, true, "FOPDT should offer Use temperature example");
        assert.strictEqual(entry.text, "Use temperature example");
        assert.strictEqual(entry.equipmentChooser, false, "the optional example must not introduce an equipment chooser");
        assert.deepStrictEqual(entry.models, [
          "Static gain",
          "First order",
          "First order with dead time",
          "Integrator",
          "Second order",
          "Piecewise linear",
          "State space",
          "Gravity-drained tank",
          "Cascaded gravity tanks",
          "Gravity-loaded robot joint",
        ]);

        await bindFirstAddressedSignals(session);
        await setValue("input[name=plant-dynamics-gain]", "2");
        await inner("d.querySelector('button[data-plant-temperature-example]').click();return 'CLICKED';");
        await sleep(100);
        const confirmation = JSON.parse(await inner("var dialog=d.querySelector('[data-plant-starting-point-confirmation]');var buttons=dialog?[...dialog.querySelectorAll('button')].map((button)=>(button.textContent||'').trim()):[];return JSON.stringify({exists:Boolean(dialog),text:(dialog&&dialog.textContent||'').replace(/\\s+/g,' ').trim(),buttons});"));
        assert.strictEqual(confirmation.exists, true, "replacing edited values should use the existing confirmation");
        assert.match(confirmation.text, /model values/i);
        assert.match(confirmation.text, /starting state/i);
        assert.match(confirmation.text, /actuator settings/i);
        assert.match(confirmation.text, /signal conversions/i);
        assert.ok(confirmation.buttons.includes("Keep current values"));
        assert.ok(confirmation.buttons.includes("Replace values"));

        await inner("var dialog=d.querySelector('[data-plant-starting-point-confirmation]');var button=dialog&&[...dialog.querySelectorAll('button')].find((candidate)=>(candidate.textContent||'').trim()==='Keep current values');button.click();return 'KEPT';");
        await sleep(100);
        const kept = JSON.parse(await inner("var gain=d.querySelector('input[name=plant-dynamics-gain]');return JSON.stringify({gain:gain&&gain.value,exampleFields:d.querySelectorAll('[data-plant-temperature-field]').length,confirmation:Boolean(d.querySelector('[data-plant-starting-point-confirmation]'))});"));
        assert.deepStrictEqual(kept, { gain: "2", exampleFields: 0, confirmation: false });

        await inner("d.querySelector('button[data-plant-temperature-example]').click();return 'CLICKED';");
        await sleep(100);
        await inner("var dialog=d.querySelector('[data-plant-starting-point-confirmation]');var button=dialog&&[...dialog.querySelectorAll('button')].find((candidate)=>(candidate.textContent||'').trim()==='Replace values');button.click();return 'REPLACED';");
        await pollValue(
          "temperature example physical fields",
          async () => inner("return String(d.querySelectorAll('[data-plant-temperature-field]').length);"),
          (value) => value === "4",
          8_000
        );

        const example = JSON.parse(await inner("var value=(selector)=>d.querySelector(selector)?.value??null;var label=(selector)=>{var field=d.querySelector(selector);return (field&&field.closest('label')?.textContent||'').replace(/\\s+/g,' ').trim()};var body=(d.querySelector('[data-plant-layout=main-canvas]')?.textContent||'').replace(/\\s+/g,' ');return JSON.stringify({resting:value('[data-plant-temperature-field=resting_temperature]'),rise:value('[data-plant-temperature-field=full_power_rise]'),response:value('[data-plant-temperature-field=time_constant_s]'),delay:value('[data-plant-temperature-field=dead_time_s]'),starting:value('input[name=plant-dynamics-starting-value]'),restingLabel:label('[data-plant-temperature-field=resting_temperature]'),riseLabel:label('[data-plant-temperature-field=full_power_rise]'),responseLabel:label('[data-plant-temperature-field=time_constant_s]'),delayLabel:label('[data-plant-temperature-field=dead_time_s]'),startingLabel:label('input[name=plant-dynamics-starting-value]'),banner:/Example settings\\s*[—-]\\s*not calibrated to your equipment/i.test(body),commandAssumption:body.includes('PLC 0–100 = heater 0–100%'),feedbackAssumption:body.includes('PLC feedback = temperature in °C'),confirmed:d.querySelector('input[name=plant-temperature-units-confirmed]')?.checked??null});"));
        assert.deepStrictEqual(
          { resting: example.resting, rise: example.rise, response: example.response, delay: example.delay, starting: example.starting },
          { resting: "20", rise: "100", response: "30", delay: "2", starting: "20" }
        );
        assert.match(example.restingLabel, /resting temperature.*°C/i);
        assert.match(example.riseLabel, /(?:full-power rise|rise at full power).*°C/i);
        assert.match(example.responseLabel, /response time.*s/i);
        assert.match(example.delayLabel, /delay.*s/i);
        assert.match(example.startingLabel, /starting temperature.*°C/i);
        assert.strictEqual(example.banner, true);
        assert.strictEqual(example.commandAssumption, true);
        assert.strictEqual(example.feedbackAssumption, true);
        assert.strictEqual(example.confirmed, false);
        await pollSave(inner, true, "unconfirmed temperature units");

        const keyboard = [];
        for (const selector of [
          "select[name=plant-model]",
          "select[name=plant-command-source]",
          "select[name=plant-feedback-target]",
          "input[name=plant-dynamics-starting-value]",
        ]) {
          const received = await inner(`var el=d.querySelector(${JSON.stringify(selector)});if(!el)return 'false';el.focus();return String(d.activeElement===el);`) === "true";
          await session.pressTab();
          const moved = await inner(`var el=d.querySelector(${JSON.stringify(selector)});return String(Boolean(el&&d.activeElement!==el));`) === "true";
          keyboard.push({ selector, received, moved });
        }
        assert.ok(keyboard.every(({ received, moved }) => received && moved), `basic temperature setup controls must participate in keyboard focus: ${JSON.stringify(keyboard)}`);

        const surface = JSON.parse(await inner("var panel=d.querySelector('[data-plant-layout=main-canvas]');var model=d.querySelector('[data-plant-setup-section=model-behavior]');var connections=d.querySelector('[data-plant-setup-section=connections]');var starting=d.querySelector('[data-plant-setup-section=starting-conditions]');var panelRect=panel?.getBoundingClientRect();var offenders=panel&&panelRect?[...panel.querySelectorAll('*')].map((el)=>{var rect=el.getBoundingClientRect();var style=getComputedStyle(el);return {tag:el.tagName.toLowerCase(),className:typeof el.className==='string'?el.className:'',text:(el.textContent||'').replace(/\\s+/g,' ').trim().slice(0,100),left:Math.round(rect.left),right:Math.round(rect.right),width:Math.round(rect.width),scrollWidth:el.scrollWidth,clientWidth:el.clientWidth,minWidth:style.minWidth,cssWidth:style.width,whiteSpace:style.whiteSpace,overflowX:style.overflowX};}).filter((item)=>item.right>panelRect.right+1||item.scrollWidth>item.clientWidth+1).sort((a,b)=>Math.max(b.right-panelRect.right,b.scrollWidth-b.clientWidth)-Math.max(a.right-panelRect.right,a.scrollWidth-a.clientWidth)).slice(0,30):[];return JSON.stringify({clientWidth:panel?.clientWidth,scrollWidth:panel?.scrollWidth,overflowX:Boolean(panel&&panel.scrollWidth>panel.clientWidth+1),panelRight:panelRect&&Math.round(panelRect.right),modelWidth:model?.getBoundingClientRect().width,connectionsWidth:connections?.getBoundingClientRect().width,startingWidth:starting?.getBoundingClientRect().width,offenders});"));

        const screenshot = process.env.TRUST_PLANT_RENDERED_SCREENSHOT?.trim();
        if (screenshot) {
          await inner("var panel=d.querySelector('[data-plant-layout=main-canvas]');if(panel)panel.scrollTop=0;return 'SCROLLED';");
          await sleep(250);
          captureRenderedWorkbench(screenshot);
          const evidencePath = screenshot.toLowerCase().endsWith(".png") ? `${screenshot.slice(0, -4)}.json` : `${screenshot}.json`;
          fs.writeFileSync(evidencePath, JSON.stringify({
            theme: process.env.TRUST_PLANT_RENDERED_THEME || "default",
            split: process.env.TRUST_PLANT_RENDERED_SPLIT === "1",
            surface,
            keyboard,
            example,
          }, null, 2));
        }
        assert.strictEqual(surface.overflowX, false, `temperature setup must not overflow horizontally: ${JSON.stringify(surface)}`);
        assert.ok(surface.clientWidth > 0 && surface.modelWidth > 0 && surface.connectionsWidth > 0 && surface.startingWidth > 0);

        await setWebviewChecked(inner, "input[name=plant-temperature-units-confirmed]", true);
        await pollSave(inner, false, "confirmed temperature units");

        await inner("var button=d.querySelector('[data-plant-model-details]');if(button&&button.getAttribute('aria-expanded')!=='true')button.click();return 'OPEN';");
        await pollValue(
          "temperature model details",
          async () => inner("return String(Boolean(d.querySelector('[data-plant-dynamics-field=initial_delayed_input]')));"),
          (value) => value === "true",
          8_000
        );
        const stored = JSON.parse(await inner("var value=(selector)=>d.querySelector(selector)?.value??null;return JSON.stringify({gain:value('[data-plant-dynamics-field=gain]'),timeConstant:value('[data-plant-dynamics-field=time_constant_s]'),delay:value('[data-plant-dynamics-field=dead_time_s]'),inputBias:value('[data-plant-dynamics-field=input_bias]'),outputBias:value('[data-plant-dynamics-field=output_bias]'),delayHistory:value('[data-plant-dynamics-field=initial_delayed_input]'),initialState:value('[data-plant-dynamics-field=initial_state]')});"));
        assert.deepStrictEqual(stored, {
          gain: "1",
          timeConstant: "30",
          delay: "2",
          inputBias: "0",
          outputBias: "20",
          delayHistory: "0",
          initialState: "[20]",
        });
        const equation = await inner("var equation=d.querySelector('[data-plant-temperature-equation]');return (equation?.textContent||'').replace(/\\s+/g,' ').trim();");
        assert.ok(
          equation.includes("20 + 1 × (power − 0)"),
          `temperature example should show the canonical contextual equation: ${equation}`
        );
        assert.match(equation, /gain.*1\s*°C\s*\/\s*%/i, "the equation explanation should state gain units");
        assert.match(equation, /full-power rise.*100\s*°C/i, "the equation explanation should distinguish the full-power rise");

        await setValue("[data-plant-temperature-field=full_power_rise]", "-200");
        await pollValue(
          "negative physical rise mapped to model gain",
          async () => inner("var gain=d.querySelector('[data-plant-dynamics-field=gain]');var rest=d.querySelector('[data-plant-temperature-field=resting_temperature]');var start=d.querySelector('input[name=plant-dynamics-starting-value]');return JSON.stringify({gain:gain?.value,rest:rest?.value,start:start?.value});"),
          (value) => value === JSON.stringify({ gain: "-2", rest: "20", start: "20" }),
          8_000
        );

        await setValue("[data-plant-temperature-field=time_constant_s]", "0");
        await pollValue(
          "invalid zero response time presentation",
          async () => inner("var field=d.querySelector('[data-plant-temperature-field=time_constant_s]');return JSON.stringify({value:field?.value,invalid:field?.getAttribute('aria-invalid')});"),
          (value) => value === JSON.stringify({ value: "0", invalid: "true" }),
          8_000
        );
        await pollSave(inner, true, "zero temperature example response time");
        await setValue("[data-plant-temperature-field=time_constant_s]", "30");
        await pollSave(inner, false, "corrected temperature example response time");

        await setValue("[data-plant-temperature-field=resting_temperature]", "not-a-temperature");
        await pollValue(
          "invalid raw resting temperature remains visible",
          async () => inner("var field=d.querySelector('[data-plant-temperature-field=resting_temperature]');return JSON.stringify({value:field?.value,invalid:field?.getAttribute('aria-invalid')});"),
          (value) => value === JSON.stringify({ value: "not-a-temperature", invalid: "true" }),
          8_000
        );
        await pollSave(inner, true, "invalid raw resting temperature");
        await setValue("[data-plant-temperature-field=resting_temperature]", "20");
        await pollSave(inner, false, "corrected raw resting temperature");

        await setValue("[data-plant-dynamics-field=gain]", "2");
        await pollValue(
          "canonical model gain reflected in physical rise",
          async () => inner("return d.querySelector('[data-plant-temperature-field=full_power_rise]')?.value||'';"),
          (value) => value === "200",
          8_000
        );
        await setValue("[data-plant-temperature-field=full_power_rise]", "300");
        await pollValue(
          "physical rise reflected in canonical model gain",
          async () => inner("return d.querySelector('[data-plant-dynamics-field=gain]')?.value||'';"),
          (value) => value === "3",
          8_000
        );

        await setValue("[data-plant-temperature-field=resting_temperature]", "stale-invalid-temperature");
        await pollValue(
          "invalid raw temperature before reapplying the example",
          async () => inner("var field=d.querySelector('[data-plant-temperature-field=resting_temperature]');return JSON.stringify({value:field?.value,invalid:field?.getAttribute('aria-invalid')});"),
          (value) => value === JSON.stringify({ value: "stale-invalid-temperature", invalid: "true" }),
          8_000
        );
        await pollSave(inner, true, "invalid raw temperature before example reapply");
        await inner("d.querySelector('button[data-plant-temperature-example]').click();return 'CLICKED';");
        await pollValue(
          "temperature example reapply confirmation",
          async () => inner("return String(Boolean(d.querySelector('[data-plant-starting-point-confirmation]')));"),
          (value) => value === "true",
          8_000
        );
        await inner("var dialog=d.querySelector('[data-plant-starting-point-confirmation]');var button=dialog&&[...dialog.querySelectorAll('button')].find((candidate)=>(candidate.textContent||'').trim()==='Replace values');button.click();return 'REPLACED';");
        await pollValue(
          "temperature example reapply completed",
          async () => inner("return String(Boolean(d.querySelector('[data-plant-starting-point-confirmation]')));"),
          (value) => value === "false",
          8_000
        );
        await sleep(100);
        const reapplied = JSON.parse(await inner("var field=d.querySelector('[data-plant-temperature-field=resting_temperature]');var confirmed=d.querySelector('input[name=plant-temperature-units-confirmed]');return JSON.stringify({value:field?.value,invalid:field?.getAttribute('aria-invalid'),confirmed:confirmed?.checked});"));
        assert.deepStrictEqual(reapplied, {
          value: "20",
          invalid: "false",
          confirmed: false,
        }, "reapplying the example must discard stale invalid raw text and require unit reconfirmation");
        await pollSave(inner, true, "reapplied example units require confirmation");
        await setWebviewChecked(inner, "input[name=plant-temperature-units-confirmed]", true);
        await pollSave(inner, false, "reapplied example units confirmed");

        await inner("d.querySelector('[data-plant-optional-open=actuator]')?.click();return 'OPEN';");
        await pollValue(
          "temperature actuator settings",
          async () => inner("return String(Boolean(d.querySelector('[data-plant-actuator-field=maximum]')));"),
          (value) => value === "true",
          8_000
        );
        const actuatorSettings = JSON.parse(await inner("var value=(selector)=>d.querySelector(selector)?.value??null;return JSON.stringify({minimum:value('[data-plant-actuator-field=minimum]'),maximum:value('[data-plant-actuator-field=maximum]'),initialApplied:value('[data-plant-actuator-field=initial_applied]')});"));
        await inner("d.querySelector('[data-plant-optional-open=signals]')?.click();return 'OPEN';");
        await pollValue(
          "temperature signal conversions",
          async () => inner("return String(Boolean(d.querySelector('[data-plant-engineering-field=\"initial_input.0\"]')));"),
          (value) => value === "true",
          8_000
        );
        const signalSettings = JSON.parse(await inner("var value=(selector)=>d.querySelector(selector)?.value??null;return JSON.stringify({initialInput:value('[data-plant-engineering-field=\"initial_input.0\"]'),inputScale:value('[data-plant-engineering-field=\"input_scale.0\"]'),inputOffset:value('[data-plant-engineering-field=\"input_offset.0\"]'),outputScale:value('[data-plant-engineering-field=\"output_scale.0\"]'),outputOffset:value('[data-plant-engineering-field=\"output_offset.0\"]')});"));
        const engineering = { ...actuatorSettings, ...signalSettings };
        assert.deepStrictEqual(engineering, {
          minimum: "0",
          maximum: "100",
          initialApplied: "0",
          initialInput: "0",
          inputScale: "1",
          inputOffset: "0",
          outputScale: "1",
          outputOffset: "0",
        });
      } finally {
        session.close();
      }
    });
  });
}
