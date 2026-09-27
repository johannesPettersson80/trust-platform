import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { controlModuleId } from "../../examples/model";
import { openRenderedExamplesGallery } from "./examples-rendered-harness";
import { captureLessonMarkdownPreview } from "./examples-markdown-preview";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, httpJson, pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";

type TestReport = { summary: { total: number; passed: number; failed: number; errors: number }; tests: Array<{ name: string; status: string; message?: string }> };

suite("Control course learner step calculations", () => {
  test("creates the learner project through the lesson gallery", async function () {
    if (process.env.TRUST_STEP_CALCULATIONS_CREATE !== "1") this.skip();
    this.timeout(30000);
    const captures = requiredEnvironment("TRUST_STEP_CALCULATIONS_CAPTURE");
    fs.mkdirSync(captures, { recursive: true });
    const destination = fs.mkdtempSync(path.join(captures, "projects-"));
    const project = path.join(destination, "step-calculations");
    const lessonId = process.env.TRUST_STEP_CALCULATIONS_LESSON ?? "control-lesson-23";
    const overrides = { TRUST_UX_EXAMPLE_DESTINATION: destination, TRUST_UX_EXAMPLE_NAME: "step-calculations", TRUST_UX_EXAMPLE_OPEN_FOLDER: "0" };
    const previous = Object.fromEntries(Object.keys(overrides).map(key => [key, process.env[key]]));
    Object.assign(process.env, overrides);
    const gallery = await openRenderedExamplesGallery();
    try {
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const click = async (action: string, id?: string) => pollValue(action, () => gallery.inner(
        `var b=[...d.querySelectorAll('button')].find(x=>x.dataset.actionType===${JSON.stringify(action)}${id ? `&&x.dataset.actionId===${JSON.stringify(id)}` : ""});if(!b)return 'WAIT';b.click();return 'CLICKED';`), value => value === "CLICKED", 10000);
      await click("openControlCurriculum");
      await click("openControlCourse", controlModuleId("control/05-experiments-identification-and-validation/23-step-response-identification"));
      await click("openControlLesson", lessonId);
      await click("useExample", lessonId);
      await pollValue("copied lesson project", async () => fs.existsSync(path.join(project, "README.md")), Boolean, 10000);
      fs.writeFileSync(path.join(captures, "created-project.json"), JSON.stringify({ project, lesson: lessonId }, null, 2));
      await sleep(350); await gallery.capture(path.join(captures, "00-created-project.png"));
    } finally {
      gallery.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      for (const key of Object.keys(overrides)) { if (previous[key] === undefined) delete process.env[key]; else process.env[key] = previous[key]; }
    }
  });

  test("executes the published guarded calculations in the opened lesson project", async function () {
    if (process.env.TRUST_STEP_CALCULATIONS_RENDERED !== "1") this.skip();
    this.timeout(60000);
    const captures = requiredEnvironment("TRUST_STEP_CALCULATIONS_CAPTURE");
    const project = requiredEnvironment("TRUST_STEP_PROJECT");
    fs.mkdirSync(captures, { recursive: true });
    assert.deepStrictEqual(vscode.workspace.workspaceFolders?.map(folder => folder.uri.fsPath), [project],
      "open the gallery-created project as the actual workspace, not loose files in another workspace");
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const devConfig = vscode.workspace.getConfiguration("trust");
    const previousDev = devConfig.inspect<string>("testRunner.executablePath")?.workspaceValue;
    await devConfig.update("testRunner.executablePath", requiredEnvironment("ST_DEV_TEST_BIN"), vscode.ConfigurationTarget.Workspace);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const capture = (name: string) => captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
    try {
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      const readme = fs.readFileSync(path.join(project, "README.md"), "utf8");
      assert.match(readme, /Model structure \(to open it, use truST > Start from example and select Lesson 16\)/);
      const brokenReferences = [...readme.matchAll(/\[[^\]]+\]\(([^)]+)\)/g)]
        .map(match => match[1]).filter(target => !/^(?:[a-z]+:|#)/i.test(target))
        .filter(target => !fs.existsSync(path.resolve(project, target.split("#")[0])));

      const measured = JSON.parse(fs.readFileSync(requiredEnvironment("TRUST_STEP_MEASUREMENTS"), "utf8"));
      const estimation = measured.preview.estimationQualification.stepResponse;
      const validation = measured.validation.validationQualification.stepResponse;
      assert.strictEqual(estimation.baseline.mean, 35); assert.strictEqual(estimation.inputChange, 12);
      assert.strictEqual(estimation.final.mean, 37.36572102459306);
      assert.strictEqual(validation.inputChange, 10); assert.strictEqual(validation.final.mean, 36.998577791923196);
      fs.copyFileSync(requiredEnvironment("TRUST_STEP_MEASUREMENTS"), path.join(captures, "qualified-measurements.json"));

      const writeInEditor = async (name: string, content: string) => {
        const file = path.join(project, "src", name);
        if (!fs.existsSync(file)) fs.writeFileSync(file, "");
        const document = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
        const editor = await vscode.window.showTextDocument(document, { preview: false });
        assert.strictEqual(await editor.edit(edit => edit.replace(new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length)), content)), true);
        assert.strictEqual(await document.save(), true);
        return editor;
      };
      const tests = learnerTests(estimation.final.mean, validation.final.mean);
      await writeInEditor("LearnerTests.st", tests);
      const stub = declarations + "(* Enter the guarded calculations here. *)\nEND_FUNCTION_BLOCK\n";
      await writeInEditor("StepCalculations.st", stub);
      const testDocument = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(project, "src", "LearnerTests.st")));
      const helperPosition = testDocument.positionAt(tests.indexOf("Lesson23ProcessGain") + 2);
      await pollValue("shipped helper resolved in copied project", async () => {
        const definitions = await vscode.commands.executeCommand<Array<vscode.Location | vscode.LocationLink>>(
          "vscode.executeDefinitionProvider", testDocument.uri, helperPosition);
        return definitions?.some(definition => ("uri" in definition ? definition.uri : definition.targetUri).fsPath === path.join(project, "src", "Main.st"));
      }, Boolean, 10000);
      const run = async (name: string) => {
        const report = await vscode.commands.executeCommand<TestReport>("trust-lsp.test.runAll", { projectUri: vscode.Uri.file(project).toString() });
        assert.ok(report, "the public ST test command must return its actual native report");
        fs.writeFileSync(path.join(captures, `${name}.json`), JSON.stringify(report, null, 2));
        return report;
      };
      const red = await run("unfinished-calculations");
      assert.deepStrictEqual(red.summary, { ...red.summary, total: 7, passed: 4, failed: 3, errors: 0 });
      assert.ok(red.tests.filter(test => test.status === "failed").every(test => test.name.startsWith("Lesson15Learner")), "the unfinished learner code must be the only failure");
      await writeInEditor("LearnerTests.st", tests);
      await sleep(350); await capture("01-unfinished-calculations");

      const editor = await writeInEditor("StepCalculations.st", declarations + calculations + "END_FUNCTION_BLOCK\n");
      await vscode.commands.executeCommand("notifications.clearAll");
      editor.revealRange(new vscode.Range(0, 0, 0, 0), vscode.TextEditorRevealType.AtTop);
      await sleep(350); await capture("02-learner-declarations");
      editor.revealRange(new vscode.Range(20, 0, 20, 0), vscode.TextEditorRevealType.AtTop);
      await sleep(350); await capture("03-guarded-calculations");
      const green = await run("completed-calculations");
      assert.deepStrictEqual(green.summary, { ...green.summary, total: 7, passed: 7, failed: 0, errors: 0 });
      const diagnostics = vscode.languages.getDiagnostics().filter(([uri]) => uri.fsPath.startsWith(project));
      fs.writeFileSync(path.join(captures, "editor-diagnostics.json"), JSON.stringify(diagnostics, null, 2));
      assert.ok(diagnostics.every(([, items]) => items.every(item => item.severity !== vscode.DiagnosticSeverity.Error)),
        "the actual copied project must resolve its declarations without editor errors");
      assert.ok(diagnostics.every(([, items]) => items.every(item => item.code !== "W009")),
        "test-runner entry points must not appear as unused production programs");
      const testEditor = await writeInEditor("LearnerTests.st", tests);
      await sleep(350); await capture("04-measured-calculations-pass");
      testEditor.revealRange(new vscode.Range(28, 0, 28, 0), vscode.TextEditorRevealType.AtTop);
      await sleep(350); await capture("05-invalid-input-clears-gain");
      await vscode.commands.executeCommand("notifications.clearAll");
      await captureLessonMarkdownPreview(path.join(project, "README.md"), path.join(captures, "06-copied-implementation"),
        { readyText: "ENO", heading: "Implementation" });
      fs.writeFileSync(path.join(captures, "exercise.json"), JSON.stringify({ project, source: requiredEnvironment("TRUST_STEP_MEASUREMENTS"), brokenReferences,
        estimation: { commandChange: 12, temperatureChange: 2.36572102459306, gain: .1971434187160881, temperature63: 36.49513568754281 },
        validation: { commandChange: 10, gain: .1998577791923196, temperature63: 36.26310116449546 } }, null, 2));
      assert.deepStrictEqual(brokenReferences, [], "a copied lesson must not leave unusable reference links");
    } finally {
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      connection.close();
      await devConfig.update("testRunner.executablePath", previousDev, vscode.ConfigurationTarget.Workspace);
    }
  });
});

