import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { execFileSync } from "child_process";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Rendered Routh–Hurwitz analysis", () => {
  test("uses the selected supplied model for Routh without reentering the polynomial", async function () {
    if (process.env.TRUST_PLANT_RENDERED_TEST !== "1") this.skip();
    this.timeout(60000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const signal = (symbol: string, address: string, unit: string) => ({ symbol, address,
      unit, direction: "read", iecType: "LREAL", range: { minimum: -10, maximum: 10 } });
    const loop = { schemaVersion: 2, id: "engineer-model-selection", name: "Engineer model selection",
      projectId: "project:engineer-model-selection", runtimeId: "simulator", revision: "sha256:selection-loop",
      controller: { instance: "Main.Law", type: "CustomProportional", design: "project", revision: "sha256:law",
        action: "direct", taskPeriodMs: 20 },
      signals: { setpoint: signal("Main.Reference", "%ML0", "m"),
        processVariable: signal("Main.Position", "%IL0", "m"), manipulatedVariable: signal("Main.Command", "%QL0", "V") },
      limits: { outputMinimum: -10, outputMaximum: 10 }, observedSampleTiming: { nominalPeriodMs: 20 },
      experiment: { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 10, seed: 1 },
      analysis: { plant: { kind: "transfer-function", numerator: [1], denominator: [1, 1, 0], delaySeconds: 0,
        inputUnit: "V", outputUnit: "m", revision: "old-model", source: "provided", validation: "unvalidated" },
        controller: { numerator: [{ parameter: "Kp" }], denominator: [1],
          parameters: { Kp: { value: 1, minimum: -10, maximum: 10, unit: "V/m" } } },
        rootLocus: { parameter: "Kp", values: [1], selectedValue: 1 } } };
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
      scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const file = path.join(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), "selected-model.json");
    try {
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await inner("Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune').click();return 'tune';");
      await pollValue("selected-model workflow ready", () => inner("return d.querySelector('[data-routh]')?.dataset.ready ?? '';"),
        value => value === "true", 10000);
      await inner("d.querySelector('[data-routh-run]').click();return 'old-model';");
      await pollValue("prior model analysis", () => inner("return d.querySelector('[data-routh-status]').textContent;"),
        value => value.includes("Analysis complete"), 10000);
      const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -10, maximum: 10 } });
      fs.writeFileSync(file, JSON.stringify({ kind: "transfer-function", numerator: [2], denominator: [1, 3, 2],
        delaySeconds: 0, revision: "selected-nonshaft", source: "provided", validation: "unvalidated", evidence: [],
        inputs: [channel("command", "V")], outputs: [channel("position", "m")] }));
      await connection.send("DOM.enable", {}, session);
      await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session);
      const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const node = tree.nodes.find((n: any) => n.nodeName === "INPUT" && n.attributes?.includes("general-model-file"));
      assert.ok(node, "Supply model uses the real file control");
      await connection.send("DOM.setFileInputFiles", { files: [file], backendNodeId: node.backendNodeId }, session);
      await pollValue("selected supplied model", () => inner("return d.querySelector('[data-general-model-status]').textContent;"),
        value => value.includes("selected-nonshaft"), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-routh-result]').hidden);"), "true",
        "Changing the selected model must invalidate the previous Routh result immediately");
      await inner("d.querySelector('[data-routh-run]').click();return 'routh';");
      await pollValue("Routh completed", () => inner("return d.querySelector('[data-routh-status]').textContent;"),
        value => value.includes("Analysis complete"), 10000);
      const result = await inner("return d.querySelector('[data-routh-result]').textContent;");
      assert.match(result, /Coefficients in descending powers of s: 1, 3, 4/,
        "Routh must consume the selected plant; the old declared plant must not survive model selection");
      assert.match(result, /Kp > -1/);
      assert.match(result, /selected-nonshaft/);
      assert.match(result, /unvalidated/, "provisional supplied evidence must not become validated");
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) {
        await inner("d.querySelector('[data-routh]').scrollIntoView({block:'start'});return 'visible';");
        await new Promise(resolve => setTimeout(resolve, 400));
        execFileSync("import", ["-window", "root", screenshot]);
      }
    } finally { connection.close(); fs.rmSync(file, { force: true }); }
  });

  test("constructs the Routh table and analytical interval for selected gains", async function () {
    if (process.env.TRUST_PLANT_RENDERED_TEST !== "1") this.skip();
    this.timeout(90000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const repository = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const loop = JSON.parse(fs.readFileSync(path.join(repository, "examples/control/07-classical-stability-and-frequency-design/45-root-locus/control-loop.json"), "utf8"));
    delete loop.plantId;
    loop.parameterBinding.parameters = [ { symbol: "P1.Kp", iec_type: "LREAL", unit: "N·m/rad", convention: "proportional gain", minimum: -1, maximum: 4 } ];
    loop.analysisParameterSymbols = { Kp: "P1.Kp" };
    loop.id = "routh-rendered";
    loop.parameterBinding.loop_id = loop.id;
    loop.projectId = "project:routh-rendered";
    loop.analysis.controller = { numerator: [{ parameter: "Kp" }], denominator: [1],
      parameters: { Kp: { value: 1, minimum: -1, maximum: 4, unit: "N·m/rad" } } };
    loop.analysis.rootLocus = { parameter: "Kp", values: [1, 4], selectedValue: 1 };
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME;
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-routh-run]')));"), "true", "Control Studio must calculate Routh–Hurwitz automatically");
      await pollValue("Routh handlers", () => inner("return d.querySelector('[data-routh]').dataset.ready;"), value => value === "true", 10000);
      await inner("Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune').click();return 'tune';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-routh-run]').getBoundingClientRect().height>0);"), "true");
      for (const [gain, expected] of [[1, "asymptotically stable"], [-1, "1 right-half-plane"], [0, "origin pole"]] as const) {
        await inner(`d.querySelector('[data-routh-value]').value='${gain}';d.querySelector('[data-routh-run]').click();return 'run';`);
        await pollValue(`Routh gain ${gain}`, () => inner("return d.querySelector('[data-routh-status]').textContent;"), value => value.includes(`Kp = ${gain}`), 10000);
        const result = await inner("return d.querySelector('[data-routh-result]').textContent;");
        assert.ok(result.includes(expected), result);
        assert.match(result, /Kp > 0/);
        assert.match(result, /First.column/i);
        assert.match(result, /unvalidated/);
        await inner("d.querySelector('[data-parameters-selected]').click();return 'selected';");
        await pollValue("selected gain draft", () => inner("return d.querySelector('[data-parameters-values]').value;"), value => {
          try { return JSON.parse(value)["P1.Kp"] === gain; } catch { return false; }
        }, 10000);
        assert.strictEqual(await inner("return String(d.querySelectorAll('[data-routh-table] tbody tr').length);"), "3");
      }
      await inner("d.querySelector('[data-routh-value]').value='1';d.querySelector('[data-routh-run]').click();return 'run';");
      await pollValue("positive table", () => inner("return d.querySelector('[data-routh-status]').textContent;"), value => value.includes("Kp = 1"), 10000);
      await vscode.commands.executeCommand("notifications.clearAll");
      await inner("d.querySelector('[data-routh]').scrollIntoView({block:'start'});return 'scroll';");
      await new Promise(resolve => setTimeout(resolve, 400));
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) execFileSync("import", ["-window", "root", screenshot]);
      assert.strictEqual(await inner("return d.querySelector('.studio').dataset.analysisView;"), "routh");
      assert.strictEqual(await inner("return String(d.querySelector('[data-control-studio-monitor]').getBoundingClientRect().height < 160);"), "true",
        "Tune Routh results must collapse the empty chart region so the table and interval can be read");
      assert.strictEqual(await inner("return String(d.querySelector('.values').getBoundingClientRect().height > 0);"), "true");
      await inner("d.querySelector('[data-declared-root-run]').click();return 'root';");
      await pollValue("root locus", () => inner("return String(d.querySelectorAll('[data-declared-root-select]').length);"), value => Number(value) > 0, 10000);
      await inner("Array.from(d.querySelectorAll('[data-declared-root-select]')).find(b=>b.dataset.declaredRootSelect==='4').click();return 'select';");
      await pollValue("root locus selection", () => inner("return d.querySelector('[data-declared-root-result] tr[data-selected=true]')?.textContent ?? '';"), value => value.includes("Kp = 4"), 10000);
      assert.strictEqual(await inner("return d.querySelector('.studio').dataset.analysisView;"), "chart");
      assert.strictEqual(await inner("return String(d.querySelector('[data-control-studio-monitor]').getBoundingClientRect().height >= 260);"), "true",
        "choosing root-locus analysis must restore the chart workspace");
      await inner("d.querySelector('[data-parameters-selected]').click();return 'use-selected';");
      await pollValue("root locus selected values", () => inner("return d.querySelector('[data-parameters-status]').textContent;"), value => value.includes("Selected analysis values copied"), 10000);
      assert.strictEqual(await inner("return JSON.parse(d.querySelector('[data-parameters-values]').value)['P1.Kp'];"), "4",
        "selected root-locus analysis values must take precedence over the prior Routh value");
      await inner("d.querySelector('[data-routh-value]').value='1';d.querySelector('[data-routh-run]').click();return 'run-routh-again';");
      await pollValue("Routh after root locus", () => inner("return d.querySelector('[data-routh-status]').textContent;"), value => value.includes("Kp = 1"), 10000);
      assert.strictEqual(await inner("return d.querySelector('.studio').dataset.analysisView;"), "routh");
      assert.strictEqual(await inner("return String(d.querySelector('[data-control-studio-monitor]').getBoundingClientRect().height < 160);"), "true");
    } finally { connection.close(); }
  });
});
