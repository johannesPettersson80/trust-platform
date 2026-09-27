import * as assert from "assert";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";

/** Read-only view traversal; operation buttons are never invoked. */
export async function captureControlStudioViews(inner: EvaluateWebview, capture: (suffix: string) => Promise<void>, checkpoint: string,
  stages: readonly string[] = ["model", "tune", "verify", "apply"], advanced = false): Promise<void> {
  const before = JSON.parse(await inner("const s=d.querySelector('[data-engineer-workflow]');return JSON.stringify({stage:s.dataset.selectedStage,options:s.querySelector('[data-engineer-options]').open,details:s.querySelector('[data-engineer-results]').open,values:[...s.querySelectorAll('input,select,textarea')].map(e=>e.value)});"));
  for (const stage of stages) {
    await inner(`d.querySelector('[data-engineer-stage-tab=${stage}]').click();d.querySelector('[data-engineer-options]').open=false;d.querySelector('[data-engineer-results]').open=false;return 'view';`);
    await capture(`-gallery-${checkpoint}-${stage}`);
    if (stage === "model") assert.strictEqual(await inner("const footerBounds=d.querySelector('.values').getBoundingClientRect();return String([...d.querySelectorAll('[data-plot-view=general-model] canvas')].every(e=>e.getBoundingClientRect().height===0||e.getBoundingClientRect().bottom<=footerBounds.top));"), "true", "Model charts must fit above the live-value footer");
    assert.strictEqual(await inner("const a=d.querySelector('[data-engineer-actions]').getBoundingClientRect(),s=d.querySelector('.engineer-secondary').getBoundingClientRect();return String(a.right<=s.left||s.right<=a.left||a.bottom<=s.top||s.bottom<=a.top);"), "true", "Settings and Details must not overlap the stage actions");
    await inner("d.querySelector('[data-engineer-options]').open=true;return 'settings';");
    assert.strictEqual(await inner(`return String([...d.querySelectorAll('[data-engineer-settings-stage]')].every(e=>e.dataset.engineerSettingsStage==='${stage}'||e.getBoundingClientRect().height===0));`), "true", "Settings must contain only the selected stage's controls");
    await capture(`-gallery-${checkpoint}-${stage}-settings`);
    await inner("d.querySelector('[data-engineer-options]').open=false;d.querySelector('[data-engineer-results]').open=true;return 'details';");
    assert.strictEqual(await inner(`return String([...d.querySelectorAll('[data-engineer-evidence-stage]')].every(e=>e.dataset.engineerEvidenceStage==='${stage}'||e.getBoundingClientRect().height===0));`), "true", "Details must contain only the selected stage's evidence");
    await capture(`-gallery-${checkpoint}-${stage}-details`);
  }
  if (advanced) {
    await inner("d.querySelector('[data-engineer-results]').open=false;const b=d.querySelector('[data-engineer-tools]');for(let p=b.closest('details');p;p=p.parentElement?.closest('details'))p.open=true;b.click();return 'advanced tools';");
    for (const mode of ["Monitor", "Test", "Identify", "Tune", "Validate"]) {
      await inner(`[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='${mode}').click();return 'tool';`);
      assert.strictEqual(await inner(`return String(!d.querySelector('[data-workflow-pane=${mode}] > h2'));`), "true", "Manual tools must not repeat the navigation heading");
      await capture(`-gallery-${checkpoint}-advanced-${mode.toLowerCase()}`);
    }
    // Component-only proof: this dialog has no launch action in the configured-loop workflow.
    await inner("d.querySelector('[data-auto-tune-plan]').hidden=false;return 'dialog component';");
    await capture(`-gallery-${checkpoint}-automatic-dialog-component`);
    await inner("d.querySelector('[data-auto-tune-plan]').hidden=true;d.querySelector('[data-engineer-return]').click();return 'workflow';");
  }
  await inner(`d.querySelector('[data-engineer-stage-tab=${before.stage}]').click();d.querySelector('[data-engineer-options]').open=${before.options};d.querySelector('[data-engineer-results]').open=${before.details};return 'restore view';`);
  assert.deepStrictEqual(JSON.parse(await inner("return JSON.stringify([...d.querySelector('[data-engineer-workflow]').querySelectorAll('input,select,textarea')].map(e=>e.value));")), before.values,
    "Reviewing pages must not change engineering inputs");
}
