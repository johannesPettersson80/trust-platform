import * as assert from "assert";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";
import { pollValue } from "./plant-rendered-harness";

export async function exerciseRequiredGoals(inner: EvaluateWebview,
  capture: (name: string) => Promise<void>): Promise<void> {
  assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-goal-fields]').disabled);"), "false");
  assert.match(await inner("return d.querySelector('[data-engineer-operating-point]').textContent;"), /Starting MV 0 V; PV 0 m; absolute target 1 m/);
  for (const name of ["reference", "error", "settling"]) {
    const value = await inner(`return d.querySelector('[data-engineer-${name}]').value;`);
    await inner(`d.querySelector('[data-engineer-${name}]').value='';d.querySelector('[data-engineer-tune]').click();return 'invalid goal';`);
    assert.strictEqual(await inner(`return String(d.querySelector('[data-engineer-${name}]').validity.valueMissing);`), "true");
    assert.match(await inner("return d.querySelector('[data-engineer-status]').textContent;"), /Correct the highlighted value/);
    await capture(`invalid-goal-${name}`);
    await inner(`d.querySelector('[data-engineer-${name}]').value=${JSON.stringify(value)};return 'restored';`);
  }
}

export async function exerciseMissingModelNavigation(inner: EvaluateWebview,
  capture: (name: string) => Promise<void>): Promise<void> {
  for (const stage of ["tune", "verify", "apply"]) {
    await inner(`d.querySelector('[data-engineer-stage-tab=${stage}]').click();return 'stage';`);
    await pollValue(`missing model action in ${stage}`, () => inner("const b=d.querySelector('[data-engineer-prerequisite-action]');const r=b?.getBoundingClientRect();return String(Boolean(b&&!b.disabled&&r?.width&&r.height));"), value => value === "true", 3000);
    assert.strictEqual(await inner("const b=d.querySelector('[data-engineer-prerequisite-action]');const r=b.getBoundingClientRect();return String(!b.disabled&&r.width>0&&r.height>0);"), "true");
    assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-goal-fields]').disabled);"), "true");
    assert.strictEqual(await inner("return d.querySelector('[data-engineer-prerequisite-action]').dataset.targetStage;"), "model", "go straight to the earliest missing prerequisite");
    await capture(`prerequisite-${stage}-needs-model`);
    await inner("d.querySelector('[data-engineer-prerequisite-action]').click();return 'prerequisite';");
    assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab][aria-selected=true]').dataset.engineerStageTab;"), "model");
  }

}

