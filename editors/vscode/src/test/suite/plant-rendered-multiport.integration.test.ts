import * as assert from "assert";
import * as fs from "fs";
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

type RenderedPort = {
  name: string;
  direction: string | null;
  index: string | null;
  label: string;
  value: string;
};

if (RENDERED_TEST_ENABLED) {
  suite("Rendered plant editor", function () {
    this.timeout(120_000);

    test("CPFIX-004 rendered robot joint multiport bindings", async function () {
      const session = await openRenderedPlantSession();
      try {
        await beginPlantDraft(session);
        await selectModel(session, "robot_joint");

        const initialStartingConditions = await readStartingConditions(session);
        assert.deepStrictEqual(initialStartingConditions, {
          fields: [
            { index: "0", label: "Starting angle (rad)", value: "0.5", invalid: "false" },
            { index: "1", label: "Starting angular speed (rad/s)", value: "0", invalid: "false" },
          ],
          rawInitialStateInDynamics: false,
        }, "robot starting conditions should expose two indexed numeric fields without a duplicate raw state array");

        const ports = await readPorts(session);
        assert.deepStrictEqual(ports.map(({ name, direction, index, label }) => ({ name, direction, index, label })), [
          { name: "plant-command-source", direction: "command", index: "0", label: "Torque command (N·m)" },
          { name: "plant-feedback-target", direction: "feedback", index: "0", label: "Joint angle (rad)" },
          { name: "plant-feedback-target-1", direction: "feedback", index: "1", label: "Angular speed (rad/s)" },
        ], "robot joint should render one command and two separately addressable feedback ports");

        await pollSave(session.inner, true, "unbound robot joint ports");
        await chooseAddress(session, "plant-command-source", "%QL0");
        await chooseAddress(session, "plant-feedback-target", "%IL0");
        await pollSave(session.inner, true, "robot joint missing angular-speed feedback");
        await chooseAddress(session, "plant-feedback-target-1", "%IL8");
        await pollSave(session.inner, false, "fully bound robot joint ports");

        await session.setValue('[data-plant-starting-state="0"]', "-0.75");
        await pollValue(
          "negative robot starting angle",
          async () => JSON.stringify((await readStartingConditions(session)).fields),
          (value) => value === JSON.stringify([
            { index: "0", label: "Starting angle (rad)", value: "-0.75", invalid: "false" },
            { index: "1", label: "Starting angular speed (rad/s)", value: "0", invalid: "false" },
          ]),
          8_000
        );
        await session.setValue('[data-plant-starting-state="1"]', "1.25");
        await pollValue(
          "nonzero robot starting angular speed",
          async () => JSON.stringify((await readStartingConditions(session)).fields),
          (value) => value === JSON.stringify([
            { index: "0", label: "Starting angle (rad)", value: "-0.75", invalid: "false" },
            { index: "1", label: "Starting angular speed (rad/s)", value: "1.25", invalid: "false" },
          ]),
          8_000
        );

        await session.setValue('[data-plant-starting-state="0"]', "not-an-angle");
        await pollValue(
          "invalid raw robot starting angle remains visible",
          async () => JSON.stringify((await readStartingConditions(session)).fields[0]),
          (value) => value === JSON.stringify({ index: "0", label: "Starting angle (rad)", value: "not-an-angle", invalid: "true" }),
          8_000
        );
        await pollSave(session.inner, true, "invalid raw robot starting angle");
        await toggleEngineeringDetails(session, true);
        await toggleEngineeringDetails(session, false);
        assert.deepStrictEqual((await readStartingConditions(session)).fields[0], {
          index: "0",
          label: "Starting angle (rad)",
          value: "not-an-angle",
          invalid: "true",
        }, "optional-section disclosure toggles must retain invalid robot starting text");
        await pollSave(session.inner, true, "invalid robot starting angle after disclosure toggles");
        await session.setValue('[data-plant-starting-state="0"]', "-1.5");
        await pollSave(session.inner, false, "corrected robot starting angle");
        await captureMultiportEvidence(session, "robot_joint");

        await saveAndReopenPlant(session);
        assert.deepStrictEqual(await readStartingConditions(session), {
          fields: [
            { index: "0", label: "Starting angle (rad)", value: "-1.5", invalid: "false" },
            { index: "1", label: "Starting angular speed (rad/s)", value: "1.25", invalid: "false" },
          ],
          rawInitialStateInDynamics: false,
        }, "saved negative angle and nonzero speed must reopen as separate exact starting conditions");
        const reopened = await readPorts(session);
        assert.deepStrictEqual(reopened.map(({ name, value }) => ({ name, value })), [
          { name: "plant-command-source", value: "%QL0" },
          { name: "plant-feedback-target", value: "%IL0" },
          { name: "plant-feedback-target-1", value: "%IL8" },
        ], "saved robot angle and angular-speed feedback bindings must reopen from persisted configuration");
      } finally {
        session.close();
      }
    });

    test("CPUI-006 rendered model starting conditions", async function () {
      const session = await openRenderedPlantSession();
      try {
        await beginPlantDraft(session);
        const expectedFields = [
          { model: "first_order", fields: [{ index: "0", label: "Starting output", value: "0", invalid: "false" }] },
          { model: "integrator", fields: [{ index: "0", label: "Starting accumulated value", value: "0", invalid: "false" }] },
          { model: "gravity_tank", fields: [{ index: "0", label: "Starting level (m)", value: "4", invalid: "false" }] },
          { model: "second_order", fields: [
            { index: "0", label: "Starting displacement", value: "0", invalid: "false" },
            { index: "1", label: "Starting rate of change", value: "0", invalid: "false" },
          ] },
        ] as const;
        for (const expected of expectedFields) {
          await selectModel(session, expected.model);
          assert.deepStrictEqual(await readStartingConditions(session), {
            fields: expected.fields,
            rawInitialStateInDynamics: false,
          }, `${expected.model} should expose labelled indexed starting conditions without a duplicate raw state array`);
        }

        await selectModel(session, "first_order");
        await bindFirstAddressedSignals(session);
        await session.setValue('[data-plant-starting-state="0"]', "not-an-output");
        await pollStartingFields(session, [
          { index: "0", label: "Starting output", value: "not-an-output", invalid: "true" },
        ], "invalid first-order starting output");
        await pollSave(session.inner, true, "invalid first-order starting output");
        await toggleEngineeringDetails(session, true);
        await toggleEngineeringDetails(session, false);
        assert.deepStrictEqual((await readStartingConditions(session)).fields[0], {
          index: "0", label: "Starting output", value: "not-an-output", invalid: "true",
        }, "first-order invalid raw text should survive optional-details disclosure toggles");
        await session.setValue('[data-plant-starting-state="0"]', "-2.5");
        await pollSave(session.inner, false, "corrected first-order starting output");
        await captureStartingConditionsEvidence(session, "first_order");
        await saveAndReopenPlant(session);
        assert.deepStrictEqual(await readStartingConditions(session), {
          fields: [{ index: "0", label: "Starting output", value: "-2.5", invalid: "false" }],
          rawInitialStateInDynamics: false,
        }, "saved first-order starting output should reopen exactly");

        await session.setValue("select[name=plant-model]", "second_order");
        await confirmStartingPointReplacement(session);
        await pollValue(
          "persisted plant replacement with second-order model",
          async () => session.inner("return d.querySelector('select[name=plant-model]')?.value||'';"),
          (value) => value === "second_order",
          8_000
        );
        await session.setValue('[data-plant-starting-state="0"]', "3.5");
        await pollStartingFields(session, [
          { index: "0", label: "Starting displacement", value: "3.5", invalid: "false" },
          { index: "1", label: "Starting rate of change", value: "0", invalid: "false" },
        ], "second-order displacement preserves rate");
        await session.setValue('[data-plant-starting-state="1"]', "-0.25");
        await pollStartingFields(session, [
          { index: "0", label: "Starting displacement", value: "3.5", invalid: "false" },
          { index: "1", label: "Starting rate of change", value: "-0.25", invalid: "false" },
        ], "second-order rate preserves displacement");
        await session.setValue('[data-plant-starting-state="1"]', "not-a-rate");
        await pollStartingFields(session, [
          { index: "0", label: "Starting displacement", value: "3.5", invalid: "false" },
          { index: "1", label: "Starting rate of change", value: "not-a-rate", invalid: "true" },
        ], "invalid second-order rate remains visible");
        await pollSave(session.inner, true, "invalid second-order starting rate");
        await toggleEngineeringDetails(session, true);
        await toggleEngineeringDetails(session, false);
        await pollStartingFields(session, [
          { index: "0", label: "Starting displacement", value: "3.5", invalid: "false" },
          { index: "1", label: "Starting rate of change", value: "not-a-rate", invalid: "true" },
        ], "second-order invalid rate survives disclosure toggles");
        await session.setValue('[data-plant-starting-state="1"]', "1.75");
        await pollSave(session.inner, false, "corrected second-order starting rate");
        await captureStartingConditionsEvidence(session, "second_order");
        await saveAndReopenPlant(session);
        assert.deepStrictEqual(await readStartingConditions(session), {
          fields: [
            { index: "0", label: "Starting displacement", value: "3.5", invalid: "false" },
            { index: "1", label: "Starting rate of change", value: "1.75", invalid: "false" },
          ],
          rawInitialStateInDynamics: false,
        }, "saved second-order starting displacement and rate should reopen exactly");
      } finally {
        session.close();
      }
    });

    test("CPFIX-004 rendered state-space multiport preservation", async function () {
      const session = await openRenderedPlantSession();
      try {
        await beginPlantDraft(session);
        await selectModel(session, "state_space");
        await session.setValue('[data-plant-state-space-dimension="inputs"]', "2");
        await session.setValue('[data-plant-state-space-dimension="outputs"]', "2");
        await pollValue(
          "2-input 2-output state-space dimensions",
          async () => session.inner("var input=d.querySelector('[data-plant-state-space-dimension=inputs]');var output=d.querySelector('[data-plant-state-space-dimension=outputs]');return JSON.stringify({input:input?.value,output:output?.value});"),
          (value) => value === JSON.stringify({ input: "2", output: "2" }),
          8_000
        );

        const delayValues = JSON.parse(await session.inner("return JSON.stringify([...d.querySelectorAll('[data-plant-input-delay-cell]')].map((field)=>field.value));"));
        if (delayValues.length !== 2 && process.env.TRUST_PLANT_RENDERED_SCREENSHOT) {
          captureRenderedWorkbench(process.env.TRUST_PLANT_RENDERED_SCREENSHOT);
        }
        assert.deepStrictEqual(delayValues, ["0", "0"], "the actual state-space editor must expose a delay in seconds for each input");
        await session.setValue('[data-plant-input-delay-cell="0"]', "0.15");
        await session.setValue('[data-plant-input-delay-cell="1"]', "0.35");
        await session.setValue('[data-plant-state-space-dimension="inputs"]', "3");
        // The resize is the language server's: the third delay shows once it answered.
        await pollValue(
          "a new input receives zero delay",
          async () => JSON.stringify(await readInputDelays(session)),
          (value) => value === JSON.stringify(["0.15", "0.35", "0"]),
          8_000
        );
        // Each resize is the language server's: the matrices, the delays and its question show once
        // it answered.
        const cells = (matrix: string) => session.inner(`return String(d.querySelectorAll('[data-plant-matrix-cell^="${matrix}:"]').length);`);
        await session.setValue('[data-plant-state-space-dimension="states"]', "2");
        await pollValue("two states", () => cells("a"), (value) => value === "4", 8_000);
        assert.deepStrictEqual(await readInputDelays(session), ["0.15", "0.35", "0"], "state dimension changes preserve input delays");
        await session.setValue('[data-plant-state-space-dimension="inputs"]', "2");
        await pollValue(
          "the question before inputs are removed",
          () => session.inner("return String(Boolean(d.querySelector('[data-plant-state-space-resize-apply]')));"),
          (value) => value === "true",
          8_000
        );
        await session.inner("d.querySelector('[data-plant-state-space-resize-apply]').click();return 'RESIZED';");
        await pollValue("two inputs", async () => JSON.stringify(await readInputDelays(session)), (value) => value === JSON.stringify(["0.15", "0.35"]), 8_000);
        await session.setValue('[data-plant-state-space-dimension="inputs"]', "1");
        const confirmation = await pollValue(
          "destructive input resizing must name its delay loss",
          () => session.inner("return d.querySelector('[data-plant-state-space-resize-confirm]')?.textContent||'';"),
          (value) => /nonzero.*delay|delay.*nonzero/i.test(value),
          8_000
        );
        assert.match(confirmation, /nonzero.*delay|delay.*nonzero/i, "destructive input resizing must name its delay loss");
        await session.inner("d.querySelector('[data-plant-state-space-resize-cancel]').click();return 'CANCELLED';");
        assert.deepStrictEqual(await readInputDelays(session), ["0.15", "0.35"], "Cancel preserves both existing delays");

        const ports = await readPorts(session);
        assert.deepStrictEqual(ports.map(({ name, direction, index, label }) => ({ name, direction, index, label })), [
          { name: "plant-command-source", direction: "command", index: "0", label: "Input 1" },
          { name: "plant-command-source-1", direction: "command", index: "1", label: "Input 2" },
          { name: "plant-feedback-target", direction: "feedback", index: "0", label: "Output 1" },
          { name: "plant-feedback-target-1", direction: "feedback", index: "1", label: "Output 2" },
        ], "state space should render one picker for every configured input and output");

        await chooseAddress(session, "plant-command-source", "%QL0");
        await chooseAddress(session, "plant-command-source-1", "%QL8");
        await chooseAddress(session, "plant-feedback-target", "%IL0");
        await chooseAddress(session, "plant-feedback-target-1", "%IL8");
        await pollSave(session.inner, false, "fully bound state-space ports");
        await session.setValue('[data-plant-input-delay-cell="1"]', "-1");
        await pollSave(session.inner, true, "negative input delay");
        // Save is held while the check runs; the delay's problem shows once Rust answered.
        await pollValue(
          "the negative delay's problem",
          () => session.inner("return d.querySelector('[data-plant-input-delays] [data-plant-input-delay-error]')?.textContent||'';"),
          (value) => /0 s or greater|nonnegative/i.test(value),
          8_000
        );
        await session.inner("d.querySelector('[data-plant-validation-summary] button')?.click();return 'FOCUSED';");
        await pollValue("invalid input delay owns focus", () => session.inner("return d.activeElement?.getAttribute('data-plant-input-delay-cell')||'';"), (value) => value === "1", 5_000);
        const delayFieldTops = JSON.parse(await session.inner("return JSON.stringify([...d.querySelectorAll('[data-plant-input-delay-cell]')].map(field=>Math.round(field.getBoundingClientRect().top)));"));
        assert.strictEqual(delayFieldTops[0], delayFieldTops[1], "a field error must keep per-input delay controls aligned");
        if (process.env.TRUST_PLANT_RENDERED_SCREENSHOT) captureRenderedWorkbench(process.env.TRUST_PLANT_RENDERED_SCREENSHOT.replace(/\.png$/, "-invalid-delay.png"));
        await session.setValue('[data-plant-input-delay-cell="1"]', "0.35");
        await session.setValue('[data-plant-matrix-cell="b:0:1"]', "3.25");
        await pollValue(
          "edited second state-space input coefficient",
          async () => session.inner("return d.querySelector('[data-plant-matrix-cell=\"b:0:1\"]')?.value||'';"),
          (value) => value === "3.25",
          8_000
        );

        await openEngineeringDetails(session);
        await session.setValue('[data-plant-engineering-field="input_scale.0"]', "2");
        await session.setValue('[data-plant-engineering-field="output_scale.0"]', "4");
        await session.setValue('[data-plant-engineering-field="step_ms"]', "1");
        await session.setValue('[data-plant-input-delay-cell="0"]', "100");
        await pollSave(session.inner, true, "input delays exceeding the native history bound");
        await pollValue(
          "the delays' history problem",
          () => session.inner("return d.querySelector('[data-plant-input-delays]')?.textContent||'';"),
          (value) => /history/i.test(value),
          8_000
        );
        await session.setValue('[data-plant-input-delay-cell="0"]', "0.15");

        await session.setValue('select[name="plant-feedback-target-1"]', "");
        await pollSave(session.inner, true, "cleared second state-space output");
        const cleared = {
          command0: "%QL0",
          feedback0: "%IL0",
          feedback1: "",
          inputScale0: "2",
          outputScale0: "4",
        };
        // The binding is the language server's; the ports show it once it answered.
        const afterClear = await pollValue(
          "clearing output 2 must retain output 1, input 1, and their conversions",
          () => preservedFirstPortState(session),
          (value) => JSON.stringify(value) === JSON.stringify(cleared),
          8_000
        );
        assert.deepStrictEqual(afterClear, cleared, "clearing output 2 must retain output 1, input 1, and their conversions");

        await chooseAddress(session, "plant-feedback-target-1", "%IL16");
        await pollSave(session.inner, false, "changed second state-space output");
        const afterChange = await preservedFirstPortState(session);
        assert.deepStrictEqual(afterChange, {
          command0: "%QL0",
          feedback0: "%IL0",
          feedback1: "%IL16",
          inputScale0: "2",
          outputScale0: "4",
        }, "changing output 2 must retain neighboring bindings and conversions");
        await captureMultiportEvidence(session, "state_space");

        await saveAndReopenPlant(session);
        const reopenedPorts = await readPorts(session);
        assert.deepStrictEqual(reopenedPorts.map(({ name, value }) => ({ name, value })), [
          { name: "plant-command-source", value: "%QL0" },
          { name: "plant-command-source-1", value: "%QL8" },
          { name: "plant-feedback-target", value: "%IL0" },
          { name: "plant-feedback-target-1", value: "%IL16" },
        ], "saved state-space port bindings must reopen from the persisted runtime configuration");
        const reopenedStateSpace = JSON.parse(await session.inner("var value=(selector)=>d.querySelector(selector)?.value??'';return JSON.stringify({inputs:value('[data-plant-state-space-dimension=inputs]'),outputs:value('[data-plant-state-space-dimension=outputs]'),b01:value('[data-plant-matrix-cell=\"b:0:1\"]')});"));
        assert.deepStrictEqual(reopenedStateSpace, {
          inputs: "2",
          outputs: "2",
          b01: "3.25",
        }, "saved dimensions and the edited second-input B coefficient must reopen from persisted configuration");
        assert.deepStrictEqual(await readInputDelays(session), ["0.15", "0.35"], "edited delays must survive actual Save and reopen");
        if (process.env.TRUST_PLANT_RENDERED_SCREENSHOT) {
          await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
          await session.inner("d.querySelector('[data-plant-state-space-editor]')?.scrollIntoView({block:'start'});return 'DELAYS_VISIBLE';");
          await sleep(250);
          captureRenderedWorkbench(process.env.TRUST_PLANT_RENDERED_SCREENSHOT.replace(/\.png$/, "-saved-delays.png"));
        }
        await openEngineeringDetails(session);
        assert.deepStrictEqual(await preservedFirstPortState(session), {
          command0: "%QL0",
          feedback0: "%IL0",
          feedback1: "%IL16",
          inputScale0: "2",
          outputScale0: "4",
        }, "saved first-port conversions must reopen with the persisted multiport bindings");
      } finally {
        session.close();
      }
    });
  });
}

