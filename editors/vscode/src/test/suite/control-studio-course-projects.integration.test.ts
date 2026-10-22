import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { pollValue } from "./plant-rendered-harness";
import { withStudio } from "./control-studio-three-page.integration.test";
import { channel, projectTestAnswer } from "./control-studio-project-test";
import { runtimeLifecycleService } from "../../runtimeLifecycle";

/**
 * Two course projects run as their own fixtures (gatedJourneys in runTest.ts): lesson 61's measured MIMO paths and lesson 89's finite
 * search. Each runs the README "Experiment in Control Studio" steps that its checks need, at 1x: the declared durations are kept, and the
 * journey types only values its README names. Both start the Simulator as README step 1 says, because a live action (the project test,
 * the finite search) runs on the connected runtime and the engine refuses it without one (A2.7). Lesson 89's journey starts at step 4
 * (its plant check, project test and measured frequency response are not part of this journey).
 */
suite("Control Studio course projects", () => {
  test("lesson 61 measures the coupled vessels gain matrix with measured MIMO paths after the project test", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    // The declared project test lasts 92.1 s, its first 2 s at rest, and then restores the outputs: the waits allow for the test and the restoration.
    this.timeout(480000);
    await withStudio(async inner => {
      await startSimulator();
      // README steps 4 to 6: Review project test, then Confirm and start. The engine ends the test with its restoration.
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='project';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));d.defaultView.__projectEvents=[];d.defaultView.addEventListener('message',event=>{if(event.data.kind==='control-studio.experiment.result')d.defaultView.__projectEvents.push(event.data);});d.querySelector('[data-engineer-find]').click();return 'review';");
      await pollValue("the engine reviews the project test", () => inner("return d.querySelector('[data-engineer-find]').textContent;"), value => value === "Confirm and start", 45000);
      await inner("d.querySelector('[data-engineer-find]').click();return 'start';");
      const messages = JSON.parse(await pollValue("the project test ends", () => inner("return JSON.stringify(d.defaultView.__projectEvents);"),
        value => JSON.parse(value).some((message: any) => message.phase === "engine-ended" || message.phase === "failed"), 240000));
      const ended = messages.find((message: any) => message.phase === "engine-ended")?.payload;
      assert.strictEqual(ended?.status, "finished", JSON.stringify(messages));
      assert.strictEqual(ended?.restoration, "restored", "The engine names the restoration its campaign confirmed");
      await pollValue("the page says the outputs are restored", () => inner("return d.querySelector('[data-three-page-experiment-status]')?.textContent ?? '';"), value => value === "Project test completed. Outputs restored.", 10000);
      // README step 7 names Tuning method, but the page offers Measured MIMO paths under Analyze on Tune, so the journey opens that list. The
      // engine's `measuredPaths` tool is the page's one entry `measured-mimo`, offered once the study keeps the project test's recording.
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      await pollValue("Measured MIMO paths in the Analyze list", () => inner(analyzeOffers("measured-mimo")), value => value === "true", 15000);
      await inner("d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool=measured-mimo]').click();return 'measured paths';");
      await pollValue("the settled window fields of both heaters", () => inner("return String(!!d.querySelector('[data-mimo-input=heater-2][data-mimo-window=settled-end]'));"), value => value === "true", 15000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-three-page-mimo-analysis]').checkVisibility());"), "true", "The measured-paths section opens on Tune");
      // README step 7 gives recording times "which include the 2 s at rest". The engine's recording starts with the project test's plan and waits
      // for no steady baseline (C12, B3.4 step 2), so the project's activation is the first active sample, 2 s into the recording. The journey
      // proves that premise from the kept recording, and then types the README's windows as the README gives them: each heater's base window
      // lies before its own step, and its settled window ends its 30 s hold. The drift tolerance is 0.001.
      const answer = await projectTestAnswer(inner);
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      const authority = loop.signals.additional.find((signal: any) => signal.address === loop.experiment.activation.address)?.id;
      assert.ok(authority, "the project's activation is one of the loop's recorded signals");
      const onset = answer.recordedSamples.find(sample => channel(sample, authority) === 1);
      assert.ok(onset, "the recording holds the project's activation");
      assert.ok(onset.elapsedSeconds >= README_REST_SECONDS - 1e-9 && onset.elapsedSeconds <= README_REST_SECONDS + ACTIVATION_LAG_SECONDS,
        `the recording starts with the project test: its activation is the first active sample, at the README's ${README_REST_SECONDS} s, not at ${onset.elapsedSeconds} s`);
      await inner(`for(const [input,key,value] of ${JSON.stringify(MIMO_WINDOWS)}){const field=d.querySelector('[data-mimo-input='+input+'][data-mimo-window='+key+']');field.value=String(value);field.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));}`
        + "const drift=d.querySelector('[data-three-page-mimo-analysis-drift]');drift.value='0.001';drift.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));return 'windows';");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-tune]').textContent;"), "Calculate measured paths", "Tune calculates the measured paths");
      await inner("d.querySelector('[data-engineer-tune]').click();return 'calculate';");
      const result = JSON.parse(await pollValue("the measured gain matrix", () => inner(MIMO_RESULT_SCRIPT), value => JSON.parse(value).rows.length === 2, 120000)) as MimoResult;
      assert.strictEqual(result.refusal, "", "The engine does not refuse the calculation");
      // Rows are the vessels and columns the heaters. The measured matrix agrees with the README's, and the README's matrix agrees with
      // the heat balance, each within the README's tolerance of 0.0009 °C per point. The heat balance's gains are the README's
      // K = [9/65 4/65; 4/65 9/65], not their four-decimal roundings: heater 2's own path reads 0.1376 (README, Evidence), 0.00086 from 9/65,
      // and exactly the tolerance from the rounded 0.1385, a difference that binary arithmetic makes 0.0009000000000000119.
      const measured = [[0.1380, 0.0607], [0.0611, 0.1376]];
      const balance = [[9 / 65, 4 / 65], [4 / 65, 9 / 65]];
      result.rows.forEach((row, vessel) => row.slice(1).map(Number).forEach((gain, heater) => {
        assert.ok(Math.abs(gain - measured[vessel][heater]) <= 0.0009, `Vessel ${vessel + 1} from heater ${heater + 1} reads ${gain}, not ${measured[vessel][heater]} within 0.0009`);
        assert.ok(Math.abs(gain - balance[vessel][heater]) <= 0.0009, `Vessel ${vessel + 1} from heater ${heater + 1} reads ${gain}, not the heat balance's ${balance[vessel][heater].toFixed(4)} within 0.0009`);
      }));
      assert.match(result.pairing, /P1\.Vessel1C ← P1\.Heater1Pct/, "Heater 1 is paired with vessel 1");
      assert.match(result.pairing, /P1\.Vessel2C ← P1\.Heater2Pct/, "Heater 2 is paired with vessel 2");
      assert.strictEqual(await inner("return d.querySelector('[data-three-page-mimo-analysis-drift]').value;"), "0.001", "The drift tolerance stays as entered");
    }, { capture: "lesson-61-measured-paths" });
  });

  test("lesson 89 finds the notch of the flexible axis by a finite search and validates the controller", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    // Each search trial is an isolated simulation run of the supplied model, within the loop's 60 s wall budget; Validate then runs the candidate.
    this.timeout(480000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const modelText = fs.readFileSync(path.join(project, "flexible-axis-model.json"), "utf8");
    await withStudio(async inner => {
      // README step 1: start Simulator. The finite search runs its trials on the connected runtime, so Tune is refused without it.
      await startSimulator();
      // README step 4: Identify, Supplied plant model, then open flexible-axis-model.json.
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='supplied-model';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'supplied';");
      await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(modelText)}],'flexible-axis-model.json',{type:'application/json'}));const file=d.querySelector('[data-studio-model-file]');file.files=transfer.files;file.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'model imported';`);
      await pollValue("the supplied flexible-axis model", () => inner("return d.querySelector('[data-studio-model-equation]')?.textContent??'';"), value => value.length > 0, 10000);
      // README step 5: P1 · PID with Automatic, Requirements, Done, then Tune controller.
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-requirements]').click();return 'requirements';");
      await pollValue("the requirements editor", () => inner("return String(!!d.querySelector('dialog[open]'));"), value => value === "true", 5000);
      await inner(requirementsScript(LESSON_89_REQUIREMENTS));
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-tuning-method]').value;"), "automatic", "The lesson keeps Automatic");
      await inner("d.querySelector('[data-engineer-tune]').click();return 'tune';");
      // The search runs as an operation and answers its trials in the Tune area (C5): one row for each combination of the grid.
      const search = JSON.parse(await pollValue("the finite search trials", () => inner(SEARCH_TRIALS_SCRIPT), value => {
        const read = JSON.parse(value) as SearchTable;
        return read.busy === "false" && read.rows.length === 8;
      }, 180000)) as SearchTable;
      assert.strictEqual(search.title, "Finite search · 8 trials", "The search tried every combination of the lesson's axes");
      assert.strictEqual(search.status, "", "The search completed without a sentence about its status");
      const column = (name: string) => search.head.findIndex(heading => heading.includes(name));
      const frequency = column("P1.NotchFrequencyHz"), quality = column("P1.NotchQuality"), bounds = column("Bounds");
      assert.ok(frequency > 0 && quality > 0 && bounds > 0, `The trials name the notch coefficients and their bounds: ${search.head.join(" | ")}`);
      const pair = (cells: string[]) => `${Number(cells[frequency])}/${Number(cells[quality])}`;
      assert.deepStrictEqual(search.rows.map(row => pair(row.cells)).sort(), ["0/1", "0/3", "2/1", "2/3", "3/1", "3/3", "4/1", "4/3"],
        "The trials are the notch frequencies 0, 2, 3 and 4 Hz with Q = 1 and 3 (README)");
      const selected = search.rows.filter(row => row.selected);
      assert.strictEqual(selected.length, 1, "One trial is selected");
      assert.strictEqual(pair(selected[0].cells), "3/1", "The selected trial is the 3 Hz notch with Q = 1 (README)");
      assert.strictEqual(selected[0].cells[bounds], "Met", "The selected trial meets the bounds before its J ranks it");
      // README step 6: Validate controller, then every row passes. The table compares the current settings with the candidate (B5.4): the
      // lesson's current notch is off, so its load swings (README, Evidence 2) and the current run fails its response checks. Apply depends on
      // the candidate's rows alone, so these are the rows that must pass.
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();d.querySelector('[data-engineer-verify]').click();return 'validate';");
      const validation = JSON.parse(await pollValue("the Validate rows", () => inner(VALIDATION_ROWS_SCRIPT), value => {
        const read = JSON.parse(value) as ValidateRows;
        return read.busy === "false" && read.rows.some(row => row.id === "overall");
      }, 180000)) as ValidateRows;
      const candidateRows = validation.rows.filter(row => !row.title.includes(CURRENT_SETTINGS_ROW));
      assert.ok(candidateRows.some(row => row.id === "response-deadline"), "The candidate's response rows are in the table");
      assert.deepStrictEqual(candidateRows.filter(row => row.verdict !== "pass").map(row => row.id), [], "Every Validate row of the candidate passes");
      assert.strictEqual(validation.rows.find(row => row.id === "overall")?.text, "Pass", "The candidate's overall row passes");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "false", "A passed validation offers Apply, which the lesson takes next");
    }, { capture: "lesson-89-validated" });
  });
});

/**
 * README step 1 of a course project: start the Simulator, unless it already runs. `withStudio` stops the debug session that is active when
 * the journey ends, unless it is the window's own, so the Simulator started here is stopped at the end.
 */
async function startSimulator(): Promise<void> {
  if ((await runtimeLifecycleService.snapshot()).status.running) return;
  const started = await runtimeLifecycleService.startLocalSimulator();
  assert.ok(started.ok, JSON.stringify(started));
}

/** The seconds at rest that README step 7's recording times count before the project's activation. */
const README_REST_SECONDS = 2;

/**
 * How much later than the README's 2 s the first active sample may lie: the recording and the experiment are two requests to the runtime, so
 * the experiment's first execution comes a scan or two after the recording's first sample. The 4.6 s wait of a steady baseline (5 % of the
 * 92.1 s) is far outside it.
 */
const ACTIVATION_LAG_SECONDS = 0.5;

/** The windows of README step 7 for the 92.1 s project test, in seconds, as the page's fields name them (`data-mimo-input`, `data-mimo-window`). */
const MIMO_WINDOWS: Array<[string, string, number]> = [
  ["heater-1", "baseline-start", 0], ["heater-1", "baseline-end", 1.9], ["heater-1", "settled-start", 27.1], ["heater-1", "settled-end", 31.9],
  ["heater-2", "baseline-start", 57.1], ["heater-2", "baseline-end", 61.9], ["heater-2", "settled-start", 87.1], ["heater-2", "settled-end", 92],
];

/** What the measured-paths section shows: the gain matrix's rows (a label, then one cell per heater), its pairing and its refusal. */
interface MimoResult { rows: string[][]; pairing: string; refusal: string }

/** The script that reads the measured-paths section: its gain matrix cells, its pairing sentence and its refusal sentence. */
const MIMO_RESULT_SCRIPT = "return JSON.stringify({rows:[...d.querySelectorAll('[data-three-page-mimo-analysis-gain] tbody tr')].map(row=>[...row.cells].map(cell=>cell.textContent)),"
  + "pairing:d.querySelector('[data-three-page-mimo-analysis-pairing]').textContent,refusal:d.querySelector('[data-three-page-mimo-analysis-refusal]').textContent});";

/** The script that opens Analyze… and reports whether its list offers `tool` as an enabled choice (the probe the three-page journeys use). */
function analyzeOffers(tool: string): string {
  return "const list=d.querySelector('[data-studio-analyze-list]');if(!list.hidden)d.querySelector('[data-studio-analyze]').click();"
    + `d.querySelector('[data-studio-analyze]').click();const option=d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool=${tool}]');return String(!!option&&!option.disabled);`;
}

