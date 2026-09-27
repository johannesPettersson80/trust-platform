import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import { controllerAdapter } from "../../controlStudio/controllerAdapters";
import { discoverControlStudioBindings } from "../../controlStudio/hostBindingDiscovery";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { declaredPidGroup, mimoLoop, projectPidGroupLoop } from "./control-studio-pid-group-fixtures";
import { pidLoop } from "./control-studio-pid-fixtures";
import { renderThreePageStudioHtml } from "../../controlStudio/threePageTemplate";

function catalog() {
  return {
    ok: true,
    processImage: {
      commands: [{ symbol: "Main.MV", address: "%QL0", type: "LREAL" }, { symbol: "Main.MV2", address: "%QL8", type: "LREAL" }],
      feedbacks: [{ symbol: "Main.PV", address: "%IL0", type: "LREAL" }, { symbol: "Main.PV2", address: "%IL8", type: "LREAL" }],
      memory: [{ symbol: "Main.SP", address: "%ML0", type: "LREAL" }, { symbol: "Main.SP2", address: "%ML8", type: "LREAL" }],
    },
    controlSymbols: {
      controllers: [{ instance: "Main", type: "Control.PID", tasks: [{ name: "MainTask", intervalMs: 20 }] }],
      symbols: [
        { symbol: "Main.Kp", type: "LREAL" }, { symbol: "Main.Ki", type: "LREAL" }, { symbol: "Main.Kd", type: "LREAL" },
        { symbol: "Main.Manual", type: "BOOL" }, { symbol: "Main.ManualCommand", type: "LREAL" },
        { symbol: "Main.Integral", type: "LREAL" }, { symbol: "Main.PreviousError", type: "LREAL" },
        { symbol: "Main.Secondary.Kp", type: "LREAL" }, { symbol: "Main.Secondary.Ki", type: "LREAL" }, { symbol: "Main.Secondary.Kd", type: "LREAL" },
        { symbol: "Main.Secondary.Manual", type: "BOOL" }, { symbol: "Main.Secondary.Integral", type: "LREAL" }, { symbol: "Main.Secondary.PreviousError", type: "LREAL" },
      ],
    },
  };
}

suite("Control Studio declared PID groups", () => {
  test("blocks cross-program groups before tuning or application when the runtime cannot transfer their states coherently", () => {
    const loop = projectPidGroupLoop();
    const crossProgram = JSON.parse(JSON.stringify(loop).replace(/P1\.Secondary/g, "P2.Secondary"));
    assert.throws(() => controllerAdapter(crossProgram), /same.*program|one.*program|cross.program|coherent/i);
  });
  test("names every PID member in the one Controller selector", () => {
    const loop = mimoLoop(); loop.controller.group = declaredPidGroup();
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(renderThreePageStudioHtml({ loopDefinition: loop } as any,
      value => value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;")));
    try {
      const selectors = dom.window.document.querySelectorAll("[data-studio-controller]");
      assert.strictEqual(selectors.length, 1);
      assert.match(selectors[0].selectedOptions[0].textContent, /Main.*Main\.Secondary/,
        "The group entry must identify every actual member rather than just its anchor");
    } finally { dom.window.close(); }
  });
  test("rejects duplicate PID member identity", () => {
    const loop = mimoLoop();
    const group = declaredPidGroup();
    (group.members as Record<string, unknown>[])[1].instance = "Main";
    loop.controller.group = group;
    const result = validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId });
    assert.strictEqual(result.kind, "rejected", "two group rows must not own the same compiled PID instance");
  });

  test("rejects a PID group that does not cover a declared model channel", () => {
    const loop = mimoLoop();
    const group = declaredPidGroup();
    (group.members as Record<string, unknown>[])[1].inputChannelId = "missing-input-channel";
    loop.controller.group = group;
    const result = validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId });
    assert.strictEqual(result.kind, "rejected", "every declared input and output channel needs one owning PID member");
  });

  test("does not fall through to scalar PID tuning without a coherent group application adapter", () => {
    const loop = pidLoop();
    loop.controller.group = declaredPidGroup();
    assert.throws(() => controllerAdapter(loop), /group|multivariable|coherent|project-owned/i,
      "an undeclared group transaction must remain unavailable rather than using one scalar PID adapter");
  });

  test("does not offer a declared PID group when a member is absent from the compiled catalog", async () => {
    const loop = mimoLoop();
    loop.controller.group = declaredPidGroup();
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-pid-group-"));
    try {
      fs.writeFileSync(path.join(root, "control-loop.json"), `${JSON.stringify(loop)}\n`);
      const discovered = await discoverControlStudioBindings(root, loop.runtimeId, catalog());
      assert.strictEqual(discovered.targets.length, 0, "a group with a missing compiled member must not be selectable");
      assert.ok(discovered.rejected.some(entry => entry.reason.includes("Main.Secondary") && /compiled|member/i.test(entry.reason)),
        "the rejection must identify the unavailable compiled member");
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("accepts a complete project-owned PID group and exposes both transfer outputs", () => {
    const loop = projectPidGroupLoop();
    const validity = validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId });
    assert.strictEqual(validity.kind, "accepted", JSON.stringify(validity));
    const adapter = controllerAdapter(loop);
    assert.deepStrictEqual(adapter.families, ["paired-PI"]);
    const binding = adapter.transferBinding();
    assert.strictEqual(1 + (binding.additional_outputs?.length ?? 0), 2, "the coherent group adapter retains both PID outputs");
  });

  test("rejects an unowned parameter from the group union", () => {
    const loop = projectPidGroupLoop();
    loop.parameterBinding.parameters.push({ ...loop.parameterBinding.parameters[0], symbol: "P1.SecondaryParameters.Unowned" });
    assert.throws(() => controllerAdapter(loop), /group|parameter|complete|declared/i);
  });

  test("rejects a group missing the second NominalOutput retained state", () => {
    const loop = projectPidGroupLoop();
    loop.controller.project.initialization.states = loop.controller.project.initialization.states
      .filter((state: { symbol: string }) => state.symbol !== "P1.Secondary.StateValue.NominalOutput");
    assert.throws(() => controllerAdapter(loop), /group|state|NominalOutput|complete|declared/i);
  });

  test("rejects a secondary initialization source that points at the primary model input", () => {
    const loop = projectPidGroupLoop();
    const state = loop.controller.project.initialization.states.find((candidate: { symbol: string }) => candidate.symbol === "P1.Secondary.StateValue.NominalOutput");
    state.initial.index = 0;
    assert.throws(() => controllerAdapter(loop), /group|coordinate|index|secondary|initial/i);
  });

  test("rejects a member mode symbol that is not the shared group mode", () => {
    const loop = projectPidGroupLoop();
    loop.controller.group.members[1].modeSymbol = "P1.SecondaryAutoMode";
    assert.throws(() => controllerAdapter(loop), /group|mode|coherent|shared/i);
  });
});