async function readInputDelays(session: RenderedPlantSession): Promise<string[]> {
  return JSON.parse(await session.inner("return JSON.stringify([...d.querySelectorAll('[data-plant-input-delay-cell]')].map((field)=>field.value));"));
}

async function selectModel(session: RenderedPlantSession, model: string): Promise<void> {
  await session.setValue("select[name=plant-model]", model);
  await pollValue(
    `${model} model selection`,
    async () => session.inner("var select=d.querySelector('select[name=plant-model]');return select?.value||'';"),
    (value) => value === model,
    8_000
  );
}

async function readPorts(session: RenderedPlantSession): Promise<RenderedPort[]> {
  return JSON.parse(await session.inner("var selects=[...d.querySelectorAll('select[name^=plant-command-source],select[name^=plant-feedback-target]')];var text=(value)=>(value||'').replace(/\\s+/g,' ').trim();return JSON.stringify(selects.map((select)=>{var label=select.labels&&select.labels[0];var labelText=label&&label.querySelector('span');return {name:select.name,direction:select.getAttribute('data-plant-port-direction'),index:select.getAttribute('data-plant-port-index'),label:text(labelText?.textContent||label?.textContent),value:select.value};}));"));
}

async function readStartingConditions(session: RenderedPlantSession): Promise<{
  fields: Array<{ index: string | null; label: string; value: string; invalid: string | null }>;
  rawInitialStateInDynamics: boolean;
}> {
  return JSON.parse(await session.inner("var section=d.querySelector('[data-plant-setup-section=starting-conditions]');var text=(value)=>(value||'').replace(/\\s+/g,' ').trim();var fields=section?[...section.querySelectorAll('[data-plant-starting-state]')].map((field)=>({index:field.getAttribute('data-plant-starting-state'),label:text(field.closest('label')?.querySelector('span')?.textContent),value:field.value,invalid:field.getAttribute('aria-invalid')})):[];return JSON.stringify({fields,rawInitialStateInDynamics:Boolean(d.querySelector('[data-plant-dynamics-field=initial_state]'))});"));
}

