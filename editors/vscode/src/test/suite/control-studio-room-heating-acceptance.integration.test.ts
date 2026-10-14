import * as assert from "assert";
import * as fs from "fs";
import * as http from "http";
import * as path from "path";
import { execFileSync } from "child_process";
import { isDeepStrictEqual } from "util";
import * as vscode from "vscode";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { loopInspectionModeId } from "../../controlStudio/loopInspectionBindings";
import { connectRuntimeParameters } from "../../controlStudio/runtimeParameterConnection";
import type { SymbolicInspectionSample } from "../../controlStudio/symbolicInspection";
import { attachControlStudio, engineAnswerRecorderScript } from "./control-studio-rendered-harness";
import {
  captureRenderedWorkbenchCdp,
  connectCdp,
  devToolsPort,
  evaluateInWebview,
  httpJson,
  pollValue,
  requiredEnvironment,
  sleep,
  type CdpConnection,
} from "./plant-rendered-harness";
import { ExtensionHostResponsiveness, LONGEST_EXTENSION_HOST_BLOCK_MS } from "./extension-host-responsiveness";
import {
  inspectAttachedLoop,
  prepareAttachedProject,
  reserveLoopbackPort,
  runtimeBinary,
  startAttachedRuntime,
} from "./control-studio-attached-runtime";

const HOUR_MS = 60 * 60 * 1000;
const TIME_SCALE = 10;
/** examples/room_heating/src/Main.st: TIC101 holds the tank at this setpoint, in degrees Celsius. */
const DESIGN_SETPOINT_C = 45;
/** The tank has settled when TIC101 is in Auto and the tank is within this band of the setpoint (specification 35, D5). */
const SETTLED_BAND_C = 0.5;
const HEATING_TIMEOUT_MS = 2 * HOUR_MS;
/** Identify, Validate and the observation of Apply each take up to 60 min of wall time at 10x; the wait adds slack. */
const STEP_TIMEOUT_MS = 65 * 60_000;
const TUNE_TIMEOUT_MS = 30 * 60_000;
const SHORT_TIMEOUT_MS = 5 * 60_000;
/**
 * These statuses end a wait early, so a failed step fails at once instead of waiting out its timeout. On the engine route the page's
 * receivers name a refused or failed answer "failed" or "verification-failed", an operation that stopped "cancelled", and an
 * automatic run that waits for an input "paused" (control-studio-rendered-harness.ts installEngineAnswerRecorder).
 */
const FAILURE_STATUSES = ["failed", "verification-failed", "recovery-required", "keep-failed", "cancelled", "paused"];
/** The envelope that carries an engine method's answer to the page (webview/engineOpened.ts). */
const ENGINE_RESULT = "control-studio.engine.result";
const HEATING_PANEL = "overview/heating-panel";

/**
 * Records the plots the webview receives, in the page; the journey reads them back with readEvents. The engine route draws its plots
 * through the page's `trust-control-studio-engineer` events, which carry no status: the identification's measured and predicted
 * response (webview/engineIdentify.ts showPlot), Tune's predicted response (engineTune.ts showTunePlots) and the Validate plot
 * (engineValidate.ts showValidationPlot). The engine's answers themselves are read from the engine answer recorder (readAnswersScript).
 */
const OBSERVE = `d.defaultView.addEventListener('trust-control-studio-engineer', event => {
  const p=event.detail;
  d.defaultView.__journeyEvents.push({
    identificationPresentation:p.identificationPresentation?{predictedResponse:p.identificationPresentation.predictedResponse.slice(0,3)}:undefined,
    predictedPoints:p.plots?.timeResponse?.length,
    validationPlot:p.validationPlot===undefined?undefined:p.validationPlot?.title??null});
});`;

/**
 * Reads the engine's answers recorded since `since` back from the page (control-studio-rendered-harness.ts), as small projections,
 * because a plot or a recording can hold thousands of samples. A running update only reports progress, so it is left out. The
 * fields are the engine's (crates/trust-control/src/messages.rs): RunResult.identification (IdentifyResult) and needs,
 * ValidateResult.applyAvailable and each scenario's plant, ApplyOutcome.result (the commissioning result) and its transfers, the
 * reason of a Restore or Keep outcome, and KeepResult.kept.
 */
