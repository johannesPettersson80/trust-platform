import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

const extensionRoot = path.resolve(__dirname, "..", "..", "..");

function homeSource(): string {
  return fs.readFileSync(path.join(extensionRoot, "src", "trustHomeView.ts"), "utf8");
}

suite("Control Studio sidebar destination", () => {
  test("sidebar delegates discovery and repair to the same no-argument command", async () => {
    const { registerTrustHome } = require("../../trustHomeView");
    const registerProvider = vscode.window.registerWebviewViewProvider;
    const registerCommand = vscode.commands.registerCommand;
    const executeCommand = vscode.commands.executeCommand;
    const warning = vscode.window.showWarningMessage;
    const subscriptions: vscode.Disposable[] = [];
    let provider: { onMessage: (message: unknown) => Promise<void> } | undefined;
    const calls: unknown[][] = [];
    (vscode.window as any).registerWebviewViewProvider = (_id: string, value: typeof provider) => {
      provider = value;
      return new vscode.Disposable(() => {});
    };
    (vscode.commands as any).registerCommand = () => new vscode.Disposable(() => {});
    (vscode.commands as any).executeCommand = async (...args: unknown[]) => { calls.push(args); };
    (vscode.window as any).showWarningMessage = async () => undefined;
    try {
      registerTrustHome({ subscriptions } as vscode.ExtensionContext);
      assert.ok(provider, "capture the actual registered home provider");
      await provider.onMessage({ type: "navControlStudio" });
      assert.deepStrictEqual(calls, [["trust-lsp.controlStudio.open"]],
        "sidebar must reuse command discovery, cancellation and repair instead of maintaining a second path");
    } finally {
      subscriptions.forEach((subscription) => subscription.dispose());
      (vscode.window as any).registerWebviewViewProvider = registerProvider;
      (vscode.commands as any).registerCommand = registerCommand;
      (vscode.commands as any).executeCommand = executeCommand;
      (vscode.window as any).showWarningMessage = warning;
    }
  });
  test("is visible below Devices & Connections and opens the existing editor", () => {
    const source = homeSource();
    const devices = source.indexOf('id="navDevices"');
    const studio = source.indexOf('id="navControlStudio"');
    const liveValues = source.indexOf('id="navLiveValues"');
    const hmi = source.indexOf('id="navHmi"');
    const libraries = source.indexOf('id="navLibraries"');

    assert.ok(studio >= 0, "the truST sidebar does not expose Control Studio");
    assert.ok(
      devices < studio && studio < liveValues && liveValues < hmi && hmi < libraries,
      "the project destinations must follow Devices, Control Studio, Live Values, HMI, Libraries"
    );
    assert.ok(
      source.includes('case "navControlStudio"') &&
        source.includes('executeCommand("trust-lsp.controlStudio.open")') &&
        source.includes('document.getElementById("navControlStudio").addEventListener'),
      "the sidebar destination must resolve and open the configured project loop"
    );
    assert.ok(
      source.includes("Control Studio</span></button>") &&
        source.includes("Open a project to use this."),
      "the no-project shell must keep Control Studio visible but disabled with a reason"
    );
  });

  test("the first proportional-control lesson declares its complete live loop", () => {
    const loopPath = path.join(
      extensionRoot,
      "..", "..", "examples", "control",
      "01-feedback-foundations-and-temperature-control", "04-water-heater-p",
      "control-loop.json"
    );
    assert.ok(fs.existsSync(loopPath), "the runnable lesson has no Control Studio loop definition");
    const loop = JSON.parse(fs.readFileSync(loopPath, "utf8"));
    assert.strictEqual(loop.controller.instance, "P1.Controller");
    assert.strictEqual(loop.controller.type, "Control.PID");
    assert.strictEqual(loop.signals.setpoint.address, "%ML0");
    assert.strictEqual(loop.signals.processVariable.address, "%IL0");
    assert.strictEqual(loop.signals.manipulatedVariable.address, "%QL0");
    assert.strictEqual(loop.controller.modeSignal, "P1.AutoMode");
    assert.strictEqual(loop.plantId, "proportional-water-heater");
  });

  test("the step-response identification lesson exposes one operable experiment loop", () => {
    const project = path.join(
      extensionRoot,
      "..", "..", "examples", "control",
      "05-experiments-identification-and-validation", "23-step-response-identification"
    );
    const loopPath = path.join(project, "control-loop.json");
    assert.ok(fs.existsSync(loopPath), "lesson 23 has no Control Studio loop definition");

    const loop = JSON.parse(fs.readFileSync(loopPath, "utf8"));
    const source = fs.readFileSync(path.join(project, "src", "Main.st"), "utf8");
    assert.strictEqual(loop.controller.instance, "P1");
    assert.strictEqual(loop.controller.type, "Main");
    assert.strictEqual(loop.controller.modeSignal, undefined);
    assert.strictEqual(loop.signals.processVariable.address, "%IL0");
    assert.strictEqual(loop.signals.manipulatedVariable.address, "%QL0");
    assert.strictEqual(loop.signals.manipulatedVariable.direction, "read");
    assert.strictEqual(loop.experiment.activation.symbol, "P1.ExperimentEnabled");
    assert.strictEqual(loop.experiment.activation.address, "%MX80.0");
    assert.strictEqual(loop.experiment.activation.iecType, "BOOL");
    assert.strictEqual(loop.experiment.activation.direction, "write");
    assert.match(source, /ExperimentEnabled/);
    assert.strictEqual(loop.plantId, "heater-step-identification");
  });
});
