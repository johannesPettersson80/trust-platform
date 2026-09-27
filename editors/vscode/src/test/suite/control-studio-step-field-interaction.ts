import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { pollValue, type CdpConnection } from "./plant-rendered-harness";

/** Exercise each real settings input through keyboard events and host Check. */
export async function exerciseStepFields(connection: CdpConnection,
  inner: (body: string) => Promise<string>, capture: (name: string) => Promise<void>, captures: string): Promise<void> {
  const targets = await connection.send("Target.getTargets");
  const page = targets.targetInfos.find((target: any) => target.type === "page");
  assert.ok(page, "keyboard input requires the owning workbench page");
  const { sessionId } = await connection.send("Target.attachToTarget", { targetId: page.targetId, flatten: true });
  try { await runStepFields(connection, sessionId, inner, capture, captures); }
  finally { await connection.send("Target.detachFromTarget", { sessionId }); }
}

async function runStepFields(connection: CdpConnection, session: string,
  inner: (body: string) => Promise<string>, capture: (name: string) => Promise<void>, captures: string): Promise<void> {
  const alternatives: Record<string, number> = { baselineDurationSeconds: .9, finalDurationSeconds: .9,
    maximumWindowRange: .2, maximumWindowSlopePerSecond: .02, measurementBound: .06,
    onsetThreshold: .2, onsetSamples: 4, baselineStep: .02, gainStep: .002, timestampShiftSeconds: .1,
    "delay.minimum": .1, "delay.maximum": .3, "delay.step": .2,
    "timeConstant.minimum": .2, "timeConstant.maximum": .5, "timeConstant.step": .15 };
  const rejected: Record<string, string> = Object.fromEntries(Object.keys(alternatives).map(key => [key,
    key === "timestampShiftSeconds" ? "" : "-1"]));
  const failures: string[] = [], observations: unknown[] = [];
  let currentField = "";
  await inner("d.querySelector('[data-control-studio-mode][aria-label^=\"Identify,\"]').click();d.querySelector('[data-step-response-settings]').open=true;var root=d.querySelector('[data-control-studio]');d.defaultView.addEventListener('message',e=>{var m=e.data;if(m?.kind==='control-studio.engineering.result'&&m.stage==='identify'&&m.payload?.phase==='qualified')root.dataset.fieldCheck=JSON.stringify(m.payload);});return 'fields ready';");
  const original = JSON.parse(await inner("return JSON.stringify(Object.fromEntries([...d.querySelectorAll('[data-step-field]')].map(f=>[f.dataset.stepField,f.value])));"));
  assert.deepStrictEqual(Object.keys(original).sort(), Object.keys(alternatives).sort());
  const key = async (key: string, code: string, virtual: number, modifiers = 0) => {
    await connection.send("Input.dispatchKeyEvent", { type: "keyDown", key, code, windowsVirtualKeyCode: virtual, modifiers,
      ...(key === "Enter" ? { text: "\r", unmodifiedText: "\r" } : {}) }, session);
    await connection.send("Input.dispatchKeyEvent", { type: "keyUp", key, code, windowsVirtualKeyCode: virtual, modifiers }, session);
  };
  const enter = async (field: string, text: string) => {
    await inner(`d.querySelector('[data-step-field="${field}"]').focus();return 'focused';`);
    await key("a", "KeyA", 65, 2);
    if (text) await connection.send("Input.insertText", { text }, session);
    else await key("Backspace", "Backspace", 8);
    assert.strictEqual(await inner(`return d.querySelector('[data-step-field="${field}"]').value;`), text,
      `${field} must accept the actual keyboard input`);
  };
  const check = async () => {
    await inner("delete d.querySelector('[data-control-studio]').dataset.fieldCheck;return 'await keyboard check';");
    for (let move = 0; move < 40 && await inner("return String(d.activeElement===d.querySelector('[data-identify-run]'));") !== "true"; move++) {
      await key("Tab", "Tab", 9);
    }
    const immediate = await inner("return String(d.activeElement===d.querySelector('[data-identify-run]'));");
    await new Promise(resolve => setTimeout(resolve, 50));
    const focused = await inner("return String(d.activeElement===d.querySelector('[data-identify-run]'));");
    if (focused !== "true") {
      const diagnostic = JSON.parse(await inner("var b=d.querySelector('[data-identify-run]');return JSON.stringify({active:d.activeElement?.tagName,field:d.activeElement?.dataset.stepField,bodyFocused:d.hasFocus(),hidden:b.hidden,disabled:b.disabled,status:d.querySelector('[data-identify-state]').textContent,rect:b.getBoundingClientRect().toJSON()});"));
      fs.writeFileSync(path.join(captures, "field-focus-diagnostic.json"), JSON.stringify({ field: currentField, immediate, diagnostic }, null, 2));
      await capture("field-focus-diagnostic");
    }
    assert.strictEqual(focused, "true", "Check must retain keyboard focus while the current edit is presented");
    await key("Enter", "Enter", 13);
  };
  for (const [field, value] of Object.entries(alternatives)) {
    currentField = field;
    await enter(field, String(value)); await check();
    let checked: any;
    try {
      checked = JSON.parse(await pollValue(`host checked ${field}`, () => inner("return d.querySelector('[data-control-studio]').dataset.fieldCheck??'';"), v => Boolean(v), 10000));
    } catch (error) {
      fs.writeFileSync(path.join(captures, "field-check-diagnostic.json"), await inner("return JSON.stringify({active:d.activeElement?.tagName,status:d.querySelector('[data-identify-state]').textContent,check:d.querySelector('[data-identify-run]').outerHTML,held:d.querySelector('[data-step-response-enabled]').checked});"));
      await capture("field-check-diagnostic"); throw error;
    }
    assert.strictEqual(checked.status, "complete", `${field} valid alternative must qualify`);
    const [group, member] = field.split(".");
    const settings = checked.qualification.stepResponse.settings;
    assert.strictEqual(member ? settings[group][member] : settings[group], value, `${field} must reach the host calculation`);
    await enter(field, rejected[field]); await check();
    await pollValue(`invalid ${field} rejected`, () => inner("return d.querySelector('[data-identify-state]').textContent;"), v => /Review|Enter/.test(v), 10000);
    const review = JSON.parse(await inner(`var f=d.querySelector('[data-step-field="${field}"]');var label=d.querySelector('[data-step-label="${field}"]').textContent;var error=d.getElementById(f.getAttribute('aria-describedby'));var r=f.getBoundingClientRect();var w=f.closest('.workspace').getBoundingClientRect();return JSON.stringify({label,invalid:f.getAttribute('aria-invalid'),focused:d.activeElement===f,error:error?.textContent??'',errorVisible:Boolean(error&&!error.hidden),visible:r.top>=w.top&&r.bottom<=w.bottom,status:d.querySelector('[data-identify-state]').textContent});`));
    if (review.invalid !== "true" || !review.focused || !review.errorVisible || !review.visible || !review.status.toLowerCase().includes(review.label.toLowerCase())) {
      failures.push(`${field}: invalid input needs its labelled, focused, visible inline error`);
    }
    observations.push({ field, accepted: value, rejected: rejected[field], review });
    await capture(`field-${field.replace(/\./g, "-")}-invalid`);
    await enter(field, original[field]); await check();
    const restored = JSON.parse(await pollValue(`corrected ${field}`, () => inner("return d.querySelector('[data-control-studio]').dataset.fieldCheck??'';"), v => Boolean(v), 10000));
    assert.strictEqual(restored.status, "complete");
    assert.notStrictEqual(await inner(`return d.querySelector('[data-step-field="${field}"]').getAttribute('aria-invalid')??'';`), "true");
  }
  fs.writeFileSync(path.join(captures, "field-interactions.json"), JSON.stringify(observations, null, 2));
  assert.deepStrictEqual(failures, [], "all sixteen settings must give actionable accessible correction");
}
