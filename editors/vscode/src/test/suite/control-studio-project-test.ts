import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import type { RecordedChartSample } from "../../controlStudio/generated/OpenResult";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";
import { pollValue, sleep } from "./plant-rendered-harness";

/**
 * The project test of a project-scripted lesson as the engineer drives it on the Identify page (specification 35, C12): choose
 * Project experiment, review the declared test, confirm it, wait for the engine to end it, and read what the engine kept. The page is
 * `webview/threePageExperiments.ts`; the answers are the engine's (`engine/routes/experimentRoute.ts`), which a journey reads back
 * from the page's recorder (`window.__engineAnswers`, control-studio-rendered-harness.ts). The helpers decide nothing.
 */

const FIND = "[data-engineer-find]";

/** The words the page gives the end of a project test whose outputs the runtime confirmed restored (webview/threePageExperiments.ts). */
export const PROJECT_TEST_COMPLETED = "Project test completed. Outputs restored.";

/** What the Identify page shows of the project test: the action's words, the test's status line and the page's own message. */
interface ProjectTestPage {
  readonly label: string;
  readonly status: string;
  readonly message: string;
}

const PROJECT_TEST_PAGE = `return JSON.stringify({label:d.querySelector(${JSON.stringify(FIND)}).textContent,`
  + "status:d.querySelector('[data-three-page-experiment-status]').textContent,message:d.querySelector('[data-studio-message=model]').textContent});";

/** The status lines that are steps of the test itself: while one shows, the test is not over (webview/threePageExperiments.ts). */
const TEST_STEPS = ["", "Reviewing test…", "Starting test…", "Test running…"];

/** What a finished project test answers (`IdentifyResult`, or `ProjectTestResult` when the fit refuses): the kept recording and its samples. */
export interface ProjectTestAnswer {
  /** The kept recording's file identifier (A8.3). */
  readonly recordingId: string;
  /** `restored` once the runtime confirmed the experiment's restoring writes (C12). */
  readonly restoration?: string;
  /** The identified model's type, when the recording supports a fit; the fit's refusal is `fitRefusal` otherwise (B3.5). */
  readonly modelType?: string;
  readonly equation?: string;
  readonly provisional?: boolean;
  readonly fitRefusal?: string;
  /** The samples of the kept recording, as the Identify chart draws them. */
  readonly recordedSamples: RecordedChartSample[];
}

/** Applies the theme a rendered run asks for (`TRUST_CONTROL_STUDIO_RENDERED_THEME`); a run that names none keeps the profile's default theme. */
export async function applyRenderedTheme(): Promise<void> {
  const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
  if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
}

/** Clicks a control once it is visible and enabled: the engine enables a control with the answer that allows it, which the page shows a moment later. */
export async function clickWhenEnabled(inner: EvaluateWebview, selector: string, timeoutMs = 20_000): Promise<void> {
  await pollValue(`${selector} visible and enabled`,
    () => inner(`const button=d.querySelector(${JSON.stringify(selector)});if(!button||!button.checkVisibility()||button.disabled)return 'unavailable';button.click();return 'clicked';`),
    value => value === "clicked", timeoutMs);
}

/** Chooses the Project experiment method of a project-scripted loop; the Identify action then offers the review. */
export async function chooseProjectExperiment(inner: EvaluateWebview): Promise<void> {
  assert.strictEqual(await inner("return String(!!d.querySelector('[data-engineer-acquisition-method] option[value=project]'));"), "true",
    "A project-scripted lesson has a reachable Project experiment method");
  await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='project';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'project';");
  // The recorded inputs of the opened study can arrive just after the open and restore the method the study kept (webview/threePageWorkflow.ts
  // restoreRecordedInputs), so the choice is read again once they have had their moment.
  await sleep(500);
  assert.strictEqual(await inner("return d.querySelector('[data-engineer-acquisition-method]').value;"), "project", "the Project experiment method stays chosen");
  assert.strictEqual(await inner(`return d.querySelector(${JSON.stringify(FIND)}).textContent;`), "Review project test");
}