const readAnswersScript = (since: number) => `return JSON.stringify((d.defaultView.__engineAnswers??[]).slice(${since}).filter(e=>e.status!=='running').map(e=>{
  const r=e.result??{};const model=r.identification??r;const commissioning=r.result??{};
  return {kind:e.kind,method:e.method,status:e.status,reason:e.reason,failure:e.failure,family:e.family,values:e.values,applicationAction:e.applicationAction,
    modelType:model.modelType,equation:model.equation,provisional:model.provisional,needs:r.needs,
    applyAvailable:r.applyAvailable,plants:Array.isArray(r.scenarios)?r.scenarios.map(s=>s.plant):undefined,
    transfers:Array.isArray(commissioning.transfers)?commissioning.transfers.map(t=>({transaction:t.transaction?{confirmation:t.transaction.confirmation,
      applied_values:t.transaction.applied_values,observed_values:t.transaction.observed_values}:undefined})):undefined,
    answerReason:typeof r.reason==='string'?r.reason:commissioning.reason,kept:r.kept};
}));`;

interface CommissioningTransfer {
  readonly transaction?: {
    readonly confirmation?: string;
    readonly applied_values?: Readonly<Record<string, number>>;
    readonly observed_values?: Readonly<Record<string, number | boolean>>;
  };
}

/** One plot event as the webview forwards it (OBSERVE). Only the field this journey reads is typed. */
interface JourneyEvent {
  readonly identificationPresentation?: { readonly predictedResponse: readonly unknown[] };
}

/** One engine answer as readAnswersScript projects it. Only the fields this journey reads are typed. */
interface JourneyAnswer {
  readonly kind: string;
  readonly method?: string;
  readonly status: string;
  readonly reason?: string;
  readonly family?: string;
  readonly applicationAction?: string;
  readonly modelType?: string;
  readonly equation?: string;
  readonly provisional?: boolean;
  readonly plants?: readonly string[];
  readonly transfers?: readonly CommissioningTransfer[];
  readonly answerReason?: string;
  readonly kept?: readonly string[];
}

/** The engine answer a step waits for: the result of `method`, with the status the page's receiver gives it. */
interface ExpectedAnswer {
  readonly method: string;
  readonly status: string;
}

/** Where a step starts: the number of plot events and of engine answers the page holds before it. */
interface JourneyMark {
  readonly events: number;
  readonly answers: number;
}

/** The parts of the loop file this journey reads directly. */
interface RawLoop {
  readonly controller: { readonly pidInitialization?: { readonly states?: readonly { readonly symbol: string }[] } };
}

interface LoopReading {
  readonly mode: string;
  readonly setpoint: number | null;
  readonly processValue: number | null;
  readonly output: number | null;
  readonly scan: number;
  readonly processTimeNs: number;
}

interface OperationRecord {
  readonly operation: string;
  readonly startedAt: string;
  readonly finishedAt: string;
  readonly status: string;
  readonly applicationAction?: string;
  /** The longest the extension host was held during the operation (A10.7). */
  readonly longestBlockMs?: number;
}

interface HmiResponse {
  readonly status: number;
  readonly setCookie: readonly string[];
  readonly text: string;
  readonly body: unknown;
}

