import * as assert from "assert";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";
import { pollValue } from "./plant-rendered-harness";

type Capture = (name: string) => Promise<void>;

/** Exercises the visible Automatic Tune plan and its unapplied review boundary. */
export async function exerciseAutomaticTuneInteractions(inner: EvaluateWebview, click: (selector: string) => Promise<void>, capture: Capture): Promise<void> {
  await click("[data-control-studio-automatic-tune]");
  assert.strictEqual(await inner("return String(!d.querySelector('[data-auto-tune-plan]').hidden);"), "true", "Automatic Tune opens a visible plan");
  assert.strictEqual(await inner("return String(d.querySelector('[data-auto-tune-objective]').getBoundingClientRect().height>0);"), "true", "Automatic Tune exposes its response objective before approval");
  await inner("const select=d.querySelector('[data-auto-tune-objective]');select.value='robust';select.dispatchEvent(new Event('change',{bubbles:true}));return 'robust';");
  assert.strictEqual(await inner("return d.querySelector('[data-auto-tune-objective]').value;"), "robust", "the selected response objective remains visible");
  assert.match(await inner("return d.querySelector('[data-auto-tune-source]').textContent;"), /Simulation|saved model/i, "Automatic Tune identifies its evidence source before approval");
  assert.deepStrictEqual(JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-auto-tune-stage]')].map(e=>e.dataset.status));")), ["waiting", "waiting", "waiting", "waiting"], "Automatic Tune starts with explicit waiting stages");
  await capture("automatic-tune-plan");
  await click("[data-auto-tune-approve]");
  await pollValue("Automatic Tune result", () => inner("return JSON.stringify({hidden:d.querySelector('[data-auto-tune-result]').hidden,error:d.querySelector('[data-auto-tune-error]').textContent,stages:[...d.querySelectorAll('[data-auto-tune-stage]')].map(e=>e.dataset.status)});"), value => {
    try { const result=JSON.parse(value); return result.hidden === false || result.stages.includes("failed"); } catch { return false; }
  }, 30_000);
  const state = JSON.parse(await inner("return JSON.stringify({hidden:d.querySelector('[data-auto-tune-result]').hidden,error:d.querySelector('[data-auto-tune-error]').textContent,source:d.querySelector('[data-auto-tune-source]').textContent,structure:d.querySelector('[data-auto-tune-structure]').textContent,method:d.querySelector('[data-auto-tune-method]').textContent,model:d.querySelector('[data-auto-tune-model]').textContent,validation:d.querySelector('[data-auto-tune-validation]').textContent,apply:{text:d.querySelector('[data-auto-tune-apply]').textContent,disabled:d.querySelector('[data-auto-tune-apply]').disabled}});"));
  assert.strictEqual(state.error, "", `Automatic Tune failed: ${state.error}`);
  assert.strictEqual(state.hidden, false, "Automatic Tune exposes its completed result");
  assert.match(state.source, /Simulation|saved model/i);
  assert.ok(state.structure && state.method && state.model && state.validation, "Automatic Tune result identifies structure, method, model and validation");
  assert.deepStrictEqual([...new Set(JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-auto-tune-stage]')].map(e=>e.dataset.status));")))], ["complete"]);
  assert.deepStrictEqual(state.apply, { text: "Review application", disabled: false }, "Automatic Tune offers review before application");
  await capture("automatic-tune-result");
  const messages=require("../../controlStudio/messages");
  const decode=messages.decodeControllerApplicationCommand;
  let applicationCommands=0;
  messages.decodeControllerApplicationCommand=(...args: unknown[])=>{const command=decode(...args);if(command)applicationCommands++;return command;};
  try {
  await inner("d.defaultView.__automaticApplicationMessages=0;d.defaultView.addEventListener('message',e=>{if(e.data?.kind==='control-studio.application.result')d.defaultView.__automaticApplicationMessages++;});return 'listen';");
  await click("[data-auto-tune-apply]");
  assert.strictEqual(await inner("return String(d.querySelector('[data-auto-tune-plan]').hidden);"), "true", "Review application closes the Automatic Tune plan");
  assert.strictEqual(await inner("return String(d.defaultView.__automaticApplicationMessages);"), "0", "Review application does not apply gains implicitly");
  assert.strictEqual(await inner("return String(d.querySelector('[data-shared-application-review]').hidden);"), "false");
  assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-shared-application-close]')));"), "true", "the unapplied review needs a visible way to return without applying");
  await capture("automatic-application-review");
  assert.strictEqual(await inner("const close=d.querySelector('[data-shared-application-close]').getBoundingClientRect();const viewport=d.querySelector('.workspace').getBoundingClientRect();return String(close.top>=viewport.top&&close.bottom<=viewport.bottom);"), "true", "review dismissal must be visible immediately, without scrolling past the settings");
  await click("[data-shared-application-close]");
  assert.strictEqual(await inner("return String(d.querySelector('[data-shared-application-review]').hidden);"), "true");
  assert.strictEqual(await inner("return String(d.defaultView.__automaticApplicationMessages);"), "0");
  assert.strictEqual(await inner("return String(d.activeElement===d.querySelector('[data-control-studio-automatic-tune]'));"),"true","Close returns focus to the visible launcher");
  await click("[data-control-studio-automatic-tune]");
  await click("[data-auto-tune-apply]");
  await inner("const close=d.querySelector('[data-shared-application-close]');close.focus();close.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));return 'escape';");
  assert.strictEqual(await inner("return String(d.querySelector('[data-shared-application-review]').hidden);"),"true");
  await capture("automatic-application-dismissed");
  assert.strictEqual(applicationCommands,0,"review, Close and Escape never dispatch an application or rollback command");
  await click("[data-control-studio-automatic-tune]");
  } finally { messages.decodeControllerApplicationCommand=decode; }
}
