import * as assert from "assert";

export type EvaluateWebview = (body: string) => Promise<string>;
export type StageTab = { stage: string | null; label: string | undefined; selected: string | null; disabled: boolean };

export async function stageTabs(inner: EvaluateWebview): Promise<StageTab[]> {
  return JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-engineer-stage-tab]')].map(tab=>({stage:tab.getAttribute('data-engineer-stage-tab'),label:tab.textContent?.trim(),selected:tab.getAttribute('aria-selected'),disabled:tab.disabled})));"));
}

export async function assertInitialStageTabs(inner: EvaluateWebview): Promise<void> {
  assert.deepStrictEqual(await stageTabs(inner), [
    { stage: "model", label: "Model", selected: "true", disabled: false },
    { stage: "tune", label: "Tune", selected: "false", disabled: false },
    { stage: "verify", label: "Verify", selected: "false", disabled: false },
    { stage: "apply", label: "Apply", selected: "false", disabled: false },
  ], "the compact engineer workflow must keep Model, Tune, Verify, and Apply as four always-visible navigable tabs");
}

export async function assertModelNavigationPreservesSettings(inner: EvaluateWebview, settings: unknown): Promise<void> {
  await inner("d.querySelector('[data-engineer-stage-tab=model]')?.click();return 'model navigation';");
  assert.deepStrictEqual((await stageTabs(inner)).map(tab => tab.selected), ["true", "false", "false", "false"],
    "clicking Model from Tune must select Model without changing the workflow state");
  assert.deepStrictEqual(JSON.parse(await inner("return JSON.stringify({text:d.querySelector('[data-engineer-find]')?.textContent?.trim(),disabled:d.querySelector('[data-engineer-find]')?.disabled,model:d.querySelector('[data-general-model-provenance]')?.textContent??''});")),
    { text: "Identify again", disabled: false, model: (settings as any).model },
    "an existing model must expose an enabled Find model again action after returning to Model");
  assert.deepStrictEqual(JSON.parse(await inner("return JSON.stringify({reference:d.querySelector('[data-engineer-reference]')?.value,error:d.querySelector('[data-engineer-error]')?.value,band:d.querySelector('[data-engineer-band]')?.value,settling:d.querySelector('[data-engineer-settling]')?.value,duration:d.querySelector('[data-engineer-duration]')?.value,model:d.querySelector('[data-general-model-provenance]')?.textContent??''});")), settings,
    "stage navigation must preserve the selected model and task settings");
  await inner("d.querySelector('[data-engineer-stage-tab=tune]')?.click();return 'tune navigation';");
}

export async function assertVerifiedNavigationPreservesEvidence(inner: EvaluateWebview, recorded: string): Promise<void> {
  await inner("d.querySelector('[data-engineer-stage-tab=model]')?.click();return 'model navigation after verify';");
  assert.deepStrictEqual(JSON.parse(await inner("return JSON.stringify({text:d.querySelector('[data-engineer-find]')?.textContent?.trim(),disabled:d.querySelector('[data-engineer-find]')?.disabled});")),
    { text: "Identify again", disabled: false },
    "Model navigation must keep the existing model action enabled after Verify");
  assert.strictEqual(await inner("return JSON.stringify({settings:{reference:d.querySelector('[data-engineer-reference]')?.value,error:d.querySelector('[data-engineer-error]')?.value,band:d.querySelector('[data-engineer-band]')?.value,settling:d.querySelector('[data-engineer-settling]')?.value,duration:d.querySelector('[data-engineer-duration]')?.value},evidence:d.defaultView.__engineerLatest?.verificationAttempts,plots:d.querySelectorAll('[data-engineer-recorded-plot]').length})"), recorded,
    "navigation must preserve recorded verification results and engineering settings");
  await inner("d.querySelector('[data-engineer-stage-tab=apply]')?.click();return 'apply navigation after model';");
}

export async function assertCleanHierarchy(inner: EvaluateWebview): Promise<void> {
  assert.strictEqual(await inner("d.defaultView.dispatchEvent(new d.defaultView.CustomEvent('trust-control-studio-stage',{detail:{stage:'Monitor'}}));return d.querySelector('[data-control-plot-tabs]').getAttribute('aria-label');"), "Plots for model", "Legacy plot events must not replace the guided Model view");
  const layout = JSON.parse(await inner("const h=d.querySelector('.header'),v=d.querySelector('.values'),p=d.querySelector('.plot-column');return JSON.stringify({headerStatus:!!h.querySelector('.status'),footerStatus:!!v.querySelector('.status'),belowPlot:v.getBoundingClientRect().top>=p.getBoundingClientRect().bottom-1});"));
  assert.strictEqual(layout.headerStatus, false, "Runtime status must not compete with navigation in the header");
  assert.strictEqual(layout.footerStatus, true, "Runtime state and SP/PV/MV belong in one live-status strip");
  assert.strictEqual(layout.belowPlot, true, "The live-status strip belongs below the working plot");
}