suite("Control Studio room heating acceptance", () => {
  test("identifies tunes validates applies restores and keeps TIC101 on the attached room heating runtime", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ROOM_HEATING !== "1" || process.env.TRUST_PLANT_RENDERED_TEST !== "1") this.skip();
    // Heating, Identify, Validate and the observation of each Apply take hours of wall time at 10x; see the waits below.
    this.timeout(4 * HOUR_MS);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const webPort = await reserveLoopbackPort();
    const endpoint = prepareAttachedProject(project, { webPort });
    const loopPath = path.join(project, "control-loops", "tank-temperature.json");
    const loop = JSON.parse(fs.readFileSync(loopPath, "utf8")) as ControlLoopDefinition;
    const rawStates = (JSON.parse(fs.readFileSync(loopPath, "utf8")) as RawLoop).controller.pidInitialization?.states ?? [];
    const runtimeBuild = execFileSync(runtimeBinary(), ["--version"], { encoding: "utf8" }).trim();
    const plant = plantNames(project);
    const operations: OperationRecord[] = [];
    const journeyStart = Date.now();
    const runtime = await startAttachedRuntime(project, { timeScale: TIME_SCALE, label: "Room heating acceptance" });
    let connection: CdpConnection | undefined;
    let readEvents: (() => Promise<string>) | undefined;
    let readAnswers: ((since: number) => Promise<string>) | undefined;
    let startAnswer: string | undefined;
    let heated: LoopReading | undefined;
    const responsiveness = new ExtensionHostResponsiveness();
    // A check that does not block the next step is recorded and the journey goes on, so one run names every failure it reaches.
    const problems: string[] = [];
    const check = (condition: unknown, message: string) => { if (!condition) problems.push(message); };
    try {
      const heatingStart = Date.now();
      startAnswer = await pressHeatingStart(webPort);
      operations.push(record("Start the heating sequence (operator, HMI Start)", heatingStart, "applied"));
      heated = await waitForHeatedTank(endpoint, loop);
      operations.push(record("Tank settles at 45 °C with TIC101 in Auto", heatingStart, "settled"));

      const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
      const cdp = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
      connection = cdp;
      await responsiveness.start();
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(cdp, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(cdp, session, body);
      readEvents = () => inner("return JSON.stringify(d.defaultView.__journeyEvents??[]);");
      readAnswers = (since: number) => inner(readAnswersScript(since));
      const click = async (selector: string) => {
        const outcome = await inner(`const button=d.querySelector(${JSON.stringify(selector)});if(!button?.checkVisibility()||button.disabled)return JSON.stringify({page:d.querySelector('[data-engineer-stage-tab][aria-selected=true]')?.textContent,visible:button?.checkVisibility(),disabled:button?.disabled});button.click();return 'clicked';`);
        if (outcome !== "clicked") assert.fail(`${selector} is visible and enabled: ${outcome}; latest engine answers ${JSON.stringify((JSON.parse(await readAnswers!(0)) as JourneyAnswer[]).slice(-3))}`
          + (problems.length ? `\nEarlier failures:\n${problems.join("\n")}` : ""));
      };
      const choose = async (selector: string, value: string) => {
        const chosen = await inner(`const select=d.querySelector(${JSON.stringify(selector)});if(!select)return 'NO_FIELD';select.value=${JSON.stringify(value)};select.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return select.value;`);
        assert.strictEqual(chosen, value, `${selector} offers the ${value} method`);
      };
      const mark = async (): Promise<JourneyMark> => {
        const since = JSON.parse(await inner("return JSON.stringify({events:(d.defaultView.__journeyEvents??[]).length,answers:(d.defaultView.__engineAnswers??[]).length});")) as JourneyMark;
        responsiveness.markPhase();
        return since;
      };
      const capture = (name: string) => captureRenderedWorkbenchCdp(cdp, path.join(captures, `room-heating-${name}.png`));
      /**
       * Waits for the engine's answer to a step. The engine route dispatches no status events (webview/threePageHostState.ts hands
       * every `control-studio.engine.*` envelope to engineOpened.ts), so the wait reads the engine answers the page received.
       */
      const waitFor = async (operation: string, expected: ExpectedAnswer, timeoutMs: number, since: JourneyMark): Promise<JourneyAnswer> => {
        const started = Date.now();
        const text = await pollValue(`${operation}: the engine's ${expected.method} answer "${expected.status}"`, () => readAnswers!(since.answers), (value) =>
          (JSON.parse(value) as JourneyAnswer[]).some((answer) => isExpected(answer, expected) || FAILURE_STATUSES.includes(answer.status)), timeoutMs);
        const answers = JSON.parse(text) as JourneyAnswer[];
        const result = answers.find((answer) => isExpected(answer, expected));
        operations.push(record(operation, started, result?.status ?? "not reached", result?.applicationAction, responsiveness.phaseMaxMs()));
        assert.ok(result, `${operation} ended without the engine's ${expected.method} answer "${expected.status}": ${JSON.stringify(answers)}`
          + (problems.length ? `\nEarlier failures:\n${problems.join("\n")}` : ""));
        return result;
      };
      /**
       * Apply, Restore previous settings and Keep in project are reviewed before they write (specification 35, §0.1b, B6.1): the button
       * asks the engine for the review, and a confirmation under the review's identity runs the reviewed change as an operation, whose
       * result the page shows. The room heating runtime runs in simulation mode, so the press of Apply or Restore is the confirmation
       * (A2.5, B9.3): the page confirms the review at once. Keep in project, which edits the project's files, shows its review with its
       * confirmation button (B6.5; webview/threePageApplication.ts showReview).
       */
      const reviewAndConfirm = async (operation: string, button: string, method: "apply" | "restore" | "keep", outcome: string, timeoutMs: number): Promise<JourneyAnswer> => {
        const reviewSince = await mark();
        await click(button);
        if (method !== "keep") return waitFor(operation, { method, status: outcome }, timeoutMs, reviewSince);
        await waitFor(`${operation} review`, { method, status: "review" }, SHORT_TIMEOUT_MS, reviewSince);
        const confirmSince = await mark();
        await click(`[data-engine-confirm="${method}"]`);
        return waitFor(operation, { method, status: outcome }, timeoutMs, confirmSince);
      };

      await pollValue("workflow ready", () => inner("return d.querySelector('[data-studio-three-page]').dataset.ready;"), (value) => value === "true", 10000);
      await pollValue("restored host state", () => inner("return d.querySelector('[data-studio-three-page]').dataset.hostReady;"), (value) => value === "true", 10000);
      // The engine serves TIC101's loop: its open answer gives each stage tab the engine's status (webview/engineOpened.ts,
      // engineShared.ts applyStageStatuses). The TypeScript route sets no status on the tabs, and its answers are not the ones read here.
      assert.notStrictEqual(await inner("return d.querySelector('[data-engineer-stage-tab=model]').dataset.status ?? '';"), "",
        "The engine serves TIC101's loop: the stage tabs show the engine's status");
      const client = (await connectRuntimeParameters(loop)).client;

      // Identify with the Automatic method: a closed-loop setpoint step. TIC101 must stay in Auto. Identify plant is one run of the
      // automatic flow through Identify (engine/routes/runRoute.ts): its answer is the run's result, which holds the identification
      // the run made (RunResult.identification), and the page shows it on the Identify page (webview/engineRun.ts, engineIdentify.ts).
      await click("[data-engineer-stage-tab=model]");
      await inner(`d.defaultView.__journeyEvents=[];${OBSERVE}return 'listen';`);
      await inner(engineAnswerRecorderScript());
      await choose("[data-engineer-acquisition-method]", "automatic");
      await capture("01-identify");
      const identifySince = await mark();
      await click("[data-engineer-find]");
      const identified = await waitFor("Identify", { method: "run", status: "completed" }, STEP_TIMEOUT_MS, identifySince);
      assert.ok(identified.modelType, `Identify returns the identified model: ${JSON.stringify(identified)}`);
      // The Identify page shows the engine's model type, " · Provisional" for a provisional model, and its equation (engineIdentify.ts).
      const shownModel = JSON.parse(await inner("const result=d.querySelector('[data-studio-model-result]');return JSON.stringify({visible:Boolean(result&&!result.hidden),type:d.querySelector('[data-studio-model-type]')?.textContent??'',equation:d.querySelector('[data-studio-model-equation]')?.textContent??''});")) as { visible: boolean; type: string; equation: string };
      check(shownModel.visible && shownModel.type === `${identified.modelType}${identified.provisional ? " · Provisional" : ""}` && shownModel.equation === identified.equation,
        `The Identify page shows the identified model: ${JSON.stringify(shownModel)}`);
      const presented = (JSON.parse(await readEvents!()) as JourneyEvent[]).slice(identifySince.events);
      check(presented.some((event) => (event.identificationPresentation?.predictedResponse.length ?? 0) > 2), "Identify shows the measured response");
      await pollValue("identification finished", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), (value) => value === "false", 10000);
      check(await inner("return String(!!d.querySelector('dialog[open]'));") === "false", "Identification must not open Tune requirements");
      const afterIdentify = await inspectAttachedLoop(endpoint, loop);
      check(afterIdentify && controllerMode(loop, afterIdentify) === "Auto", "Identify with Automatic keeps TIC101 in Auto");
      await checkRoleNames(inner, "the identified response", check);
      await capture("02-identified");

      // Tune with the Automatic method.
      await click("[data-engineer-stage-tab=tune]");
      await choose("[data-engineer-tuning-method]", "automatic");
      const valuesBeforeTune = (await client.status()).values;
      const tuneSince = await mark();
      await click("[data-engineer-tune]");
      const tuned = await waitFor("Tune", { method: "tune", status: "candidate" }, TUNE_TIMEOUT_MS, tuneSince);
      // The Tune page shows the candidate's settings (webview/engineTune.ts tuneCandidatePayload, threePageResults.ts renderTunedController).
      check(await inner("const area=d.querySelector('[data-studio-tuning-result]');return String(Boolean(area&&!area.hidden&&area.querySelector('[data-studio-parameter]')));") === "true",
        `The Tune page shows the ${tuned.family} candidate's settings`);
      await checkRoleNames(inner, "the predicted response", check);
      await capture("03-tuned");

      // Validate: the model check and the isolated simulation. Neither writes the running controller.
      await click("[data-engineer-stage-tab=verify]");
      const validateSince = await mark();
      await click("[data-engineer-verify]");
      const verified = await waitFor("Validate", { method: "validate", status: "verified" }, STEP_TIMEOUT_MS, validateSince);
      // B5.3: the room's plant reaches TIC101 only through the program (transmitter scaling and the heating sequence), and an
      // isolated run starts at time zero from the plant's configured state, so the actual Control.PID runs against the model. The
      // engine names the plant each scenario's runs ran against: the model is "model <revision>" (ScenarioOutcome.plant,
      // crates/trust-control/src/validation/texts.rs model_name); the target-response scenario is the first.
      const plants = verified.plants ?? [];
      check(/^model /.test(plants[0] ?? ""), `Validate runs the isolated simulation against the qualified model: ${plants[0]}`);
      check(plants.length > 0 && plants.every((plant) => /^model /.test(plant)),
        `Every isolated run carries the qualified model as its plant: ${JSON.stringify(plants)}`);
      check(isDeepStrictEqual((await client.status()).values, valuesBeforeTune), "Tuning and isolated validation do not apply runtime gains");
      const table = await inner("return d.querySelector('[data-studio-validation-result]').textContent ?? '';");
      // The engine's words (B5.4): the model check is the table's first row, named Model agreement; it states its error in percent of
      // the measured output's span, so it names no channel (validation/texts.rs MODEL_AGREEMENT, study/validate/answer.rs
      // model_agreement). The requirement rows name the measurement by its role and unit (webview/engineValidate.ts channelText).
      const firstRow = JSON.parse(await inner("const row=d.querySelector('[data-studio-validation-result] tbody tr');return JSON.stringify({id:row?.dataset.studioRequirement??'',title:row?.cells[0]?.textContent??''});")) as { id: string; title: string };
      check(firstRow.id === "model-check" && firstRow.title === "Model agreement", `B5.4: the results table starts with the model check, named Model agreement: ${JSON.stringify(firstRow)}`);
      check(/· Measurement \(°C\)/.test(table), `B9.9: the results table names the measurement by its role: ${table}`);
      check(!/Main\./.test(table), `B9.9: the results table shows no symbol: ${table}`);
      await capture("04-validated");

      // Apply, with observation: one Apply waits for the plant to rest at the validated point, then commissions. The engine names a
      // passed Apply "applied" (webview/engineApplication.ts applyOutcomePayload), and its commissioning result holds the transfers.
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "false", "Apply is offered for the validated candidate");
      const valuesBeforeApply = (await client.status()).values;
      const applied = await reviewAndConfirm("Apply", "[data-engineer-apply]", "apply", "applied", STEP_TIMEOUT_MS);
      const stateTransfer = (applied.transfers ?? []).find((transfer) =>
        transfer.transaction?.applied_values?.["Main.TIC101.StateValue.NominalOutput"] !== undefined);
      check(stateTransfer, "The actual library state transfer was observed");
      for (const state of stateTransfer ? rawStates : []) {
        const adopted: number | undefined = stateTransfer?.transaction?.applied_values?.[state.symbol];
        const observed: number | boolean | undefined = stateTransfer?.transaction?.observed_values?.[state.symbol];
        check(typeof observed === "number" && Number.isFinite(observed), `${state.symbol} observed after ST`);
        if (state.symbol === loop.signals.processVariable.symbol) {
          // A6.4: TIC101's measurement is the PID's own process-value state. The transfer observes it and never writes it.
          check(adopted === undefined, `${state.symbol} is the loop's measurement and is not written`);
        } else {
          check(typeof adopted === "number" && Number.isFinite(adopted), `${state.symbol} adopted`);
        }
      }
      check(stateTransfer?.transaction?.confirmation === "output-image", "The Auto entry is confirmed at the output image");
      check(await inner("return d.querySelector('[data-engineer-apply]').textContent;") === "Restore previous settings",
        "After Apply, the page offers Restore previous settings");
      await capture("05-applied");

      // Restore previous settings: the parameter values are those from before Apply, and TIC101 returns to Auto, the mode it had
      // before Apply (A6.2, B6.4, C1 step 8). The engine names a confirmed Restore "commissioned" (engineApplication.ts
      // restoreOutcomePayload).
      const restored = await reviewAndConfirm("Restore previous settings", "[data-engineer-apply]", "restore", "commissioned", SHORT_TIMEOUT_MS);
      check(restored.applicationAction === "restore", `Restore answers as a restore: ${restored.applicationAction}`);
      check(isDeepStrictEqual((await client.status()).values, valuesBeforeApply), "Restore returns the parameter values from before Apply");
      const afterRestore = await inspectAttachedLoop(endpoint, loop);
      check(afterRestore && controllerMode(loop, afterRestore) === "Auto",
        `Restore returns TIC101 to Auto, the mode it had before Apply (A6.2, B6.4): ${restored.answerReason}`);
      await capture("06-restored");

      // The page offers Apply again only for a candidate validated after the Restore, so validate once more if it is disabled.
      if (await inner("return String(d.querySelector('[data-engineer-apply]').disabled);") === "true") {
        await click("[data-engineer-stage-tab=verify]");
        const revalidateSince = await mark();
        await click("[data-engineer-verify]");
        await waitFor("Validate again after Restore", { method: "validate", status: "verified" }, STEP_TIMEOUT_MS, revalidateSince);
      }
      await reviewAndConfirm("Apply again", "[data-engineer-apply]", "apply", "applied", STEP_TIMEOUT_MS);
      await capture("07-applied-again");

      // Keep in project: the running values become the project's initial values. The engine's Keep result names the fields it wrote
      // (KeepResult.kept) and says what it kept (KeepResult.reason).
      await pollValue("Keep in project offered", () => inner("const keep=d.querySelector('[data-engineer-keep]');return String(Boolean(keep)&&!keep.hidden&&!keep.disabled);"), (value) => value === "true", SHORT_TIMEOUT_MS);
      const kept = await reviewAndConfirm("Keep in project", "[data-engineer-keep]", "keep", "kept", SHORT_TIMEOUT_MS);
      check(kept.applicationAction === "keep", `Keep answers as a keep: ${kept.applicationAction}`);
      check((kept.kept ?? []).length > 0, `Keep in project names the parameters it kept: ${kept.answerReason}`);
      await capture("08-kept");
      check(responsiveness.overallMaxMs() <= LONGEST_EXTENSION_HOST_BLOCK_MS,
        `Control Studio held the extension host for ${Math.round(responsiveness.overallMaxMs())} ms at once; A10.7 allows `
        + `${LONGEST_EXTENSION_HOST_BLOCK_MS} ms. Per operation: ${JSON.stringify(operations.map((item) => [item.operation, item.longestBlockMs]))}`);
      assert.deepStrictEqual(problems, [], `The journey reached Keep in project with these failures:\n${problems.join("\n")}`);
    } finally {
      const finishedAt = new Date().toISOString();
      let events: unknown = [];
      let answers: unknown = [];
      try {
        if (readEvents) events = JSON.parse(await readEvents());
        if (readAnswers) answers = JSON.parse(await readAnswers(0));
      } catch {
        // The owned editor may already have closed; the operations above still record the journey.
      }
      let processTime: { scan: number; processTimeNs: number } | undefined;
      try {
        const sample = await inspectAttachedLoop(endpoint, loop);
        if (sample) processTime = { scan: sample.scan, processTimeNs: sample.runtimeTimeNs };
      } catch {
        // The runtime may already be stopped.
      }
      fs.mkdirSync(captures, { recursive: true });
      const longestBlockMs = responsiveness.overallMaxMs();
      const profileProblem = await responsiveness.stop(path.join(captures, "room-heating-extension-host.cpuprofile"));
      fs.writeFileSync(path.join(captures, "room-heating-acceptance.json"), JSON.stringify({
        journey: "Control Studio room heating acceptance",
        specification: "docs/specs/35-vscode-control-studio.md Part D5",
        project: { fixture: "examples/room_heating", projectId: loop.projectId, loopId: loop.id, loopRevision: loop.revision },
        controller: {
          instance: loop.controller.instance,
          type: loop.controller.type,
          taskPeriodMs: loop.controller.taskPeriodMs,
          revision: loop.controller.revision,
          modeSignal: loop.controller.modeSignal,
        },
        runtime: {
          build: runtimeBuild,
          endpoint,
          timeScale: TIME_SCALE,
          attachedAs: "a runtime started outside VS Code in simulation mode, attached over its unix control socket",
        },
        plant,
        start: startAnswer,
        heating: heated,
        processTime,
        timing: { startedAt: new Date(journeyStart).toISOString(), finishedAt, wallMs: Date.now() - journeyStart },
        responsiveness: {
          specification: "docs/specs/35-vscode-control-studio.md A10.7",
          longestBlockMs,
          limitMs: LONGEST_EXTENSION_HOST_BLOCK_MS,
          profile: profileProblem ? `not written: ${profileProblem}` : "room-heating-extension-host.cpuprofile",
        },
        operations,
        problems,
        events,
        answers,
      }, null, 2));
      connection?.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await runtime.stop();
    }
  });
});

