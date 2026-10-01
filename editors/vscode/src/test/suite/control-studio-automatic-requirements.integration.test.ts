import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { withStudio } from "./control-studio-three-page.integration.test";
import { pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio automatic requirements UI", () => {
  test("prefills an older saved model that has no tuning task", async function () {
    if (process.env.TRUST_AUTOMATIC_REQUIREMENTS_LEGACY !== "1") this.skip();
    this.timeout(35000);
    const storage=require("../../controlStudio/engineerDraftStorage");
    const original=storage.restoreEngineerDraft, workflow=storage.restoreWorkflowDraft;
    storage.restoreEngineerDraft=()=>({campaigns:[],selectedModel:{kind:'transfer-function',numerator:[1.5],denominator:[4,1],delaySeconds:.5,
      inputs:[{id:'u',unit:'%',operatingRange:{minimum:0,maximum:100}}],outputs:[{id:'y',unit:'°C',operatingRange:{minimum:0,maximum:100}}],
      operatingPoint:{inputs:[20],outputs:[50]},source:'provided',validation:'validated',revision:'legacy-model',evidence:['declared-legacy-model']}});
    storage.restoreWorkflowDraft=()=>undefined;
    try {
      await withStudio(async (inner,_reopen,capture)=>{
        await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-requirements]').click();return 'legacy requirements';");
        await capture('legacy-automatic-requirements');
        assert.ok(Number(await inner("return d.querySelector('[data-studio-requirement=durationSeconds]').value;"))>0,'The restored host must send its generated task to the optional editor');
      });
    } finally {storage.restoreEngineerDraft=original;storage.restoreWorkflowDraft=workflow;}
  });
  test("identifies, tunes without a form, retains editable defaults and validates their frozen criteria", async function () {
    if (process.env.TRUST_AUTOMATIC_REQUIREMENTS !== "1") this.skip();
    this.timeout(170000);
    await withStudio(async (inner, reopen, capture) => {
      await inner("d.defaultView.__automatic=[];d.defaultView.addEventListener('trust-control-studio-engineer',e=>d.defaultView.__automatic.push(e.detail));d.querySelector('[data-engineer-find]').click();return 'identify';");
      await pollValue("identified model",()=>inner("return JSON.stringify(d.defaultView.__automatic);"),value=>JSON.parse(value).some((item:any)=>['model-ready','failed'].includes(item.status)),50000);
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-requirements]').click();return 'preview defaults';");
      const initialFields=JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-studio-requirement]')].map(e=>({key:e.dataset.studioRequirement,value:e.value,required:e.required,placeholder:e.placeholder})));"));
      const required=initialFields.filter((field:any)=>field.required);
      assert.ok(required.length>=5 && required.every((field:any)=>field.value!==''),'The optional Requirements editor must prefill every required field before the first Tune');
      assert.deepStrictEqual(initialFields.filter((field:any)=>!field.required).map((field:any)=>[field.key,field.value,field.placeholder]),[['constantInputDisturbance','','None'],['transferTolerance','','Automatic']],
        'no load trial is declared and the Auto-entry tolerance stays blank for the automatic allowance');
      assert.ok(initialFields.every((field:any)=>Number(field.value)===Number(Number(field.value).toPrecision(3))),'Generated values should use readable engineering precision');
      await capture('automatic-requirements-before-tune');
      await inner("[...d.querySelectorAll('[data-trust-compact-editor] button')].find(b=>b.textContent==='Cancel').click();d.querySelector('[data-engineer-tune]').click();return 'tune';");
      await new Promise(resolve=>setTimeout(resolve,700));
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"),'false','Tune must derive missing test settings instead of opening an empty requirements form');
      const tuned=JSON.parse(await pollValue("automatic candidate",()=>inner("return JSON.stringify(d.defaultView.__automatic);"),value=>JSON.parse(value).some((item:any)=>['candidate','failed'].includes(item.status)),25000));
      const candidate=tuned.find((item:any)=>item.status==='candidate');
      assert.ok(candidate,JSON.stringify(tuned.map((item:any)=>({status:item.status,reason:item.reason}))));
      assert.ok(candidate.task?.automaticCriteria?.fields.length);
      assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.selectedStage;"),'tune');
      assert.ok(!tuned.some((item:any)=>item.liveModelValidation||item.status==='verified'));
      await capture('automatic-tune');
      await inner("d.querySelector('[data-studio-requirements]').click();return 'requirements';");
      const fields=JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-studio-requirement]')].map(e=>({key:e.dataset.studioRequirement,value:e.value})));"));
      assert.ok(fields.filter((field:any)=>['referenceChange','maximumError','settlingBand','settlingTimeSeconds','durationSeconds'].includes(field.key)).every((field:any)=>field.value!==''));
      assert.match(await inner("return d.querySelector('[data-trust-compact-editor]').textContent;"),/Automatic/);
      await capture('automatic-requirements');
      if (process.env.TRUST_AUTOMATIC_REQUIREMENTS_REVIEW_ONLY === '1') return;
      await inner("[...d.querySelectorAll('[data-trust-compact-editor] button')].find(b=>b.textContent==='Cancel').click();return 'cancel';");
      await reopen();
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-requirements]').click();return 'reopen requirements';");
      assert.strictEqual(Number(await inner("return d.querySelector('[data-studio-requirement=durationSeconds]').value;")),candidate.task.durationSeconds);
      await inner("[...d.querySelectorAll('[data-trust-compact-editor] button')].find(b=>b.textContent==='Cancel').click();d.querySelector('[data-engineer-tune]').click();return 'retune';");
      await pollValue("retune completed",()=>inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"),value=>value==='false',25000);
      await inner("d.defaultView.__validated=[];d.defaultView.addEventListener('trust-control-studio-engineer',e=>d.defaultView.__validated.push(e.detail));d.querySelector('[data-engineer-stage-tab=verify]').click();d.querySelector('[data-engineer-verify]').click();return 'validate';");
      const validated=JSON.parse(await pollValue("automatic validation",()=>inner("return JSON.stringify(d.defaultView.__validated);"),value=>JSON.parse(value).some((item:any)=>['verified','verification-failed','failed'].includes(item.status)),90000));
      fs.writeFileSync(path.join(path.dirname(requiredEnvironment('TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT')),'automatic-criteria.json'),JSON.stringify({tuned,validated}));
      const result=validated.find((item:any)=>item.status==='verified');
      assert.ok(result,JSON.stringify(validated.map((item:any)=>({status:item.status,reason:item.reason}))));
      assert.deepStrictEqual(result.task,candidate.task,'Validate must use the frozen criteria');
      assert.match(await inner("return d.querySelector('[data-studio-validation-result]').textContent;"),/Automatic/);
      await capture('automatic-validation');
    },{loop:loop=>{loop.experiment.durationSeconds=30;}});
  });
});