async function pollStartingFields(
  session: RenderedPlantSession,
  expected: ReadonlyArray<{ index: string; label: string; value: string; invalid: string }>,
  label: string
): Promise<void> {
  const serialized = JSON.stringify(expected);
  await pollValue(
    label,
    async () => JSON.stringify((await readStartingConditions(session)).fields),
    (value) => value === serialized,
    8_000
  );
}

async function confirmStartingPointReplacement(session: RenderedPlantSession): Promise<void> {
  await pollValue(
    "saved model replacement confirmation",
    async () => session.inner("return String(Boolean(d.querySelector('[data-plant-starting-point-confirmation]')));"),
    (value) => value === "true",
    8_000
  );
  const clicked = await session.inner("var dialog=d.querySelector('[data-plant-starting-point-confirmation]');var button=dialog&&[...dialog.querySelectorAll('button')].find((candidate)=>(candidate.textContent||'').trim()==='Replace values');if(!button)return 'NO_BUTTON';button.click();return 'REPLACED';");
  assert.strictEqual(clicked, "REPLACED", "saved-model replacement should use the existing explicit confirmation");
}

async function chooseAddress(
  session: RenderedPlantSession,
  name: string,
  expectedAddress: string
): Promise<void> {
  const selector = `select[name=${JSON.stringify(name)}]`;
  const value = await pollValue(
    `${name} ${expectedAddress} option`,
    async () => session.inner(`var select=d.querySelector(${JSON.stringify(selector)});var option=select&&[...select.options].find((candidate)=>candidate.value.toUpperCase()===${JSON.stringify(expectedAddress)});return option?.value||'';`),
    (candidate) => candidate.toUpperCase() === expectedAddress,
    20_000
  );
  await session.setValue(selector, value);
  await pollValue(
    `${name} selected address`,
    async () => session.inner(`return d.querySelector(${JSON.stringify(selector)})?.value||'';`),
    (candidate) => candidate === value,
    8_000
  );
}