/** Whether an engine answer is the result a step waits for: the method's result, with the status the page's receiver gives it. */
function isExpected(answer: JourneyAnswer, expected: ExpectedAnswer): boolean {
  return answer.kind === ENGINE_RESULT && answer.method === expected.method && answer.status === expected.status;
}

/** B9.9: a plot names the loop's signals by their role, such as "Measurement (°C)", and shows no symbol. */
async function checkRoleNames(inner: (body: string) => Promise<string>, plot: string, check: (condition: unknown, message: string) => void): Promise<void> {
  const label = await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label') ?? '';");
  check(/Measurement \(°C\)/.test(label), `B9.9: ${plot} names the measurement by its role: ${label}`);
  check(!/Main\./.test(label), `B9.9: ${plot} shows no symbol: ${label}`);
}

function record(operation: string, startedMs: number, status: string, applicationAction?: string, longestBlockMs?: number): OperationRecord {
  return {
    operation,
    startedAt: new Date(startedMs).toISOString(),
    finishedAt: new Date().toISOString(),
    status,
    ...(applicationAction ? { applicationAction } : {}),
    ...(longestBlockMs === undefined ? {} : { longestBlockMs: Math.round(longestBlockMs) }),
  };
}

function plantNames(project: string): string[] {
  const text = fs.readFileSync(path.join(project, "simulation.toml"), "utf8");
  return [...text.matchAll(/^name\s*=\s*"([^"]*)"/gm)].map((match) => match[1]);
}