const declarations = `FUNCTION_BLOCK StepCalculations
VAR_INPUT
    BaselineCommandPct : LREAL;
    FinalCommandPct : LREAL;
    BaselineTemperatureC : LREAL;
    FinalTemperatureC : LREAL;
    RecordQualified : BOOL;
END_VAR
VAR_OUTPUT
    CommandChangePct : LREAL;
    TemperatureChangeC : LREAL;
    ProcessGainCPerPoint : LREAL;
    Temperature63C : LREAL;
    GainValid : BOOL;
END_VAR
`;

const calculations = `(* These values come from qualified recorded windows, not live PV. *)
CommandChangePct := FinalCommandPct - BaselineCommandPct;
TemperatureChangeC := FinalTemperatureC - BaselineTemperatureC;
GainValid := FALSE;
ProcessGainCPerPoint := LREAL#0.0;
Temperature63C := LREAL#0.0;
IF RecordQualified AND ABS(CommandChangePct) > LREAL#0.0 THEN
    ProcessGainCPerPoint := TemperatureChangeC / CommandChangePct;
    Temperature63C := BaselineTemperatureC
        + LREAL#0.632 * TemperatureChangeC;
    GainValid := TRUE;
END_IF;
`;

function learnerTests(finalEstimation: number, finalValidation: number): string {
  return `(* @trustlsp:runtime-ignore — arithmetic exercise, not scheduled plant I/O. *)
TEST_PROGRAM Lesson15LearnerImportedRecord
VAR Calc : StepCalculations; HelperGain : LREAL; HelperValid : BOOL; END_VAR
Calc(BaselineCommandPct := LREAL#75, FinalCommandPct := LREAL#87,
    BaselineTemperatureC := LREAL#35, FinalTemperatureC := LREAL#${finalEstimation}, RecordQualified := TRUE);
ASSERT_TRUE(Calc.GainValid);
ASSERT_NEAR(Calc.CommandChangePct, LREAL#12, LREAL#0.000001);
ASSERT_NEAR(Calc.TemperatureChangeC, LREAL#2.36572102459306, LREAL#0.000001);
ASSERT_NEAR(Calc.ProcessGainCPerPoint, LREAL#0.1971434187160881, LREAL#0.000001);
ASSERT_NEAR(Calc.Temperature63C, LREAL#36.49513568754281, LREAL#0.000001);
HelperGain := Lesson23ProcessGain(InitialInput := LREAL#75, FinalInput := LREAL#87,
    InitialOutput := LREAL#35, FinalOutput := LREAL#${finalEstimation}, ENO => HelperValid);
ASSERT_TRUE(HelperValid);
ASSERT_NEAR(HelperGain, Calc.ProcessGainCPerPoint, LREAL#0.000001);
END_TEST_PROGRAM

TEST_PROGRAM Lesson15LearnerIndependentRecord
VAR Calc : StepCalculations; END_VAR
Calc(BaselineCommandPct := LREAL#75, FinalCommandPct := LREAL#85,
    BaselineTemperatureC := LREAL#35, FinalTemperatureC := LREAL#${finalValidation}, RecordQualified := TRUE);
ASSERT_TRUE(Calc.GainValid);
ASSERT_NEAR(Calc.CommandChangePct, LREAL#10, LREAL#0.000001);
ASSERT_NEAR(Calc.ProcessGainCPerPoint, LREAL#0.1998577791923196, LREAL#0.000001);
ASSERT_NEAR(Calc.Temperature63C, LREAL#36.26310116449546, LREAL#0.000001);
END_TEST_PROGRAM

TEST_PROGRAM Lesson15LearnerZeroStepClearsPreviousGain
VAR Calc : StepCalculations; END_VAR
Calc(BaselineCommandPct := LREAL#75, FinalCommandPct := LREAL#85,
    BaselineTemperatureC := LREAL#35, FinalTemperatureC := LREAL#37, RecordQualified := TRUE);
ASSERT_TRUE(Calc.GainValid);
Calc(FinalCommandPct := LREAL#75);
ASSERT_FALSE(Calc.GainValid);
ASSERT_NEAR(Calc.ProcessGainCPerPoint, LREAL#0, LREAL#0.000001);
Calc(FinalCommandPct := LREAL#65);
ASSERT_TRUE(Calc.GainValid);
ASSERT_NEAR(Calc.ProcessGainCPerPoint, LREAL#-0.2, LREAL#0.000001);
END_TEST_PROGRAM

TEST_PROGRAM Lesson15LearnerUnqualifiedRecord
VAR Calc : StepCalculations; END_VAR
Calc(BaselineCommandPct := LREAL#75, FinalCommandPct := LREAL#85,
    BaselineTemperatureC := LREAL#35, FinalTemperatureC := LREAL#37, RecordQualified := FALSE);
ASSERT_FALSE(Calc.GainValid);
ASSERT_NEAR(Calc.ProcessGainCPerPoint, LREAL#0, LREAL#0.000001);
END_TEST_PROGRAM
`;
}

