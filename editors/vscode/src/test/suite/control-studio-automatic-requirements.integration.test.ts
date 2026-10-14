import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { engineAnswerRecorderScript } from "./control-studio-rendered-harness";
import { withStudio } from "./control-studio-three-page.integration.test";
import { pollValue, requiredEnvironment } from "./plant-rendered-harness";
import { parseDurationSeconds } from "../../controlStudio/webview/durationField";

// The fixture loop (examples/control-pid-commissioning, a Control.PID law without a parked feature) is served by the engine
// (crates/trust-control/src/loop_definition.rs `rust_engine_support`), so the automatic journey reads the engine's answers that the
// page records (control-studio-rendered-harness.ts `engineAnswerRecorderScript`): the engine route sends no status events.

type View = (body: string) => Promise<string>;

/** One requirement row of the candidate's target-response run in Validate's table (RequirementOutcome, B5.4). */
interface ValidationRow { id: string; limit?: number; automatic: boolean }
/** One recorded engine answer as this journey reads it: the status the page gives it and the fields it checks, never a plot. */
interface Answer {
  kind: string;
  method?: string;
  status: string;
  command?: string;
  reason?: string;
  /** The type of the model a Run or an Identify answered (IdentifyResult, B3.8). */
  modelType?: string;
  /** The requirements a Tune or a Validate answered (TuneResult and ValidateResult `requirements`, B4.3, B5.4). */
  requirements?: { durationSeconds?: number; maximumError?: number; automaticCriteria?: { fields: string[] }; [key: string]: unknown };
  /** The candidate's requirement rows of the target-response case in a Validate answer. */
  rows?: ValidationRow[];
}

/** The engine answers the page recorded, from the `from`-th on. */
const answersFrom = (from: number): string =>
  `return JSON.stringify((d.defaultView.__engineAnswers||[]).slice(${from}).map(e=>{const r=e.result||{};const i=e.method==='run'?r.identification:e.method==='identify'?r:undefined;const target=e.method==='validate'&&e.kind==='control-studio.engine.result'?(r.scenarios||[]).find(s=>s.id==='target-response'):undefined;return {kind:e.kind,method:e.method,status:e.status,command:e.command,reason:e.reason,modelType:i&&i.modelType,requirements:(e.method==='tune'||e.method==='validate')&&e.kind==='control-studio.engine.result'?r.requirements:undefined,rows:target?target.candidate.requirements.map(q=>({id:q.id,limit:q.limit,automatic:q.automatic})):undefined};}));`;
const answerCount = async (inner: View): Promise<number> => Number(await inner("return String((d.defaultView.__engineAnswers||[]).length);"));
/** Whether a Tune has ended since the `from`-th answer: its candidate, or its failure. */
const tuneEndedScript = (from: number): string =>
  `return JSON.stringify({busy:d.querySelector('[data-studio-three-page]').dataset.busy,tuned:(d.defaultView.__engineAnswers||[]).slice(${from}).some(e=>e.status==='candidate'||(e.status==='failed'&&(e.method==='tune'||e.command==='tune')))});`;

const REPORTED_GAPS = "Checks the engine route cannot pass yet (reported gaps)";
/**
 * Runs one check of a value the engine route does not show yet (a reported gap) where it stands, logs it when it fails, and lets the
 * journey go on, so that one run shows every gap; the test fails at its end when any gap failed.
 */