/**
 * Starts the Simulator, unless it already runs, as a course project's README step 1 does. The project test's review needs the running
 * runtime: without it the engine answers with the next action Start simulator (specification 35, A2.8), which the page offers as a button.
 */
export async function startSimulator(): Promise<void> {
  if ((await runtimeLifecycleService.snapshot()).status.running) return;
  const started = await runtimeLifecycleService.startLocalSimulator();
  assert.ok(started.ok, JSON.stringify(started));
}

/**
 * Presses Review project test on the running Simulator and waits for the engine's review (the action then reads Confirm and start).
 * Returns the declared duration the review shows. A refused review fails here with the sentence the page shows.
 */
export async function reviewProjectTest(inner: EvaluateWebview, timeoutMs = 90_000): Promise<string> {
  await startSimulator();
  await clickWhenEnabled(inner, FIND);
  const page = JSON.parse(await pollValue("the engine's review of the project test", () => inner(PROJECT_TEST_PAGE), value => {
    const shown = JSON.parse(value) as ProjectTestPage;
    return shown.label === "Confirm and start" || (shown.label === "Review project test" && !TEST_STEPS.includes(shown.status));
  }, timeoutMs)) as ProjectTestPage;
  assert.strictEqual(page.label, "Confirm and start", `The engine reviews the project test: ${JSON.stringify(page)}`);
  return inner("return d.querySelector('[data-three-page-experiment-duration]').textContent;");
}

/** Presses Confirm and start: the engine starts the campaign the review described, and time zero is the press. */
export async function startProjectTest(inner: EvaluateWebview): Promise<void> {
  await clickWhenEnabled(inner, FIND);
}

/** Waits until the page offers Abort: the engine has started the test (README, Select Abort test to stop early). */
export async function waitForProjectTestRunning(inner: EvaluateWebview, timeoutMs = 15_000): Promise<void> {
  await pollValue("the running project test", () => inner(`return d.querySelector(${JSON.stringify(FIND)}).textContent;`), value => value.startsWith("Abort"), timeoutMs);
}

/**
 * Waits for the engine to end the project test: the action is Review project test again and the status is no longer a step of the
 * test. Returns the status the page shows (`Project test completed. Outputs restored.` for a completed test).
 */
export async function waitForProjectTestEnd(inner: EvaluateWebview, timeoutMs: number): Promise<string> {
  const page = JSON.parse(await pollValue("the end of the project test", () => inner(PROJECT_TEST_PAGE), value => {
    const shown = JSON.parse(value) as ProjectTestPage;
    return shown.label === "Review project test" && !TEST_STEPS.includes(shown.status);
  }, timeoutMs)) as ProjectTestPage;
  return page.status;
}

/**
 * Waits until the chart draws the kept recording. The chart counts its series and its points on each render (webview/threePageChart.ts),
 * and the page draws the recording the engine kept (`recording.evidence`, enginePanelRoute.ts) as one series for each selected signal
 * with a point for every sample, a gap for an invalid one. A live chart or a model comparison does not hold that many points. A
 * timeout names what the chart held, so a different outcome shows at once.
 */
export async function waitForRecordedChart(inner: EvaluateWebview, answer: ProjectTestAnswer): Promise<void> {
  await pollValue(`the kept recording of ${answer.recordedSamples.length} samples on the chart (series and points held)`,
    () => inner("const chart=d.querySelector('[data-control-studio-chart]');return JSON.stringify({series:Number(chart.dataset.seriesCount),points:Number(chart.dataset.pointCount)});"),
    value => {
      const chart = JSON.parse(value) as { series: number; points: number };
      return chart.series >= 2 && chart.points === chart.series * answer.recordedSamples.length;
    }, 15_000);
}