suite("Control course model structure calculations", () => {
  test("runs the published model-calculation exercise in the copied lesson project", async function () {
    if (process.env.TRUST_MODEL_CALCULATIONS_RENDERED !== "1") this.skip();
    this.timeout(60000);
    const project = requiredEnvironment("TRUST_STEP_PROJECT");
    const captures = requiredEnvironment("TRUST_STEP_CALCULATIONS_CAPTURE");
    fs.mkdirSync(captures, { recursive: true });
    assert.deepStrictEqual(vscode.workspace.workspaceFolders?.map(folder => folder.uri.fsPath), [project]);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const config = vscode.workspace.getConfiguration("trust");
    const previousDev = config.inspect<string>("testRunner.executablePath")?.workspaceValue;
    await config.update("testRunner.executablePath", requiredEnvironment("ST_DEV_TEST_BIN"), vscode.ConfigurationTarget.Workspace);
    const file = path.join(project, "src/tests.st");
    const original = fs.readFileSync(file, "utf8");
    const lessonId = JSON.parse(fs.readFileSync(path.join(project, "lesson.json"), "utf8")).id;
    assert.ok(["control-lesson-24", "control-lesson-25"].includes(lessonId));
    const identifiability = lessonId === "control-lesson-25";
    const document = await vscode.workspace.openTextDocument(file);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const write = async (text: string) => {
      const editor = await vscode.window.showTextDocument(document);
      assert.ok(await editor.edit(edit => edit.replace(new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length)), text)));
      await document.save();
      return editor;
    };
    const run = async (name: string) => {
      const report = await vscode.commands.executeCommand<TestReport>("trust-lsp.test.runAll", { projectUri: vscode.Uri.file(project).toString() });
      assert.ok(report);
      fs.writeFileSync(path.join(captures, `${name}.json`), JSON.stringify(report, null, 2));
      await sleep(250);
      await captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
      return report;
    };
    try {
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.window.showTextDocument(document);
      const ready = await run("01-published-calculations");
      assert.deepStrictEqual(ready.summary, { ...ready.summary, total: 2, passed: 2, failed: 0, errors: 0 });
      // Perform exactly the lesson's proposed learner edit, then restore it.
      const early = identifiability
        ? original.replace("LateObservationS : LREAL := 10.0;", "LateObservationS : LREAL := 1.0;")
        : original.replace("    WaterDeviationC := NextWaterDeviationC;\n", "")
          .replace("    NextSensorDeviationC :=", "    WaterDeviationC := NextWaterDeviationC;\n    NextSensorDeviationC :=");
      assert.notStrictEqual(early, original);
      const editor = await write(early);
      editor.revealRange(new vscode.Range(20, 0, 20, 0), vscode.TextEditorRevealType.InCenter);
      const changed = await run(identifiability ? "02-short-observation" : "02-early-water-assignment");
      assert.deepStrictEqual(changed.summary, { ...changed.summary, total: 2, passed: 1, failed: 1, errors: 0 });
      assert.strictEqual(changed.tests.find(test => test.status === "failed")?.name,
        identifiability ? "Lesson25EarlyAndLateSeparation" : "Lesson24BothStatesUsePreviousValues");
      await write(original);
      const restored = await run("03-restored-calculations");
      assert.deepStrictEqual(restored.summary, { ...restored.summary, total: 2, passed: 2, failed: 0, errors: 0 });
      const diagnostics = vscode.languages.getDiagnostics(document.uri);
      fs.writeFileSync(path.join(captures, "editor-diagnostics.json"), JSON.stringify(diagnostics, null, 2));
      assert.ok(diagnostics.every(item => item.severity !== vscode.DiagnosticSeverity.Error && item.code !== "W009"));
    } finally {
      if (document.getText() !== original) await write(original);
      await config.update("testRunner.executablePath", previousDev, vscode.ConfigurationTarget.Workspace);
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      connection.close();
    }
  });
});