/**
 * The README's Requirements for lesson 89 (step 5): the setpoint change, the error limit, the settling time and band, the validation duration
 * and the Auto-entry tolerance. The page names the setpoint change "Test setpoint change", and the editor's keys are these.
 */
const LESSON_89_REQUIREMENTS: Record<string, number> = { referenceChange: 1, maximumError: 0.02, settlingTimeSeconds: 4, settlingBand: 0.01, durationSeconds: 8, transferTolerance: 1.5 };

/** The script that types each value into the requirements editor by its field key, then presses Done. */
function requirementsScript(values: Record<string, number>): string {
  return "const set=(key,value)=>{const field=d.querySelector('[data-studio-requirement='+key+']');field.value=String(value);field.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));};"
    + `for(const [key,value] of ${JSON.stringify(Object.entries(values))})set(key,value);d.querySelector('[data-studio-editor-done]').click();return 'requirements set';`;
}

/**
 * The finite search as the Tune area shows it: the status, the title, the trials' headings and rows, whether the page is busy, and the
 * page's own message and failure detail for Tune (what a refusal says when the search does not start).
 */
interface SearchTable { busy: string; status: string; title: string; message: string; detail: string; head: string[]; rows: Array<{ selected: boolean; cells: string[] }> }

/** The script that reads the finite search's table (C5), its busy state and the Tune page's message, with the sentence of a search that did not complete. */
const SEARCH_TRIALS_SCRIPT = "const root=d.querySelector('[data-studio-three-page]');const table=d.querySelector('[data-studio-search-trials]');"
  + "return JSON.stringify({busy:root.dataset.busy??'false',status:d.querySelector('[data-studio-search-status]')?.textContent??'',"
  + "title:d.querySelector('[data-studio-tuning-result] p')?.textContent??'',message:d.querySelector('[data-studio-message=tune]')?.textContent??'',"
  + "detail:d.querySelector('[data-studio-failure-detail=tune]')?.textContent??'',"
  + "head:table?[...table.tHead.rows[0].cells].map(cell=>cell.textContent):[],"
  + "rows:table?[...table.tBodies[0].rows].map(row=>({selected:row.hasAttribute('data-studio-search-selected'),cells:[...row.cells].map(cell=>cell.textContent)})):[]});";

/**
 * The Validate table: each row's title (the engine's words, which name the run the row judges), its requirement id and its verdict, read
 * from the verdict cell that ends every row, and the Validate message.
 */
interface ValidateRows { busy: string; message: string; rows: Array<{ title: string; id: string; verdict: string; text: string }> }

/**
 * The words that name the run a row judges when it judges the current settings' run (`CURRENT_SETTINGS` of `validation/texts.rs`), followed by
 * the separator of the title; a scenario's own words may come before them when the campaign has several scenarios.
 */
const CURRENT_SETTINGS_ROW = "Current settings · ";

/** The script that reads the Validate table's rows, whether the page is busy and the Validate page's message. */
const VALIDATION_ROWS_SCRIPT = "return JSON.stringify({busy:d.querySelector('[data-studio-three-page]').dataset.busy??'false',"
  + "message:d.querySelector('[data-studio-message=verify]')?.textContent??'',"
  + "rows:[...d.querySelectorAll('[data-studio-validation-result] tbody tr')].map(row=>({title:row.cells[0]?.textContent??'',id:row.dataset.studioRequirement,verdict:row.lastElementChild?.dataset.verdict??'',text:row.lastElementChild?.textContent??''}))});";