async function reportedGap(gaps: string[], name: string, check: () => unknown): Promise<void> {
  try { await check(); }
  catch (error) {
    const gap = `${name}: ${error instanceof Error ? error.message : String(error)}`;
    gaps.push(gap);
    console.error(`Reported gap: ${gap}`);
  }
}

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
        assert.ok((parseDurationSeconds(await inner("return d.querySelector('[data-studio-requirement=durationSeconds]').value;")) ?? 0)>0,'The restored host must send its generated task to the optional editor');
      });
    } finally {storage.restoreEngineerDraft=original;storage.restoreWorkflowDraft=workflow;}
  });
  test("identifies, tunes without a form, retains editable defaults and validates their frozen criteria", async function () {
    if (process.env.TRUST_AUTOMATIC_REQUIREMENTS !== "1") this.skip();
    // 20 s more than the TypeScript route's 170 s: the runtime start is now its own step before Identify.
    this.timeout(190000);
    await withStudio(async (inner, reopen, capture) => {
      const gaps: string[] = [];
      await inner(engineAnswerRecorderScript());
      // The engine route identifies and validates on the connected runtime and refuses without one (specification 35, A2.7;
      // connected_run.rs `runtime_required`); the TypeScript route started the local simulator itself (runtimeSearchConnection.ts).
      const started = await runtimeLifecycleService.startLocalSimulator();
      assert.ok(started.ok, JSON.stringify(started));
      await inner("d.querySelector('[data-engineer-find]').click();return 'identify';");
      // The Identify page's run answers the identification it made (engineRun.ts), or its failure.
      await pollValue("identified model",()=>inner(answersFrom(0)),value=>(JSON.parse(value) as Answer[]).some(item=>typeof item.modelType==='string'||item.status==='failed'),50000);
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-requirements]').click();return 'preview defaults';");
      const initialFields=JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-studio-requirement]')].map(e=>({key:e.dataset.studioRequirement,value:e.value,required:e.required,placeholder:e.placeholder})));"));
      const required=initialFields.filter((field:any)=>field.required);
      // Gap G5: the Identify answer names the requirements Tune designs to (IdentifyResult `requirements`, B4.3), and the editor opens
      // with them, so every required row has a value before the first Tune.
      await reportedGap(gaps, "G5 requirements before the first Tune", () => assert.ok(required.length>=5 && required.every((field:any)=>field.value!==''),'The optional Requirements editor must prefill every required field before the first Tune'));
      assert.deepStrictEqual(initialFields.filter((field:any)=>!field.required).map((field:any)=>[field.key,field.value,field.placeholder]),[['maximumOvershootPercent','','None'],['constantInputDisturbance','','None'],['transferTolerance','','Automatic']],
        'no overshoot limit or load trial is declared and the Auto-entry tolerance stays blank for the automatic allowance');
      // Durations show in their largest unit ("47.4 s", "15 min") and parse back; other values keep three significant digits.
      assert.ok(initialFields.filter((field:any)=>/Seconds$/.test(field.key)&&field.value!=='').every((field:any)=>parseDurationSeconds(field.value)!==undefined),'Generated durations should read as durations');
      assert.ok(initialFields.filter((field:any)=>!/Seconds$/.test(field.key)).every((field:any)=>Number(field.value)===Number(Number(field.value).toPrecision(3))),'Generated values should use readable engineering precision');
      await capture('automatic-requirements-before-tune');
      const beforeTune=await answerCount(inner);
      await inner("[...d.querySelectorAll('[data-trust-compact-editor] button')].find(b=>b.textContent==='Cancel').click();d.querySelector('[data-engineer-tune]').click();return 'tune';");
      await new Promise(resolve=>setTimeout(resolve,700));
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"),'false','Tune must derive missing test settings instead of opening an empty requirements form');
      const tuned=JSON.parse(await pollValue("automatic candidate",()=>inner(answersFrom(0)),value=>(JSON.parse(value) as Answer[]).slice(beforeTune).some(item=>['candidate','failed'].includes(item.status)),25000)) as Answer[];
      const candidate=tuned.slice(beforeTune).find(item=>item.status==='candidate');
      assert.ok(candidate,JSON.stringify(tuned.map(item=>({status:item.status,reason:item.reason}))));
      // The engine's Tune answer names the requirements it generated (TuneResult `requirements.automaticCriteria`, B4.3).
      assert.ok(candidate.requirements?.automaticCriteria?.fields.length);
      assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.selectedStage;"),'tune');
      // Tune collects no model evidence and validates nothing: the engine has run no Validate.
      assert.ok(!tuned.some(item=>item.method==='validate'||item.command==='validate'||item.status==='verified'));
      await capture('automatic-tune');
      await inner("d.querySelector('[data-studio-requirements]').click();return 'requirements';");
      const fields=JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-studio-requirement]')].map(e=>({key:e.dataset.studioRequirement,value:e.value})));"));
      const editorText=await inner("return d.querySelector('[data-trust-compact-editor]').textContent;");
      // Gap G5: the engine's Tune candidate reaches the page as the `engineer` stage with its requirements as `task` (TuneResult
      // `requirements`), so the editor shows them and their Automatic marks.
      await reportedGap(gaps, "G5 candidate requirements in the editor", () => assert.ok(fields.filter((field:any)=>['referenceChange','maximumError','settlingBand','settlingTimeSeconds','durationSeconds'].includes(field.key)).every((field:any)=>field.value!=='')));
      await reportedGap(gaps, "G5 Automatic marks in the editor", () => assert.match(editorText,/Automatic/));
      await capture('automatic-requirements');
      if (process.env.TRUST_AUTOMATIC_REQUIREMENTS_REVIEW_ONLY === '1') { assert.deepStrictEqual(gaps, [], REPORTED_GAPS); return; }
      await inner("[...d.querySelectorAll('[data-trust-compact-editor] button')].find(b=>b.textContent==='Cancel').click();return 'cancel';");
      await reopen();
      await inner(engineAnswerRecorderScript());
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-requirements]').click();return 'reopen requirements';");
      const reopenedDuration=parseDurationSeconds(await inner("return d.querySelector('[data-studio-requirement=durationSeconds]').value;"));
      // Gap G3: the reopened loop's identification names the requirements Tune designs to (OpenResult `identification`), which the editor
      // opens with; the study's recorded requirements replace them only when the study holds some.
      await reportedGap(gaps, "G3 reopened requirements", () => assert.strictEqual(reopenedDuration,candidate.requirements?.durationSeconds));
      const beforeRetune=await answerCount(inner);
      await inner("[...d.querySelectorAll('[data-trust-compact-editor] button')].find(b=>b.textContent==='Cancel').click();d.querySelector('[data-engineer-tune]').click();return 'retune';");
      // Tune has run again when the page is idle and the engine has answered this Tune, a candidate after the click (gap G3: the
      // reopened identification must give the page its model evidence, or Tune stays unavailable and this wait cannot end).
      await pollValue("retune completed",()=>inner(tuneEndedScript(beforeRetune)),value=>{const state=JSON.parse(value) as {busy:string;tuned:boolean};return state.busy==='false'&&state.tuned;},25000);
      const beforeValidate=await answerCount(inner);
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();d.querySelector('[data-engineer-verify]').click();return 'validate';");
      const validated=JSON.parse(await pollValue("automatic validation",()=>inner(answersFrom(beforeValidate)),value=>(JSON.parse(value) as Answer[]).some(item=>['verified','verification-failed','failed'].includes(item.status)),90000)) as Answer[];
      fs.writeFileSync(path.join(path.dirname(requiredEnvironment('TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT')),'automatic-criteria.json'),JSON.stringify({tuned,validated}));
      const result=validated.find(item=>item.status==='verified');
      assert.ok(result,JSON.stringify(validated.map(item=>({status:item.status,reason:item.reason}))));
      // Validate judged the candidate against the criteria Tune froze (B4.3, B5.4): the target response's error limit is the frozen
      // maximum error (crates/trust-control/src/validation/engineer_verification_plan.rs `target-error`), and the generated settling
      // time and error rows are marked Automatic (study/validate/answer.rs `generated_limit`).
      const rows=result.rows??[];
      assert.strictEqual(rows.find(row=>row.id==='target-error')?.limit,candidate.requirements?.maximumError,'Validate must use the frozen criteria');
      const generated=candidate.requirements?.automaticCriteria?.fields??[];
      for (const [field,id] of [['settlingTimeSeconds','response-deadline'],['maximumError','target-error']] as const) {
        if (generated.includes(field)) assert.strictEqual(rows.find(row=>row.id===id)?.automatic,true,`${field} is a frozen automatic criterion`);
      }
      // The Validate answer carries the requirements it judged against (ValidateResult `requirements`, B5.4); they must be the
      // candidate's whole record (gap G4).
      await reportedGap(gaps, "G4 frozen requirement record", () => assert.deepStrictEqual(result.requirements,candidate.requirements,'Validate must use the frozen criteria'));
      assert.match(await inner("return d.querySelector('[data-studio-validation-result]').textContent;"),/Automatic/);
      await capture('automatic-validation');
      assert.deepStrictEqual(gaps, [], REPORTED_GAPS);
    },{loop:loop=>{loop.experiment.durationSeconds=30;}});
  });
});