async function preservedFirstPortState(session: RenderedPlantSession): Promise<Record<string, string>> {
  return JSON.parse(await session.inner("var value=(selector)=>d.querySelector(selector)?.value??'';return JSON.stringify({command0:value('select[name=plant-command-source]'),feedback0:value('select[name=plant-feedback-target]'),feedback1:value('select[name=plant-feedback-target-1]'),inputScale0:value('[data-plant-engineering-field=\"input_scale.0\"]'),outputScale0:value('[data-plant-engineering-field=\"output_scale.0\"]')});"));
}

async function openEngineeringDetails(session: RenderedPlantSession): Promise<void> {
  await session.inner("d.querySelector('[data-plant-optional-open=signals]')?.click();return 'OPEN';");
  await pollValue(
    "reopened signal engineering fields",
    async () => session.inner("return String(Boolean(d.querySelector('[data-plant-engineering-field=\"input_scale.0\"]')&&d.querySelector('[data-plant-engineering-field=\"output_scale.0\"]')));"),
    (value) => value === "true",
    8_000
  );
}

async function toggleEngineeringDetails(
  session: RenderedPlantSession,
  open: boolean
): Promise<void> {
  await pollValue(
    `${open ? "open" : "close"} signal conversions`,
    async () => session.inner(`var button=d.querySelector('[data-plant-optional-open=signals]');if(!button||button.getAttribute('aria-expanded')!==${JSON.stringify(String(!open))})return 'WAIT';button.click();return 'CLICKED';`),
    (value) => value === "CLICKED",
    8_000
  );
  await pollValue(
    `signal conversions ${open ? "open" : "closed"} state`,
    async () => session.inner("return d.querySelector('[data-plant-optional-open=signals]')?.getAttribute('aria-expanded')||'';"),
    (value) => value === String(open),
    8_000
  );
}

