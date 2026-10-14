import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { decodeEngineerStudy, type EngineerStudy } from "../../controlStudio/engineerStudyPersistence";
import { draftStorageKey } from "../../controlStudio/engineerDraftStorage";
import { navigationStorageKey, storeEngineerStage } from "../../controlStudio/engineerNavigationStorage";
import { mimoAnalysisStorageKey, workflowResultStorageKey } from "../../controlStudio/panelPersistence";
import { openWorkspaceStudy, studyPointerKey } from "../../controlStudio/studyMigration";
import {
  openProjectStudy, projectStudyState, startNewWorkingStudy, studyDirectory, studyFolderName, updateProjectStudy, writeFileAtomic,
} from "../../controlStudio/studyStore";
import { compactRecordedRun, inlineRecordings, recordingIdFor } from "../../controlStudio/recordingFile";
import { STUDY_STORAGE_LIMIT_BYTES } from "../../controlStudio/controlStudioLimits";
import {
  experimentRecordingJournal, journalBeforeDispatch, testStartPorts, UNPINNED_RUNTIME_MESSAGE, UnconfirmedTestStarts,
} from "../../controlStudio/testExperimentGuards";
import { startExperimentWithRecording, type ExperimentRecordingOwner } from "../../controlStudio/experimentRecording";

