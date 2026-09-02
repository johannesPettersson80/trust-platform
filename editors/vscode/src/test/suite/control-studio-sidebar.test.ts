import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

const extensionRoot = path.resolve(__dirname, "..", "..", "..");

function homeSource(): string {
  return fs.readFileSync(path.join(extensionRoot, "src", "trustHomeView.ts"), "utf8");
}

suite("Control Studio sidebar destination", () => {
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
        source.includes("controlStudioRequestForWorkspace") &&
        source.includes('executeCommand("trust-lsp.controlStudio.open", request)') &&
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
    assert.strictEqual(loop.signals.setpoint.address, "%ML0");
    assert.strictEqual(loop.signals.processVariable.address, "%IL0");
    assert.strictEqual(loop.signals.manipulatedVariable.address, "%QL0");
    assert.strictEqual(loop.controller.modeSignal, "%MX16.0");
    assert.strictEqual(loop.plantId, "first_order_dead_time");
  });

  test("the heater identification lesson exposes one operable Manual experiment loop", () => {
    const project = path.join(
      extensionRoot,
      "..", "..", "examples", "control",
      "01-feedback-foundations-and-temperature-control", "16-heater-step-identification"
    );
    const loopPath = path.join(project, "control-loop.json");
    assert.ok(fs.existsSync(loopPath), "lesson 16 has no Control Studio loop definition");

    const loop = JSON.parse(fs.readFileSync(loopPath, "utf8"));
    const source = fs.readFileSync(path.join(project, "src", "Main.st"), "utf8");
    assert.strictEqual(loop.controller.instance, "P1.Controller");
    assert.strictEqual(
      loop.controller.revision,
      "sha256:4af4084c5bbd67aa35e327d6e2d8b2ca2ecd918f63301c61142b340b3e309b86",
      "the reviewed loop must name the active initial Control.PID parameter revision"
    );
    assert.strictEqual(loop.controller.modeSignal, "%MX16.3");
    assert.strictEqual(loop.signals.setpoint.address, "%ML8");
    assert.strictEqual(loop.signals.processVariable.address, "%IL0");
    assert.strictEqual(loop.signals.manipulatedVariable.address, "%ML0");
    assert.strictEqual(loop.signals.manipulatedVariable.direction, "write");
    assert.strictEqual(loop.signals.actuator.address, "%QL0");
    assert.strictEqual(loop.signals.actuator.direction, "read");
    assert.match(source, /AutoMode\s*:\s*BOOL\s*:=\s*FALSE/);
    assert.match(source, /Input\.ManualOutput\s*:=\s*ExperimentInput/);
    assert.doesNotMatch(source, /TestAuthorityEnabled/);
    assert.strictEqual(loop.plantId, "unknown_local_self_regulating_heater");
  });
});