async function saveAndReopenPlant(session: RenderedPlantSession): Promise<void> {
  const clicked = await session.inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');var save=panel&&[...panel.querySelectorAll('button')].find((candidate)=>(candidate.textContent||'').trim()==='Save');if(!save||save.disabled)return 'NOT_READY';save.click();return 'SAVED';");
  assert.strictEqual(clicked, "SAVED", "the valid multiport plant should be ready for a real save");
  await pollValue(
    "saved plant draft to close",
    async () => session.inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');return panel?'OPEN':'CLOSED';"),
    (value) => value === "CLOSED",
    20_000
  );
  await pollValue(
    "persisted plant node to reopen",
    async () => session.inner("var node=[...d.querySelectorAll('.react-flow__node')].find((candidate)=>(candidate.textContent||'').includes('New simulated plant')&&!(candidate.textContent||'').includes('DRAFT'));if(!node)return 'WAIT';node.dispatchEvent(new node.ownerDocument.defaultView.MouseEvent('click',{bubbles:true}));return 'CLICKED';"),
    (value) => value === "CLICKED",
    20_000
  );
  await pollValue(
    "persisted plant editor",
    async () => session.inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');return (panel?.textContent||'').includes('Configure plant')?'OPEN':'WAIT';"),
    (value) => value === "OPEN",
    20_000
  );
  // The editor shows the saved plant's fields once Rust has opened it.
  await pollValue(
    "persisted plant fields",
    async () => session.inner("return String(Boolean(d.querySelector('select[name=plant-model]')));"),
    (value) => value === "true",
    20_000
  );
}