suite("Control Studio project study store", () => {
  const baseTask = { referenceChange: 1, maximumError: 0.2, settlingBand: 0.05, settlingTimeSeconds: 1, durationSeconds: 3 };
  const channelTask = { referenceChange: 1, maximumError: 0.2, settlingBand: 0.05, settlingTimeSeconds: 1 };

  /** A loop definition with the signals and outputs the test declares; only the fields the store reads are present. */
  function loopAt(revision: string, additional: string[] = ["sigA"], outputs: string[] = ["out1"]): ControlLoopDefinition {
    return {
      projectId: "project", runtimeId: "sim", id: "loop-1", revision,
      signals: {
        processVariable: { symbol: "pv", unit: "degC" },
        manipulatedVariable: { symbol: "mv", unit: "%" },
        additional: additional.map(symbol => ({ symbol, role: "MV", unit: "%" })),
      },
      channels: { inputs: [], outputs: outputs.map(modelChannelId => ({ modelChannelId, signalId: "pv" })) },
    } as unknown as ControlLoopDefinition;
  }

  function tempRoot(): string {
    return fs.mkdtempSync(path.join(os.tmpdir(), "control-studio-study-"));
  }

  /** A recorded run in the extension's own per-channel form, with one missing and one good value. */
  function sampleRun(): Record<string, unknown> {
    return {
      id: "run-1", evidence: "ST-executed", parameters: {}, conditions: { seed: 1 },
      channels: [{ id: "temperature", unit: "degC" }, { id: "heater", unit: "%" }],
      samples: [
        { timeSeconds: 0, sequence: 0, values: { temperature: { value: 20, quality: "good" }, heater: { value: null, quality: "missing" } } },
        { timeSeconds: 0.1, sequence: 1, values: { temperature: { value: 20.5, quality: "good" }, heater: { value: 40, quality: "good" } } },
      ],
    };
  }

  function studyWithLimits(): EngineerStudy {
    return {
      campaigns: [],
      task: { ...baseTask, channelRequirements: { out1: channelTask, "out-gone": channelTask } },
      acquisitionSettings: { method: "mv_step", amplitude: 1, durationSeconds: 3, reuseRecordings: true,
        setpointSignalId: "sigGone", inputRanges: { sigA: { minimum: 0, maximum: 100 }, sigGone: { minimum: 0, maximum: 50 } } },
    } as unknown as EngineerStudy;
  }

  /** Every file under a folder, keyed by its relative path, with its exact content. */
  function snapshot(directory: string): Record<string, string> {
    const files: Record<string, string> = {};
    const visit = (current: string): void => {
      for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
        const file = path.join(current, entry.name);
        if (entry.isDirectory()) visit(file);
        else files[path.relative(directory, file)] = fs.readFileSync(file, "utf8");
      }
    };
    visit(directory);
    return files;
  }

  function bytesUnder(directory: string): number {
    return Object.values(snapshot(directory)).reduce((sum, text) => sum + Buffer.byteLength(text, "utf8"), 0);
  }

  /** A workspace memento for the migration tests, with the same get and update contract VS Code provides. */
  function memento(initial: Record<string, unknown>) {
    const values = new Map<string, unknown>(Object.entries(initial));
    return {
      values,
      get: (key: string): unknown => values.get(key),
      update: async (key: string, value: unknown): Promise<void> => {
        if (value === undefined) values.delete(key);
        else values.set(key, value);
      },
    };
  }

  test("a page change for a loop whose study is not open is kept for the session and raises nothing (A8.3)", async () => {
    const loop = loopAt("not-open");
    const scope = { projectId: loop.projectId, editorSessionId: "editor", loopId: loop.id, loopRevision: loop.revision };
    const message = { kind: "control-studio.navigation.select", schemaVersion: 1, projectId: loop.projectId, editorSessionId: "editor",
      loopId: loop.id, expectedLoopRevision: loop.revision, payload: { stage: "tune" } };
    assert.strictEqual(await storeEngineerStage(loop, message, scope as never), true, "the selection is handled");
  });

  test("writes one study document and one compact recording file under control-studio/<loop id>", async () => {
    const root = tempRoot();
    const loop = loopAt("sha256:v1");
    await openProjectStudy(root, loop);
    await updateProjectStudy(loop, "draft", { loopRevision: loop.revision, study: { campaigns: [], tuningRuns: [{ run: sampleRun() }] } });
    const directory = studyDirectory(root, loop.id);
    const document = JSON.parse(fs.readFileSync(path.join(directory, "study.json"), "utf8"));
    assert.strictEqual(document.schemaVersion, 1);
    assert.strictEqual(document.loop.bindingRevision, "sha256:v1");
    const recordings = fs.readdirSync(path.join(directory, "recordings"));
    assert.strictEqual(recordings.length, 1, "one recorded run is one recording file");
    const reference = document.state.draft.study.tuningRuns[0].run;
    assert.deepStrictEqual(reference, { $recording: recordings[0].replace(/\.json$/, "") }, "the study document refers to the recording instead of holding its samples");
    const file = JSON.parse(fs.readFileSync(path.join(directory, "recordings", recordings[0]), "utf8"));
    assert.deepStrictEqual(file.run.samples[0].values, [20, null], "values are kept in channel order");
    assert.deepStrictEqual(file.run.samples[0].quality, [0, 3], "quality codes appear where a value is not good");
    assert.strictEqual("quality" in file.run.samples[1], false, "a sample whose values are all good has no quality array");
    assert.deepStrictEqual(file.run.channels, [{ id: "temperature", unit: "degC" }, { id: "heater", unit: "%" }]);
  });

  test("a recorded run round-trips through the compact wire form", () => {
    const run = sampleRun();
    const compact = compactRecordedRun(run);
    assert.deepStrictEqual(compact.channels, run.channels, "channel ids and units are written once");
    const samples = compact.samples as Array<Record<string, unknown>>;
    assert.deepStrictEqual(samples[0].values, [20, null]);
    assert.strictEqual("quality" in samples[1], false);
    const restored = inlineRecordings({ $recording: recordingIdFor(compact) }, () => compact);
    assert.deepStrictEqual(restored, run, "the expanded run equals the run the study holds");
  });

  test("a recording file that already exists is never rewritten", async () => {
    const root = tempRoot();
    const loop = loopAt("sha256:v1");
    await openProjectStudy(root, loop);
    await updateProjectStudy(loop, "draft", { loopRevision: loop.revision, study: { campaigns: [], tuningRuns: [{ run: sampleRun() }] } });
    const directory = path.join(studyDirectory(root, loop.id), "recordings");
    const [file] = fs.readdirSync(directory).map(name => path.join(directory, name));
    fs.appendFileSync(file, "\n// kept as written\n");
    await updateProjectStudy(loop, "draft", { loopRevision: loop.revision, study: { campaigns: [], tuningRuns: [{ run: sampleRun() }] } });
    assert.ok(fs.readFileSync(file, "utf8").endsWith("// kept as written\n"), "a later save of the same recording leaves its file unchanged");
  });

  test("an atomic write replaces the file whole and leaves no temporary file behind", async () => {
    const root = tempRoot();
    const file = path.join(root, "nested", "file.json");
    await writeFileAtomic(file, "one");
    await writeFileAtomic(file, "two");
    assert.strictEqual(fs.readFileSync(file, "utf8"), "two");
    assert.deepStrictEqual(fs.readdirSync(path.dirname(file)), ["file.json"]);
  });

  test("a failed atomic write keeps the previous content and leaves no temporary file behind", async () => {
    const root = tempRoot();
    const file = path.join(root, "target.json");
    fs.mkdirSync(file);
    await assert.rejects(() => writeFileAtomic(file, "data"));
    assert.deepStrictEqual(fs.readdirSync(root), ["target.json"], "the failed write leaves only the existing entry");
    assert.ok(fs.statSync(file).isDirectory(), "the existing entry is untouched");
  });

  test("folder names keep safe loop ids and add a hash only where the id changes", () => {
    assert.strictEqual(studyFolderName("loop-1.a_b"), "loop-1.a_b");
    assert.match(studyFolderName("pump/1 main"), /^pump_1_main-[0-9a-f]{8}$/);
    assert.match(studyFolderName(".."), /^loop-[0-9a-f]{8}$/, "dot-only names are never used as folder names");
    assert.notStrictEqual(studyFolderName("a b"), studyFolderName("a_b"), "different ids keep different folders");
  });

  test("a changed binding revision keeps the old study as history and carries over only signals that still exist", async () => {
    const root = tempRoot();
    const v1 = loopAt("sha256:v1", ["sigA", "sigGone"], ["out1", "out-gone"]);
    await openProjectStudy(root, v1);
    await updateProjectStudy(v1, "draft", { loopRevision: v1.revision, study: studyWithLimits() });
    const v2 = loopAt("sha256:v2", ["sigA"], ["out1"]);
    await openProjectStudy(root, v2);
    const directory = studyDirectory(root, v2.id);
    const working = JSON.parse(fs.readFileSync(path.join(directory, "study.json"), "utf8"));
    assert.strictEqual(working.loop.bindingRevision, "sha256:v2");
    const history = fs.readdirSync(path.join(directory, "history"));
    assert.strictEqual(history.length, 1, "the earlier study is kept as one history folder");
    const archived = JSON.parse(fs.readFileSync(path.join(directory, "history", history[0], "study.json"), "utf8"));
    assert.strictEqual(archived.loop.bindingRevision, "sha256:v1", "the history keeps the revision it was written for");
    const carried = projectStudyState(v2).draft as { loopRevision: string; study: any };
    assert.strictEqual(carried.loopRevision, "sha256:v2");
    assert.deepStrictEqual(Object.keys(carried.study.task.channelRequirements), ["out1"], "a requirement whose output is gone is dropped");
    assert.deepStrictEqual(Object.keys(carried.study.acquisitionSettings.inputRanges), ["sigA"], "a limit whose signal is gone is dropped");
    assert.strictEqual("setpointSignalId" in carried.study.acquisitionSettings, false, "a setpoint that is gone is dropped");
    assert.strictEqual(carried.study.acquisitionSettings.method, "mv_step", "the settings that name no signal are carried over");
  });

  test("the overshoot limit of a task and of its outputs survives a study round trip", async () => {
    const root = tempRoot();
    const loop = loopAt("sha256:v1");
    await openProjectStudy(root, loop);
    const study = { campaigns: [], task: { ...baseTask, maximumOvershootPercent: 5,
      channelRequirements: { out1: { ...channelTask, maximumOvershootPercent: 4 } } } };
    await updateProjectStudy(loop, "draft", { loopRevision: loop.revision, study });
    const reopened = await openProjectStudy(root, loop);
    const decoded = decodeEngineerStudy((reopened.draft as { study: unknown }).study);
    assert.ok(decoded, "the saved study decodes after reopening");
    assert.strictEqual(decoded!.task!.maximumOvershootPercent, 5, "the task limit is kept");
    assert.strictEqual((decoded!.task!.channelRequirements!.out1 as unknown as { maximumOvershootPercent?: number }).maximumOvershootPercent, 4,
      "the output limit is kept");
    const older = decodeEngineerStudy({ campaigns: [], task: { ...baseTask } });
    assert.ok(older && older.task, "a study saved before the limit existed still loads");
    assert.strictEqual("maximumOvershootPercent" in older!.task!, false, "an absent limit declares none");
  });

  test("Reset keeps the previous working study as history and starts an empty one", async () => {
    const root = tempRoot();
    const loop = loopAt("sha256:v1");
    await openProjectStudy(root, loop);
    await updateProjectStudy(loop, "draft", { loopRevision: loop.revision, study: studyWithLimits() });
    await updateProjectStudy(loop, "page", "tune");
    await startNewWorkingStudy(loop, { draft: { loopRevision: loop.revision, study: { campaigns: [] } } });
    const directory = studyDirectory(root, loop.id);
    const history = fs.readdirSync(path.join(directory, "history"));
    assert.strictEqual(history.length, 1);
    const archived = JSON.parse(fs.readFileSync(path.join(directory, "history", history[0], "study.json"), "utf8"));
    assert.strictEqual(archived.state.page, "tune", "the previous page is kept with the previous study");
    assert.ok(archived.state.draft.study.task, "the previous requirements are kept with the previous study");
    assert.deepStrictEqual(projectStudyState(loop), { draft: { loopRevision: loop.revision, study: { campaigns: [] } } });
  });

  test("reopening restores the study from its files and writes nothing", async () => {
    const root = tempRoot();
    const loop = loopAt("sha256:v1");
    await openProjectStudy(root, loop);
    await updateProjectStudy(loop, "draft", { loopRevision: loop.revision, study: { campaigns: [], tuningRuns: [{ run: sampleRun() }] } });
    const before = snapshot(path.join(root, "control-studio"));
    const reopened = await openProjectStudy(root, loop);
    assert.deepStrictEqual(snapshot(path.join(root, "control-studio")), before, "opening an unchanged study writes no file");
    const restoredRun = (reopened.draft as any).study.tuningRuns[0].run;
    assert.deepStrictEqual(restoredRun.samples[0].values.heater, { value: null, quality: "missing" }, "the recording is expanded on reopening");
  });

  test("reopening waits for the write the closing panel queued, so it reads the study as last saved (A8.6)", async () => {
    const root = tempRoot();
    const loop = loopAt("sha256:v1");
    await openProjectStudy(root, loop);
    // The closing panel queues its final draft and does not wait for it; the reopening panel opens the study at once.
    const closing = updateProjectStudy(loop, "draft", { loopRevision: loop.revision, study: { campaigns: [], tuningRuns: [{ run: sampleRun() }] } });
    const reopened = await openProjectStudy(root, loop);
    await closing;
    assert.ok((reopened.draft as any)?.study?.tuningRuns?.length === 1, "the reopened study holds the draft the closing panel saved");
  });

  test("migration moves the workspace study into the project and clears the old place only after writing", async () => {
    const root = tempRoot();
    const loop = loopAt("sha256:v1");
    const old = memento({
      [draftStorageKey(loop)]: { loopRevision: loop.revision, study: { campaigns: [] } },
      [workflowResultStorageKey(loop)]: [],
      [navigationStorageKey(loop)]: "verify",
      [mimoAnalysisStorageKey(loop)]: { loopRevision: "sha256:other", snapshot: { result: {} } },
    });
    await openWorkspaceStudy(old, root, loop);
    const state = projectStudyState(loop);
    assert.deepStrictEqual(state.draft, { loopRevision: loop.revision, study: { campaigns: [] } });
    assert.deepStrictEqual(state.history, []);
    assert.strictEqual(state.page, "verify");
    assert.strictEqual(state.mimo, undefined, "an analysis of another binding revision is kept as history, not as the working study");
    assert.ok(fs.existsSync(path.join(studyDirectory(root, loop.id), "history")), "the other revision's analysis is in history");
    assert.strictEqual(old.get(draftStorageKey(loop)), undefined, "the workspace copy is cleared once it is in the project");
    assert.strictEqual(old.get(navigationStorageKey(loop)), undefined);
    assert.strictEqual(old.get(mimoAnalysisStorageKey(loop)), undefined);
    assert.ok(old.get(studyPointerKey(loop)), "the workspace state keeps the pointer to the current study");
  });

  test("a migration whose project write fails keeps the workspace copy", async () => {
    const root = tempRoot();
    const loop = loopAt("sha256:v1");
    fs.writeFileSync(path.join(root, "control-studio"), "a file where the study folder belongs");
    const old = memento({ [draftStorageKey(loop)]: { loopRevision: loop.revision, study: { campaigns: [] } } });
    await assert.rejects(() => openWorkspaceStudy(old, root, loop));
    assert.ok(old.get(draftStorageKey(loop)) !== undefined, "nothing is lost when the project cannot be written");
  });

  test("a study over the storage limit is refused with its size and the limit, and nothing is written", async () => {
    const root = tempRoot();
    const loop = loopAt("sha256:v1");
    assert.strictEqual(STUDY_STORAGE_LIMIT_BYTES, 256 * 1024 * 1024, "the limit is the study storage limit of A7");
    await openProjectStudy(root, loop, { storageLimitBytes: 4096 });
    await assert.rejects(() => updateProjectStudy(loop, "history", [{ padding: "x".repeat(8192) }]), (error: unknown) => {
      const message = String(error);
      assert.match(message, /above the limit of 4096 bytes/, "the message names the limit");
      assert.match(message, /The study is \d+ bytes/, "the message names the size");
      assert.match(message, /Nothing was written/);
      return true;
    });
    assert.strictEqual(fs.existsSync(path.join(studyDirectory(root, loop.id), "study.json")), false, "no study document was written");
    assert.ok(bytesUnder(root) === 0 || !fs.existsSync(path.join(root, "control-studio")), "no folder was written either");
  });

  test("a Test identity is journalled before dispatch, and a failed journal never dispatches", async () => {
    const order: string[] = [];
    await journalBeforeDispatch(async () => { order.push("journal"); }, async () => { order.push("dispatch"); });
    assert.deepStrictEqual(order, ["journal", "dispatch"], "the identity is written before anything is sent");
    const sent: string[] = [];
    await assert.rejects(() => journalBeforeDispatch(async () => { throw new Error("journal full"); }, async () => { sent.push("dispatch"); }));
    assert.deepStrictEqual(sent, [], "nothing is dispatched when the identity cannot be journalled");
  });

  test("an identity whose start was not confirmed is never sent again", () => {
    const starts = new UnconfirmedTestStarts();
    starts.markUnconfirmed("experiment-1");
    assert.throws(() => starts.assertSendable("experiment-1"), /not confirmed/);
    starts.assertSendable("experiment-2");
    starts.confirm("experiment-1");
    starts.assertSendable("experiment-1");
  });

  test("the Test page starts only with its pinned runtime target and its Test journal", () => {
    const target = { runtimeId: "sim", endpoint: "tcp://127.0.0.1:4000" };
    const journal = experimentRecordingJournal({ persist: async () => undefined, clearOwner: async () => undefined });
    assert.throws(() => testStartPorts(undefined, journal), (error: unknown) => String(error).includes(UNPINNED_RUNTIME_MESSAGE),
      "without the target pinned at open, no start is made");
    assert.throws(() => testStartPorts(target, undefined), /journal/, "without a journal, no start is made");
    const ports = testStartPorts(target, journal);
    assert.strictEqual(ports.target, target, "the start uses the target pinned when the loop opened");
    assert.strictEqual(ports.journal, journal, "the recording is journalled through the Test journal");
  });

  test("the Test journal forwards the recording owner to persist and clears only the owner", async () => {
    const events: string[] = [];
    const journal = experimentRecordingJournal({
      persist: async () => { events.push("persist"); },
      clearOwner: async () => { events.push("clear"); },
    });
    await journal.persist({} as never);
    await journal.clear({} as never);
    assert.deepStrictEqual(events, ["persist", "clear"]);
  });

  test("a Test recording is refused without its journal and starts once a journal is given", async () => {
    const owner = {
      recording: undefined, recoveryOwner: undefined,
      start: async () => ({ kind: "started" }),
      stop: async () => undefined,
    } as unknown as ExperimentRecordingOwner;
    const loop = loopAt("sha256:v1");
    let sent = 0;
    await assert.rejects(() => startExperimentWithRecording(owner, loop, async () => { sent += 1; return "started"; }, undefined), /journal/,
      "the recording gate refuses to dispatch without a journal");
    assert.strictEqual(sent, 0, "no request reaches the experiment without a journal");
    const journal = experimentRecordingJournal({ persist: async () => undefined, clearOwner: async () => undefined });
    assert.strictEqual(await startExperimentWithRecording(owner, loop, async () => { sent += 1; return "started"; }, journal), "started");
    assert.strictEqual(sent, 1, "the experiment is started once, through the journal");
  });

  test("keeps the TypeScript session's study beside the engine's study of the loop, never in its files (A8.3, §0.1a)", async () => {
    // The engine keeps the loop's study document and recordings in control-studio/<loop id>/; the TypeScript session keeps what it
    // still owns in its own folder there, so writes, a reset and a new binding revision of either leave the other's files as they were.
    const root = tempRoot();
    const loop = loopAt("sha256:v1");
    const loopFolder = path.join(root, "control-studio", studyFolderName(loop.id));
    const directory = studyDirectory(root, loop.id);
    assert.strictEqual(path.dirname(directory), loopFolder, "the TypeScript session's folder is inside the loop's study folder");
    assert.notStrictEqual(directory, loopFolder, "the TypeScript session never uses the engine's folder itself");
    fs.mkdirSync(path.join(loopFolder, "recordings"), { recursive: true });
    const engineStudy = `${JSON.stringify({ format: 1, loopId: loop.id, loopRevision: "sha256:v1", sections: { identify: { kept: true } }, recordings: ["r1"] }, null, 2)}\n`;
    const engineRecording = `${JSON.stringify({ format: 1, id: "r1" })}\n`;
    fs.writeFileSync(path.join(loopFolder, "study.json"), engineStudy);
    fs.writeFileSync(path.join(loopFolder, "recordings", "r1.json"), engineRecording);
    const engineFiles = (): string[] => [fs.readFileSync(path.join(loopFolder, "study.json"), "utf8"),
      fs.readFileSync(path.join(loopFolder, "recordings", "r1.json"), "utf8")];
    assert.deepStrictEqual(await openProjectStudy(root, loop), {}, "the engine's study is not the TypeScript session's");
    await updateProjectStudy(loop, "draft", { campaigns: [] });
    assert.ok(fs.existsSync(path.join(directory, "study.json")), "the TypeScript session writes its own document");
    assert.deepStrictEqual(engineFiles(), [engineStudy, engineRecording], "a TypeScript write leaves the engine's files as they were");
    assert.deepStrictEqual((await openProjectStudy(root, loop)).draft, { campaigns: [] }, "reopening reads the TypeScript session's document back");
    await startNewWorkingStudy(loop, {});
    assert.deepStrictEqual(engineFiles(), [engineStudy, engineRecording], "a TypeScript reset archives only the TypeScript session's study");
    assert.strictEqual(fs.existsSync(path.join(loopFolder, "history")), false, "nothing of the engine's moves to history");
    await openProjectStudy(root, loopAt("sha256:v2"));
    assert.deepStrictEqual(engineFiles(), [engineStudy, engineRecording], "a new binding revision leaves the engine's study to the engine");
  });
});
