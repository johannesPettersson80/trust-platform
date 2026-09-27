import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";
import { pollValue, type CdpConnection } from "./plant-rendered-harness";

export async function exerciseImportedTuning(connection: CdpConnection, session: string,
  inner: EvaluateWebview, project: string, click: (selector: string) => Promise<void>,
  capture: (name: string) => Promise<void>): Promise<void> {
  const settings = JSON.parse(await inner("return JSON.stringify(d.defaultView.__legacyTune.candidate.parameters);"));
  settings.proportionalGain = 3.25;
  settings.integralGain = 0.15;
  settings.trackingTime = 2;
  settings.antiWindup = "Combined";
  const invalid = path.join(project,"invalid-controller-settings.json");
  const valid = path.join(project,"reviewed-controller-settings.json");
  fs.writeFileSync(invalid,"[]"); fs.writeFileSync(valid,JSON.stringify(settings));
  await inner("const method=d.querySelector('#tune-method');method.value='Imported';method.dispatchEvent(new Event('change'));return 'imported';");
  const upload = async (file: string) => {
    await connection.send("DOM.enable",{},session);
    await connection.send("DOM.getDocument",{depth:-1,pierce:true},session);
    const tree=await connection.send("DOM.getFlattenedDocument",{depth:-1,pierce:true},session);
    const node=tree.nodes.find((entry: any)=>entry.nodeName==='INPUT'&&entry.attributes?.includes('tune-import-file'));
    assert.ok(node);
    await connection.send("DOM.setFileInputFiles",{files:[file],backendNodeId:node.backendNodeId},session);
  };
  await upload(invalid);
  await pollValue("invalid controller file",()=>inner("return d.querySelector('[data-tune-import-state]').textContent;"),value=>value.includes("Invalid JSON"),5000);
  await capture("invalid-controller-settings");
  await upload(valid);
  await pollValue("controller settings imported",()=>inner("return d.querySelector('[data-tune-import-state]').textContent;"),value=>value.includes("reviewed-controller-settings.json ready"),5000);
  await inner("d.defaultView.__legacyTune=undefined;return 'next tuning';");
  await click("[data-tune-generate]");
  const result=JSON.parse(await pollValue("imported tuning result",()=>inner("return JSON.stringify(d.defaultView.__legacyTune??{});"),value=>["complete","failed"].includes(JSON.parse(value).status),15000));
  assert.strictEqual(result.status,"complete",result.reason);
  assert.strictEqual(result.candidate.source,"Imported");
  for(const key of ["proportionalGain","integralGain","trackingTime","antiWindup"]){assert.strictEqual(result.candidate.parameters[key],settings[key],`${key} must retain its imported value`);}
  await capture("imported-controller-settings");
}