async function captureMultiportEvidence(
  session: RenderedPlantSession,
  model: "robot_joint" | "state_space"
): Promise<void> {
  const surface = JSON.parse(await session.inner("var panel=d.querySelector('[data-plant-layout=main-canvas]');var rect=panel?.getBoundingClientRect();return JSON.stringify({clientWidth:panel?.clientWidth,scrollWidth:panel?.scrollWidth,overflowX:Boolean(panel&&panel.scrollWidth>panel.clientWidth+1),left:rect&&Math.round(rect.left),right:rect&&Math.round(rect.right),portCount:d.querySelectorAll('select[data-plant-port-direction]').length});"));
  assert.strictEqual(surface.overflowX, false, `${model} multiport editor must not overflow horizontally: ${JSON.stringify(surface)}`);
  const screenshot = process.env.TRUST_PLANT_RENDERED_SCREENSHOT?.trim();
  if (!screenshot) return;
  const captureStartingConditions = model === "robot_joint"
    && process.env.TRUST_PLANT_RENDERED_CAPTURE_SECTION === "starting_conditions";
  const capturePosition = JSON.parse(await session.inner(`var panel=d.querySelector('[data-plant-layout=main-canvas]');var target=d.querySelector('[data-plant-setup-section=${captureStartingConditions ? "starting-conditions" : "connections"}]');if(panel&&target)panel.scrollTop=Math.max(0,target.offsetTop-70);return JSON.stringify({scrollTop:panel?.scrollTop,targetTop:target?.getBoundingClientRect().top,panelTop:panel?.getBoundingClientRect().top});`));
  await sleep(250);
  captureRenderedWorkbench(screenshot);
  const evidencePath = screenshot.toLowerCase().endsWith(".png")
    ? `${screenshot.slice(0, -4)}.json`
    : `${screenshot}.json`;
  fs.writeFileSync(evidencePath, JSON.stringify({
    model,
    split: process.env.TRUST_PLANT_RENDERED_SPLIT === "1",
    surface,
    capturePosition,
    ports: await readPorts(session),
    startingConditions: model === "robot_joint" ? await readStartingConditions(session) : undefined,
    stateSpace: model === "state_space" ? JSON.parse(await session.inner("var value=(selector)=>d.querySelector(selector)?.value??'';return JSON.stringify({inputs:value('[data-plant-state-space-dimension=inputs]'),outputs:value('[data-plant-state-space-dimension=outputs]'),b01:value('[data-plant-matrix-cell=\"b:0:1\"]')});")) : undefined,
  }, null, 2));
}

