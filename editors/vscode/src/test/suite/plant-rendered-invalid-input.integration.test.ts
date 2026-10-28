import * as assert from "assert";

import {
  beginPlantDraft,
  bindFirstAddressedSignals,
  fieldState,
  openRenderedPlantSession,
  pollSave,
  pollValue,
} from "./plant-rendered-harness";

const RENDERED_TEST_ENABLED = process.env.TRUST_PLANT_RENDERED_TEST === "1";

if (RENDERED_TEST_ENABLED) {
  suite("Rendered plant editor", function () {
    this.timeout(120_000);

    test("CPFIX-005 rendered plant invalid input guard", async function () {
      const session = await openRenderedPlantSession();
      try {
        const { inner, setValue } = session;
        const { add, plant } = await beginPlantDraft(session);
        assert.strictEqual(add, "ADD");
        assert.strictEqual(plant, "PLANT");
        const setupLayout = JSON.parse(await inner("var panel=d.querySelector('[data-plant-layout=main-canvas]');var rect=panel&&panel.getBoundingClientRect();return JSON.stringify({width:rect?rect.width:0,sections:[...d.querySelectorAll('[data-plant-setup-section]')].map((x)=>x.getAttribute('data-plant-setup-section')),optional:[...d.querySelectorAll('[data-plant-optional-destination]')].map((x)=>x.getAttribute('data-plant-optional-destination')),explanations:d.querySelectorAll('[data-plant-model-explanation]').length});"));
        assert.ok(setupLayout.width > 600, `plant setup should use the main canvas, got ${setupLayout.width}px`);
        assert.deepStrictEqual(setupLayout.sections, ["model-behavior", "connections", "starting-conditions"]);
        assert.deepStrictEqual(setupLayout.optional, ["actuator", "signals", "realism", "scenarios"]);
        assert.strictEqual(setupLayout.explanations, 1);
        await setValue("select[name=plant-model]", "first_order_dead_time");
        await pollValue(
          "FOPDT fields",
          async () => inner("return [...d.querySelectorAll('[data-plant-dynamics-field]')].map((x)=>x.getAttribute('data-plant-dynamics-field')).join(',');"),
          (value) => value === "gain,time_constant_s,dead_time_s,starting_value",
          10_000
        );
        const organization = JSON.parse(await inner("var section=(selector)=>{var el=d.querySelector(selector);var owner=el&&el.closest('[data-plant-setup-section]');return owner&&owner.getAttribute('data-plant-setup-section')};var collapsed=(selector)=>{var el=d.querySelector(selector);var details=el&&el.closest('details');return Boolean(details&&!details.open)};var explanation=(d.querySelector('[data-plant-model-explanation]')?.textContent||'').trim();var panelText=(d.querySelector('[data-plant-layout=main-canvas]')?.textContent||'');var purposeCopies=explanation?panelText.split(explanation).length-1:0;return JSON.stringify({model:section('select[name=plant-model]'),command:section('select[name=plant-command-source]'),feedback:section('select[name=plant-feedback-target]'),startingValue:section('[data-plant-dynamics-field=starting_value]'),technicalIdCollapsed:collapsed('input[name=plant-id]'),descriptionCollapsed:collapsed('textarea[name=plant-description]'),purposeCopies:purposeCopies});"));
        assert.strictEqual(organization.model, "model-behavior", "model choice should live in Model & behavior");
        assert.strictEqual(organization.command, "connections", "command signal should live in Connections");
        assert.strictEqual(organization.feedback, "connections", "feedback signal should live in Connections");
        assert.strictEqual(organization.startingValue, "starting-conditions", "FOPDT starting value should live in Starting conditions");
        assert.strictEqual(organization.technicalIdCollapsed, true, "technical plant ID should start inside collapsed advanced details");
        assert.strictEqual(organization.descriptionCollapsed, true, "technical description should start inside collapsed advanced details");
        assert.strictEqual(organization.purposeCopies, 1, "model purpose should be explained once");

        await bindFirstAddressedSignals(session);
        await pollSave(inner, false, "initial valid draft");

        await setValue("input[name=plant-dynamics-gain]", "bad");
        await pollValue(
          "invalid raw gain",
          async () => fieldState(inner, "input[name=plant-dynamics-gain]"),
          (value) => {
            const state = JSON.parse(value);
            return state.value === "bad" && state.invalid === "true";
          },
          8_000
        );
        await pollSave(inner, true, "invalid gain");
        await inner("var button=d.querySelector('[data-plant-model-details]');if(!button)return 'NO_DETAILS';button.click();return 'CLICKED';");
        await pollValue(
          "open Model details",
          async () => inner("var button=d.querySelector('[data-plant-model-details]');var state=d.querySelector('textarea[name=plant-dynamics-initial_state]');return JSON.stringify({open:button&&button.getAttribute('aria-expanded'),state:Boolean(state)});"),
          (value) => {
            const state = JSON.parse(value);
            return state.open === "true" && state.state;
          },
          8_000
        );
        await setValue("textarea[name=plant-dynamics-initial_state]", "[bad]");
        await pollValue(
          "invalid raw initial-state vector",
          async () => fieldState(inner, "textarea[name=plant-dynamics-initial_state]"),
          (value) => {
            const state = JSON.parse(value);
            return state.value === "[bad]" && state.invalid === "true";
          },
          8_000
        );
        await pollSave(inner, true, "invalid gain and state vector");

        await inner("d.querySelector('[data-plant-model-details]').click();return 'CLOSED';");
        await pollValue(
          "closed Model details",
          async () => inner("return String(Boolean(d.querySelector('textarea[name=plant-dynamics-initial_state]')));"),
          (value) => value === "false",
          8_000
        );
        await inner("d.querySelector('[data-plant-model-details]').click();return 'OPENED';");
        const reopenedInvalid = JSON.parse(await pollValue(
          "reopened invalid state",
          async () => fieldState(inner, "textarea[name=plant-dynamics-initial_state]"),
          (value) => {
            const state = JSON.parse(value);
            return state.value === "[bad]" && state.invalid === "true";
          },
          8_000
        ));
        assert.deepStrictEqual(reopenedInvalid, { value: "[bad]", invalid: "true" });

        await setValue("input[name=plant-dynamics-gain]", "2.5");
        await pollValue(
          "corrected gain while state remains invalid",
          async () => fieldState(inner, "input[name=plant-dynamics-gain]"),
          (value) => {
            const state = JSON.parse(value);
            return state.value === "2.5" && state.invalid === "false";
          },
          8_000
        );
        await pollSave(inner, true, "valid gain with invalid state vector");
        await setValue("textarea[name=plant-dynamics-initial_state]", "[3]");
        await pollSave(inner, false, "corrected gain and state vector");
        const recovered = JSON.parse(await fieldState(inner, "textarea[name=plant-dynamics-initial_state]"));
        assert.deepStrictEqual(recovered, { value: "[3]", invalid: "false" });

        await inner("d.querySelector('[data-plant-model-details]').click();return 'CLOSED';");
        await pollValue(
          "closed corrected Model details",
          async () => inner("return String(Boolean(d.querySelector('textarea[name=plant-dynamics-initial_state]')));"),
          (value) => value === "false",
          8_000
        );
        await inner("d.querySelector('[data-plant-model-details]').click();return 'OPENED';");
        const reopenedRecovered = JSON.parse(await pollValue(
          "reopened corrected state",
          async () => fieldState(inner, "textarea[name=plant-dynamics-initial_state]"),
          (value) => JSON.parse(value).value === "[3]",
          8_000
        ));
        assert.deepStrictEqual(reopenedRecovered, { value: "[3]", invalid: "false" });
      } finally {
        session.close();
      }
    });
  });
}
