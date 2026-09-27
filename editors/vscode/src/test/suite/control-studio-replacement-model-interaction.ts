import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { pollValue, type CdpConnection } from "./plant-rendered-harness";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";

export async function exerciseReplacementModel(connection: CdpConnection, session: string, inner: EvaluateWebview,
  project: string, capture: (name: string) => Promise<void>): Promise<void> {
  const model = { kind: "transfer-function", numerator: [2], denominator: [3, 1], delaySeconds: 0,
    source: "provided", validation: "unvalidated", evidence: [], revision: "replacement-unvalidated",
    inputs: [{ id: "Main.MV", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
    outputs: [{ id: "Main.PV", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
  const file = path.join(project, "replacement-model.json");
  fs.writeFileSync(file, JSON.stringify(model));
  await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'Model';");
  await connection.send("Page.setInterceptFileChooserDialog", { enabled: true }, session);
  try {
    assert.strictEqual(await inner("const b=d.querySelector('[data-engineer-supply]');const r=b.getBoundingClientRect();return String(!b.disabled&&r.width>0&&r.height>0);"), "true");
    await inner("d.querySelector('[data-engineer-supply]').click();return 'Import model';");
    await connection.send("DOM.enable", {}, session);
    await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session);
    const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
    const node = tree.nodes.find((entry: any) => entry.nodeName === "INPUT" && entry.attributes?.includes("general-model-file"));
    assert.ok(node);
    await connection.send("DOM.setFileInputFiles", { files: [file], backendNodeId: node.backendNodeId }, session);
  } finally { await connection.send("Page.setInterceptFileChooserDialog", { enabled: false }, session); }
  await pollValue("replacement needs validation", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /independent validation/i.test(value), 10000);
  await capture("12-replacement-model");
  assert.strictEqual(await inner("return String(d.querySelector('[data-plot-view=fit]').hidden);"), "true",
    "The previous measured fit must disappear when a different model is imported");
  await pollValue("replacement frequency plot", () => inner("return String([...d.querySelectorAll('[data-plot-view=general-model] canvas')].some(c=>c.getBoundingClientRect().height>50));"), value => value === "true", 10000);
  assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "true",
    "Inspecting an unvalidated model must not qualify a controller");
  await capture("13-replacement-frequency");
  assert.strictEqual(await inner("return String(d.querySelector('[data-general-model-save]').disabled);"), "true",
    "An automatic preview is not a completed host-owned analysis to save");
  await inner("const b=d.querySelector('[data-engineer-tools]');const parents=[];for(let p=b.parentElement;p;p=p.parentElement)if(p.tagName==='DETAILS')parents.push(p);for(const p of parents.reverse())if(!p.open)p.querySelector(':scope > summary').click();b.click();return 'advanced tools';");
  await inner("const b=[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Tune');if(b.getBoundingClientRect().width===0)throw Error('Tune tool is hidden');b.click();return 'manual Tune';");
  await inner("const b=d.querySelector('[data-general-model-analyze]');const parents=[];for(let p=b.parentElement;p;p=p.parentElement)if(p.tagName==='DETAILS')parents.push(p);for(const p of parents.reverse())if(!p.open)p.querySelector(':scope > summary').click();d.defaultView.addEventListener('trust-control-studio-general-model',e=>d.defaultView.__replacementAnalysis=e.detail);return 'analysis controls';");
  await inner("for(const [id,value] of [['general-model-min','0.2'],['general-model-max','20'],['general-model-points','64']]){const e=d.getElementById(id);e.value=value;e.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));}const b=d.querySelector('[data-general-model-analyze]');if(b.getBoundingClientRect().width===0)throw Error('Analyze is hidden');b.click();return 'analyze';");
  await pollValue("chosen frequency grid analyzed", () => inner("return String(d.defaultView.__replacementAnalysis?.saved?.grid?.points);"), value => value === "64", 10000);
  await inner("const b=d.querySelector('[data-general-model-save]');if(b.disabled||b.getBoundingClientRect().width===0)throw Error('Save is unavailable');b.click();return 'save';");
  await pollValue("model analysis saved", () => inner("return d.querySelector('[data-general-model-status]').textContent;"), value => /Saved revision/.test(value), 10000);
  await capture("14-saved-model-analysis");
  await inner("d.querySelector('[data-engineer-return]').click();return 'return to Model';");
  await capture("15-model-frequency-after-save");
}