async function captureStartingConditionsEvidence(
  session: RenderedPlantSession,
  model: "first_order" | "second_order"
): Promise<void> {
  const screenshot = process.env.TRUST_PLANT_RENDERED_SCREENSHOT?.trim();
  if (!screenshot || process.env.TRUST_PLANT_RENDERED_CAPTURE_MODEL !== model) return;
  const surface = JSON.parse(await session.inner("var panel=d.querySelector('[data-plant-layout=main-canvas]');var rect=panel?.getBoundingClientRect();return JSON.stringify({clientWidth:panel?.clientWidth,scrollWidth:panel?.scrollWidth,overflowX:Boolean(panel&&panel.scrollWidth>panel.clientWidth+1),left:rect&&Math.round(rect.left),right:rect&&Math.round(rect.right)});"));
  assert.strictEqual(surface.overflowX, false, `${model} Starting conditions must not overflow horizontally: ${JSON.stringify(surface)}`);
  const capturePosition = JSON.parse(await session.inner("var panel=d.querySelector('[data-plant-layout=main-canvas]');var target=d.querySelector('[data-plant-setup-section=starting-conditions]');if(panel&&target)panel.scrollTop=Math.max(0,target.offsetTop-70);return JSON.stringify({scrollTop:panel?.scrollTop,targetTop:target?.getBoundingClientRect().top,panelTop:panel?.getBoundingClientRect().top});"));
  await sleep(250);
  captureRenderedWorkbench(screenshot);
  const evidencePath = screenshot.toLowerCase().endsWith(".png")
    ? `${screenshot.slice(0, -4)}.json`
    : `${screenshot}.json`;
  fs.writeFileSync(evidencePath, JSON.stringify({
    model,
    split: process.env.TRUST_PLANT_RENDERED_SPLIT === "1",
    surface,
    capturePosition,
    startingConditions: await readStartingConditions(session),
    ports: await readPorts(session),
  }, null, 2));
}