/** The last answer of the engine to a campaign that kept a recording, as the page's recorder holds it (`IdentifyResult.recordedSamples`, identify_action.rs keep_campaign). */
const LAST_PROJECT_TEST_ANSWER = "const answers=(d.defaultView.__engineAnswers||[]).filter(e=>e.kind==='control-studio.engine.result'&&e.method==='identify'&&e.result&&Array.isArray(e.result.recordedSamples));"
  + "const last=answers[answers.length-1];if(!last)return 'null';const r=last.result;"
  + "return JSON.stringify({recordingId:r.recordingId,restoration:r.restoration,modelType:r.modelType,equation:r.equation,provisional:r.provisional,fitRefusal:r.fitRefusal,recordedSamples:r.recordedSamples});";

/** The engine's answer to the finished project test, with the samples of the recording it kept. */
export async function projectTestAnswer(inner: EvaluateWebview): Promise<ProjectTestAnswer> {
  const text = await pollValue("the engine's answer to the project test", () => inner(LAST_PROJECT_TEST_ANSWER), value => value !== "null", 15_000);
  return JSON.parse(text) as ProjectTestAnswer;
}

/**
 * The recording file the engine keeps in the project (A8.3): `<project>/control-studio/<study folder>/recordings/<id>.json`, which holds
 * the run exactly as the runtime returned it (`run`, with its acquisition record) and the verdict on the recording (`evidence`).
 */
export function keptRecording(project: string, recordingId: string): any {
  const studies = path.join(project, "control-studio");
  for (const folder of fs.existsSync(studies) ? fs.readdirSync(studies) : []) {
    const file = path.join(studies, folder, "recordings", `${recordingId}.json`);
    if (fs.existsSync(file)) return JSON.parse(fs.readFileSync(file, "utf8"));
  }
  return assert.fail(`The study keeps no recording ${recordingId} under ${studies}.`);
}

/**
 * Checks that the recording file the engine kept holds the run exactly as the runtime recorded it, complete: the runtime scan
 * buffer without a gap, the verdict on the recording, and the same number of samples as the answer carries (A8.3, E4.5). Returns the file.
 */
export function assertKeptRecordingComplete(project: string, answer: ProjectTestAnswer): any {
  const file = keptRecording(project, answer.recordingId);
  assert.strictEqual(file.run.acquisition.method, "runtime_scan_buffer");
  assert.strictEqual(file.run.acquisition.completeness, "complete");
  assert.deepStrictEqual(file.run.acquisition.missingScanRanges, []);
  assert.strictEqual(file.evidence.completeness, "complete", file.evidence.reason);
  assert.strictEqual(file.run.samples.length, answer.recordedSamples.length, "the answer carries every sample of the kept recording");
  return file;
}

/** What the Identify page shows of the study's model: whether the result is shown, its type line and its equation. */
async function shownModel(inner: EvaluateWebview): Promise<{ shown: boolean; type: string; equation: string }> {
  return JSON.parse(await inner("const result=d.querySelector('[data-studio-model-result]');return JSON.stringify({shown:Boolean(result&&!result.hidden),"
    + "type:d.querySelector('[data-studio-model-type]')?.textContent??'',equation:d.querySelector('[data-studio-model-equation]')?.textContent??''});"));
}

/**
 * A reopened panel shows the study the engine kept (A8.6): the model the project test identified when its recording supported a fit, and
 * none when the fit was refused and the study kept the recording alone (C12, B3.5).
 */
export async function assertStudyReopened(inner: EvaluateWebview, answer: ProjectTestAnswer): Promise<void> {
  const shown = await shownModel(inner);
  if (answer.modelType === undefined) {
    assert.strictEqual(shown.shown, false, "a recording that supported no fit leaves the reopened study without a model");
    return;
  }
  assert.deepStrictEqual(shown, { shown: true, type: `${answer.modelType}${answer.provisional ? " · Provisional" : ""}`, equation: answer.equation },
    "the reopened study shows the model the project test identified");
}

/** The value of the additional signal `id` at a sample of the kept recording; the signal must be present, finite and good. */
export function channel(sample: RecordedChartSample, id: string): number {
  const found = sample.additional?.find(value => value.id === id);
  assert.ok(found && typeof found.value === "number" && Number.isFinite(found.value), `${id} at scan ${sample.scan}`);
  assert.strictEqual(found.quality, "good");
  return found.value;
}