function valueOf(sample: SymbolicInspectionSample, id: string): number | null {
  const index = sample.bindings.findIndex((binding) => binding.id === id);
  return index < 0 ? null : sample.values[index] ?? null;
}

/** Auto or Manual, read from the loop's mode channel: the same channel Control Studio inspects. */
function controllerMode(loop: ControlLoopDefinition, sample: SymbolicInspectionSample): string {
  const modeId = loop.controller.modeSignal ? loopInspectionModeId(loop, sample.bindings, loop.controller.modeSignal) : undefined;
  const value = valueOf(sample, modeId ?? "controllerMode");
  if (value !== 0 && value !== 1) return "unknown";
  const whenTrue = loop.controller.modeWhenTrue ?? "Auto";
  return value === 1 ? whenTrue : whenTrue === "Auto" ? "Manual" : "Auto";
}

function readLoop(loop: ControlLoopDefinition, sample: SymbolicInspectionSample): LoopReading {
  return {
    mode: controllerMode(loop, sample),
    setpoint: valueOf(sample, "setpoint"),
    processValue: valueOf(sample, "processVariable"),
    output: valueOf(sample, "manipulatedVariable"),
    scan: sample.scan,
    processTimeNs: sample.runtimeTimeNs,
  };
}

/** Polls the runtime every 10 s until TIC101 is in Auto at the design setpoint and the tank is within the band. */
async function waitForHeatedTank(endpoint: string, loop: ControlLoopDefinition): Promise<LoopReading> {
  const deadline = Date.now() + HEATING_TIMEOUT_MS;
  let last: LoopReading | undefined;
  while (Date.now() < deadline) {
    const sample = await inspectAttachedLoop(endpoint, loop);
    if (sample) {
      last = readLoop(loop, sample);
      if (last.mode === "Auto" && last.setpoint === DESIGN_SETPOINT_C
        && last.processValue !== null && Math.abs(last.processValue - DESIGN_SETPOINT_C) <= SETTLED_BAND_C) return last;
    }
    await sleep(10_000);
  }
  throw new Error(`TIC101 did not settle at ${DESIGN_SETPOINT_C} °C in Auto within 2 h of wall time. Last values: ${JSON.stringify(last ?? "no coherent sample")}`);
}

