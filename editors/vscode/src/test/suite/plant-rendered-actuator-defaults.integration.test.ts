import * as assert from "assert";

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

    test("CPFIX-008 rendered actuator blank defaults", async function () {
      const session = await openRenderedPlantSession();
      try {
        const { inner, setValue } = session;
        await beginPlantDraft(session);
        await setValue("select[name=plant-model]", "first_order_dead_time");
        await pollValue(
          "selected FOPDT model",
          async () => inner("return d.querySelector('select[name=plant-model]')?.value||'';"),
          (value) => value === "first_order_dead_time",
          8_000
        );
        await bindFirstAddressedSignals(session);

        await inner("d.querySelector('[data-plant-optional-open=actuator]')?.click();return 'OPEN';");
        await pollValue(
          "actuator engineering fields",
          async () => inner("return String(d.querySelectorAll('[data-plant-actuator-field]').length);"),
          (value) => value === "10",
          8_000
        );

        const resolved = JSON.parse(await inner("var keys=['minimum','maximum','rate_up_per_s','rate_down_per_s','deadband','stiction_band','slip_jump','characteristic_exponent','lag_s','initial_applied'];return JSON.stringify(keys.map((key)=>{var input=d.querySelector('[data-plant-actuator-field='+key+']');var meaning=d.querySelector('[data-plant-actuator-blank-meaning='+key+']');return {key,value:input?.value??null,placeholder:input?.getAttribute('placeholder'),meaning:(meaning?.textContent||'').replace(/\\s+/g,' ').trim()};}));"));
        assert.strictEqual(
          resolved.find((field: { key: string }) => field.key === "deadband")?.placeholder,
          "0 (no deadband)",
          "blank deadband must not look unbounded"
        );
        assert.deepStrictEqual(resolved, [
          { key: "minimum", value: "", placeholder: "Unbounded", meaning: "Empty: Unbounded" },
          { key: "maximum", value: "", placeholder: "Unbounded", meaning: "Empty: Unbounded" },
          { key: "rate_up_per_s", value: "", placeholder: "Unbounded; 0 blocks upward movement", meaning: "Empty: Unbounded; 0 blocks upward movement" },
          { key: "rate_down_per_s", value: "", placeholder: "Unbounded; 0 blocks downward movement", meaning: "Empty: Unbounded; 0 blocks downward movement" },
          { key: "deadband", value: "", placeholder: "0 (no deadband)", meaning: "Empty: 0 (no deadband)" },
          { key: "stiction_band", value: "", placeholder: "0 (no static friction)", meaning: "Empty: 0 (no static friction)" },
          { key: "slip_jump", value: "", placeholder: "0 (no breakaway jump)", meaning: "Empty: 0 (no breakaway jump)" },
          { key: "characteristic_exponent", value: "", placeholder: "1 (linear characteristic)", meaning: "Empty: 1 (linear characteristic)" },
          { key: "lag_s", value: "", placeholder: "0 (no lag)", meaning: "Empty: 0 (no lag)" },
          { key: "initial_applied", value: "", placeholder: "Initial input (resolved at runtime)", meaning: "Empty: Initial input (resolved at runtime)" },
        ]);

        await setValue("[data-plant-actuator-field=rate_up_per_s]", "0");
        await pollValue(
          "explicit zero rising rate",
          async () => inner("var field=d.querySelector('[data-plant-actuator-field=rate_up_per_s]');return JSON.stringify({value:field?.value,invalid:field?.getAttribute('aria-invalid')});"),
          (value) => value === JSON.stringify({ value: "0", invalid: "false" }),
          8_000
        );
        await pollSave(inner, false, "explicit zero rising rate");

        await setValue("[data-plant-actuator-field=rate_up_per_s]", "");
        await pollValue(
          "absent rising rate",
          async () => inner("var field=d.querySelector('[data-plant-actuator-field=rate_up_per_s]');return JSON.stringify({value:field?.value,invalid:field?.getAttribute('aria-invalid')});"),
          (value) => value === JSON.stringify({ value: "", invalid: "false" }),
          8_000
        );
        await pollSave(inner, false, "absent unbounded rising rate");
      } finally {
        session.close();
      }
    });

    test("CPUI-017/019 rendered optional task destinations", async function () {
      const session = await openRenderedPlantSession();
      try {
        const { inner, setValue } = session;
        await beginPlantDraft(session);
        await setValue("select[name=plant-model]", "first_order_dead_time");
        await bindFirstAddressedSignals(session);
        await pollSave(inner, false, "valid draft before optional tasks");

        const initial = JSON.parse(await inner("var text=(selector)=>(d.querySelector(selector)?.textContent||'').replace(/\\s+/g,' ').trim();return JSON.stringify({destinations:[...d.querySelectorAll('[data-plant-optional-destination]')].map((x)=>x.getAttribute('data-plant-optional-destination')),legacy:[...d.querySelectorAll('button')].some((x)=>/engineering details/i.test(x.textContent||'')),editors:d.querySelectorAll('[data-plant-optional-editor]').length,summaries:{actuator:text('[data-plant-optional-summary=actuator]'),signals:text('[data-plant-optional-summary=signals]'),realism:text('[data-plant-optional-summary=realism]'),scenarios:text('[data-plant-optional-summary=scenarios]')}});"));
        assert.deepStrictEqual(initial.destinations, ["actuator", "signals", "realism", "scenarios"]);
        assert.strictEqual(initial.legacy, false, "the all-at-once engineering disclosure must be removed");
        assert.strictEqual(initial.editors, 0, "optional editors should start closed");
        assert.match(initial.summaries.actuator, /no actuator limits or response effects/i);
        assert.match(initial.summaries.signals, /no signal scaling or offset/i);
        assert.match(initial.summaries.realism, /no process or sensor imperfections/i);
        assert.match(initial.summaries.scenarios, /no scheduled scenarios/i);

        await openOptionalDestination(session, "actuator");
        await pollValue(
          "only actuator destination open",
          async () => optionalEditorState(inner),
          (value) => value === "actuator:10",
          8_000
        );
        await setValue("[data-plant-actuator-field=maximum]", "80");

        await openOptionalDestination(session, "signals");
        await pollValue(
          "only signal-conversion destination open",
          async () => optionalEditorState(inner),
          (value) => value === "signals:6",
          8_000
        );
        // The summary is the language server's answer for the draft, which comes after the edit.
        await pollValue(
          "actuator summary names the maximum",
          async () => inner("return (d.querySelector('[data-plant-optional-summary=actuator]')?.textContent||'').replace(/\\s+/g,' ').trim();"),
          (value) => /maximum 80/i.test(value),
          8_000
        );
        await setValue('[data-plant-engineering-field="input_scale.0"]', "2");

        await openOptionalDestination(session, "realism");
        await pollValue(
          "only realism destination open",
          async () => optionalEditorState(inner),
          (value) => value === "realism:0",
          8_000
        );
        await inner("d.querySelector('[data-plant-uncertainty-add]')?.click();return 'ADDED';");
        await inner("d.querySelector('[data-plant-uncertainty-add-type=bias]')?.click();return 'BIAS_SELECTED';");
        await pollValue(
          "configured realism summary",
          async () => inner("return (d.querySelector('[data-plant-optional-summary=realism]')?.textContent||'').replace(/\\s+/g,' ').trim();"),
          (value) => /1 enabled effect/i.test(value) && /sensor 1/i.test(value),
          8_000
        );
        await inner("var row=d.querySelector('[data-plant-uncertainty-row=\"0\"]');var include=row?.querySelector('input[type=checkbox]');include?.click();return 'EXCLUDED';");
        await pollValue(
          "configured but disabled realism summary",
          async () => inner("return (d.querySelector('[data-plant-optional-summary=realism]')?.textContent||'').replace(/\\s+/g,' ').trim();"),
          (value) => /1 configured effect/i.test(value) && /0 enabled/i.test(value) && /sensor 1/i.test(value),
          8_000
        );
        await inner("var row=d.querySelector('[data-plant-uncertainty-row=\"0\"]');var include=row?.querySelector('input[type=checkbox]');include?.click();return 'INCLUDED';");

        await openOptionalDestination(session, "scenarios");
        await pollValue(
          "only scenarios destination open",
          async () => optionalEditorState(inner),
          (value) => value === "scenarios:0",
          8_000
        );
        await inner("d.querySelector('[data-plant-scenario-event-add]')?.click();return 'ADDED';");
        await pollValue(
          "configured scenario summary",
          async () => inner("return (d.querySelector('[data-plant-optional-summary=scenarios]')?.textContent||'').replace(/\\s+/g,' ').trim();"),
          (value) => /1 included event/i.test(value) && /first at 0 s/i.test(value),
          8_000
        );
        await inner("var row=d.querySelector('[data-plant-scenario-event-row=\"0\"]');var include=row?.querySelector('input[type=checkbox]');include?.click();return 'EXCLUDED';");
        await pollValue(
          "configured but excluded scenario summary",
          async () => inner("return (d.querySelector('[data-plant-optional-summary=scenarios]')?.textContent||'').replace(/\\s+/g,' ').trim();"),
          (value) => /1 configured event/i.test(value) && /0 included/i.test(value),
          8_000
        );
        await inner("var row=d.querySelector('[data-plant-scenario-event-row=\"0\"]');var include=row?.querySelector('input[type=checkbox]');include?.click();return 'INCLUDED';");
        await pollValue(
          "signal summary counts the changed scaling",
          async () => inner("return (d.querySelector('[data-plant-optional-summary=signals]')?.textContent||'').replace(/\\s+/g,' ').trim();"),
          (value) => /1 scaling or offset value changed/i.test(value),
          8_000
        );

        await setValue("input[name=plant-dynamics-gain]", "bad");
        await pollSave(inner, true, "invalid dynamics while optional destination is open");
        await openOptionalDestination(session, "scenarios");
        // The gain's problem shows again once the language server has answered the draft as it is now.
        await pollValue(
          "invalid gain kept while the optional destination closes",
          async () => inner("var gain=d.querySelector('input[name=plant-dynamics-gain]');return JSON.stringify({value:gain?.value,invalid:gain?.getAttribute('aria-invalid'),editors:d.querySelectorAll('[data-plant-optional-editor]').length,summaries:d.querySelectorAll('[data-plant-optional-summary]').length});"),
          (value) => value === JSON.stringify({ value: "bad", invalid: "true", editors: 0, summaries: 4 }),
          8_000
        );
        await pollSave(inner, true, "invalid dynamics after closing optional destination");

        await setValue("input[name=plant-dynamics-gain]", "1");
        await pollSave(inner, false, "corrected draft with optional settings retained");
        await openOptionalDestination(session, "scenarios");
        const screenshot = process.env.TRUST_PLANT_RENDERED_SCREENSHOT?.trim();
        if (screenshot) {
          await inner("d.querySelector('[data-plant-optional-settings]')?.scrollIntoView({block:'start'});return 'SCROLLED';");
          await sleep(500);
          captureRenderedWorkbench(screenshot);
        }
      } finally {
        session.close();
      }
    });
  });
}

async function openOptionalDestination(
  session: Awaited<ReturnType<typeof openRenderedPlantSession>>,
  destination: "actuator" | "signals" | "realism" | "scenarios"
): Promise<void> {
  await session.inner(`var button=d.querySelector('[data-plant-optional-open=${destination}]');if(!button)return 'NO_DESTINATION';button.click();return 'CLICKED';`);
}

function optionalEditorState(inner: (body: string) => Promise<string>): Promise<string> {
  return inner("var editors=[...d.querySelectorAll('[data-plant-optional-editor]')];if(editors.length!==1)return 'COUNT:'+editors.length;var editor=editors[0].getAttribute('data-plant-optional-editor');var fields=editor==='actuator'?d.querySelectorAll('[data-plant-actuator-field]').length:editor==='signals'?d.querySelectorAll('[data-plant-engineering-field]').length:0;return editor+':'+fields;");
}