export async function exerciseStageInspectors(inner: EvaluateWebview,
  capture: (name: string) => Promise<void>): Promise<void> {
  const values = () => inner("return JSON.stringify([...d.querySelectorAll('[data-engineer-workflow] input,[data-engineer-workflow] select')].map(e=>[e.name,e.value,e.checked]));");
  const initial = await values();
  const click = async (selector: string) => {
    assert.strictEqual(await inner(`const b=d.querySelector(${JSON.stringify(selector)});const r=b?.getBoundingClientRect();
      if(!b||b.disabled||!r?.width||!r.height)return 'unavailable';b.click();return 'clicked';`), "clicked", selector);
  };
  for (const stage of ["model", "tune", "verify", "apply"]) {
    await click(`[data-engineer-stage-tab=${stage}]`);
    assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab][aria-selected=true]')?.dataset.engineerStageTab;"), stage);
    if (stage === "verify") {
      assert.strictEqual(await inner("const toolbar=d.querySelector('.monitor > .plot-toolbar');return String([...toolbar.querySelectorAll('button')].every(b=>b.getBoundingClientRect().right<=Math.min(d.defaultView.innerWidth,toolbar.parentElement.getBoundingClientRect().right)));"), "true", "recorded plot toolbar actions must remain inside the webview");
      await pollValue("commissioning chart roles", () => inner("return d.querySelector('#engineer-recorded-commissioning')?.getAttribute('aria-label')??'';"), value => value.includes("upper m lane") && value.includes("lower V lane"), 3000);
      await capture("navigation-commissioning-curves");
      for (const title of ["Record details", "Accessible recorded values"]) {
        assert.strictEqual(await inner(`const s=[...d.querySelectorAll('[data-engineer-recorded-plot] summary')].find(e=>e.textContent===${JSON.stringify(title)});if(!s)return 'missing';s.click();return String(s.parentElement.open);`), "true");
        await capture(`navigation-${title.toLowerCase().replace(/ /g, "-")}`);
      }
      assert.ok(Number(await inner("return String(d.querySelectorAll('[data-engineer-recorded-values] tbody tr').length);")) > 10);
      for (const source of ["verification", "commissioning"]) {
        await inner(`const s=d.querySelector('[data-engineer-recorded-source]');s.value='${source}';s.dispatchEvent(new Event('change'));return 'selected';`);
        assert.strictEqual(await inner("return d.querySelector('[data-engineer-recorded-source]').value;"), source);
        await capture(`navigation-recording-${source}`);
      }
    }
    for (const [name, attribute] of [["settings", "options"], ["details", "results"]]) {
      const selector = `[data-engineer-${attribute}]`;
      await click(`${selector} > summary`);
      await pollValue(`${stage} ${name} inspector`, () => inner(`return d.querySelector('[data-control-studio]').dataset.inspectorOpen;`), value => value === name, 3000);
      assert.strictEqual(await inner(`return d.querySelector('${selector} .engineer-inspector-header strong').textContent.toLowerCase();`), `${stage} ${name}`);
      assert.strictEqual(await inner("return JSON.stringify({open:[...d.querySelectorAll('[data-engineer-options],[data-engineer-results]')].filter(e=>e.open).length,inspector:d.querySelector('[data-control-studio]').dataset.inspectorOpen,plot:getComputedStyle(d.querySelector('.plot-column')).visibility});"),
        JSON.stringify({ open: 1, inspector: name, plot: "visible" }),
        "the approved side inspector keeps the plot visible and exposes exactly one inspector");
      await capture(`navigation-${stage}-${name}`);
      await click(`[data-engineer-inspector-close=${name}]`);
      assert.strictEqual(await inner(`return String(d.activeElement===d.querySelector('${selector} > summary'));`), "true", "Close returns focus to its launcher");
    }
    assert.strictEqual(await values(), initial, "navigation and inspectors preserve all editable values");
  }
  await click("[data-engineer-stage-tab=tune]");
  await click("[data-engineer-options] > summary");
  await click("[data-engineer-stage-tab=model]");
  await pollValue("open inspector follows selected stage", () => inner("return d.querySelector('[data-engineer-options] .engineer-inspector-header strong').textContent;"), value => value === "Model settings", 3000);
  await capture("navigation-open-inspector-follows-model");
  await inner("d.querySelector('[data-engineer-inspector-close=settings]').focus();d.activeElement.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));return 'escape';");
  assert.strictEqual(await inner("return String(!d.querySelector('[data-engineer-options]').open&&d.activeElement===d.querySelector('[data-engineer-options] > summary'));"), "true", "Escape closes the inspector and restores focus");
  await capture("navigation-escape-closed");

  await click("[data-engineer-stage-tab=model]");
  const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
  const originalHandle = EngineerWorkflowSession.prototype.handle;
  const operationKinds: string[] = [];
  EngineerWorkflowSession.prototype.handle = function(message: any, ...args: any[]) {
    if (typeof message?.kind === "string" && message.kind.startsWith("control-studio.engineer.")) operationKinds.push(message.kind);
    return originalHandle.call(this, message, ...args);
  };
  try {
  const keyCases = [["ArrowRight", "tune"], ["ArrowLeft", "model"], ["End", "apply"], ["Home", "model"]] as const;
  for (const [key, expected] of keyCases) {
    const state = JSON.parse(await inner(`const tab=d.querySelector('[data-engineer-stage-tab][aria-selected=true]');
      tab.focus();tab.dispatchEvent(new KeyboardEvent('keydown',{key:${JSON.stringify(key)},bubbles:true}));
      const selected=d.querySelector('[data-engineer-stage-tab][aria-selected=true]');
      return JSON.stringify({stage:selected?.dataset.engineerStageTab,focused:d.activeElement?.dataset.engineerStageTab});`));
    assert.deepStrictEqual(state, { stage: expected, focused: expected }, `${key} must move and focus the guided stage tab`);
  }
  await new Promise(resolve => setTimeout(resolve, 150));
  assert.deepStrictEqual(operationKinds, [], "keyboard navigation must not dispatch an engineer operation");
  } finally { EngineerWorkflowSession.prototype.handle = originalHandle; }


  await click("[data-engineer-results] > summary");
  assert.strictEqual(await inner("return JSON.stringify({open:[...d.querySelectorAll('[data-engineer-options],[data-engineer-results]')].filter(e=>e.open).length,inspector:d.querySelector('[data-control-studio]').dataset.inspectorOpen,plot:getComputedStyle(d.querySelector('.plot-column')).visibility});"),
    JSON.stringify({ open: 1, inspector: "details", plot: "visible" }),
    "the approved side inspector keeps the plot visible while exposing exactly one details inspector");
  await inner("const close=d.querySelector('[data-engineer-inspector-close=details]');close.focus();close.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));return 'escape';");
  await capture("navigation-details-escape-closed");
  assert.strictEqual(await inner("return String(!d.querySelector('[data-engineer-results]').open&&d.activeElement===d.querySelector('[data-engineer-results] > summary'));"), "true", "Escape closes the details inspector and restores focus");
}