/** The HMI bundle of the heating panel: a page command must name the bundle it was shown with. */
async function heatingPanelBundle(webPort: number): Promise<string> {
  const popup: { bundle?: unknown } | undefined = await pollValue("the heating panel's HMI bundle",
    () => httpJson(webPort, `/api/hmi/popup?faceplate=${HEATING_PANEL}`).catch(() => undefined),
    (value) => typeof value?.bundle === "string", 30_000);
  return String(popup?.bundle);
}

/**
 * Presses Start on the heating sequence panel as the operator does: log in as operator, then invoke the panel's start
 * command (Plant.ISequence.Start on Main.Heating). The runtime answers "applied" once the method call is accepted.
 */
async function pressHeatingStart(webPort: number): Promise<string> {
  const bundle = await heatingPanelBundle(webPort);
  const login = await postJson(webPort, "/api/hmi/login", { user: "operator", password: "operator" });
  assert.strictEqual(login.status, 200, `The operator login failed: ${login.text}`);
  const cookie = (login.setCookie[0] ?? "").split(";")[0];
  assert.ok(cookie.length > 0, "The operator login gave no session cookie");
  const command = await postJson(webPort, "/api/hmi/command", {
    id: `heating-start-${Date.now()}`,
    bundle,
    page: `faceplate:${HEATING_PANEL}`,
    object: "start",
    action: "invoke",
  }, cookie);
  assert.strictEqual(command.status, 200, `The Start command was not answered: ${command.text}`);
  assert.strictEqual((command.body as { outcome?: unknown } | undefined)?.outcome, "applied",
    `The heating sequence did not accept Start: ${command.text}`);
  return command.text;
}

function postJson(port: number, route: string, body: unknown, cookie?: string): Promise<HmiResponse> {
  return new Promise((resolve, reject) => {
    const payload = JSON.stringify(body);
    const request = http.request({
      host: "127.0.0.1",
      port,
      path: route,
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Content-Length": Buffer.byteLength(payload),
        ...(cookie ? { Cookie: cookie } : {}),
      },
    }, (response) => {
      let text = "";
      response.setEncoding("utf8");
      response.on("data", (chunk: string) => { text += chunk; });
      response.on("end", () => {
        let parsed: unknown;
        try {
          parsed = JSON.parse(text);
        } catch {
          parsed = undefined;
        }
        resolve({ status: response.statusCode ?? 0, setCookie: response.headers["set-cookie"] ?? [], text, body: parsed });
      });
    });
    request.on("error", reject);
    request.setTimeout(10_000, () => request.destroy(new Error(`HMI request timed out: ${route}`)));
    request.end(payload);
  });
}
