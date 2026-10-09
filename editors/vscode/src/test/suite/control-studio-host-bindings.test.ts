import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

function pidLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
}

function catalog() {
  return {
    ok: true,
    processImage: {
      commands: [{ symbol: "Main.MV", address: "%QL0", type: "LREAL" }, { symbol: "Main.MV2", address: "%QL8", type: "LREAL" }],
      feedbacks: [{ symbol: "Main.PV", address: "%IL0", type: "LREAL" }, { symbol: "Main.PV2", address: "%IL8", type: "LREAL" }],
      memory: [{ symbol: "Main.SP", address: "%ML0", type: "LREAL" }],
    },
    controlSymbols: {
      controllers: [{ instance: "Main", type: "Control.PID", tasks: [{ name: "MainTask", intervalMs: 20 }] }],
      symbols: [
        { symbol: "Main.Kp", type: "LREAL" }, { symbol: "Main.Ki", type: "LREAL" }, { symbol: "Main.Kd", type: "LREAL" },
        { symbol: "Main.Manual", type: "BOOL" }, { symbol: "Main.ManualCommand", type: "LREAL" },
        { symbol: "Main.Integral", type: "LREAL" }, { symbol: "Main.PreviousError", type: "LREAL" },
      ],
    },
  };
}

function hostTarget(loop: any) {
  const signal = (id: string, symbol: string, role: string, order: number) => ({ id, symbol, address: `%${symbol}`, iecType: "LREAL", unit: "1", bounds: { minimum: 0, maximum: 1 }, role, order });
  return {
    configurationPath: "/tmp/control-loop.json",
    loopDefinition: loop,
    controller: { instance: "Main", type: "Control.PID", tasks: [{ name: "MainTask", intervalMs: 20 }] },
    inputs: [signal("manipulatedVariable", "Main.MV", "MV", 0)],
    outputs: [signal("processVariable", "Main.PV", "PV", 0)],
    controllerCompatibility: { kind: "accepted", families: ["P", "PI", "PD", "PID"] },
  };
}

suite("Control Studio host binding discovery", () => {
  test("lists exact configured channels and resolves only an explicit controller and mapping", async () => {
    const api = loadApi();
    assert.ok(api, "host binding discovery service must be available");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-bindings-"));
    try {
      const loop = pidLoop();
      loop.signals.additional = [
        { id: "mv-secondary", channel: "secondary", role: "MV", symbol: "Main.MV2", address: "%QL8", iecType: "LREAL", direction: "write", unit: "V", range: { minimum: -5, maximum: 5 } },
        { id: "pv-secondary", channel: "secondary", role: "PV", symbol: "Main.PV2", address: "%IL8", iecType: "LREAL", direction: "read", unit: "m", range: { minimum: -3, maximum: 3 } },
      ];
      loop.channels = {
        inputs: [
          { modelChannelId: "primary", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: -20, maximum: 20 } },
          { modelChannelId: "secondary", signalId: "mv-secondary", actuator: { kind: "continuous", minimum: -5, maximum: 5 } },
        ],
        outputs: [
          { modelChannelId: "primary", signalId: "processVariable" },
          { modelChannelId: "secondary", signalId: "pv-secondary" },
        ],
      };
      fs.writeFileSync(path.join(root, "control-loop.json"), `${JSON.stringify(loop)}\n`);

      const discovered = await api.discoverControlStudioBindings(root, loop.runtimeId, catalog());
      assert.deepStrictEqual(discovered.rejected, []);
      assert.strictEqual(discovered.targets.length, 1);
      const target = discovered.targets[0];
      assert.strictEqual(target.controller.instance, "Main");
      assert.strictEqual(target.controller.type, "Control.PID");
      assert.deepStrictEqual(target.inputs.map((entry: any) => [entry.id, entry.symbol, entry.bounds]), [
        ["manipulatedVariable", "Main.MV", { minimum: -20, maximum: 20 }],
        ["mv-secondary", "Main.MV2", { minimum: -5, maximum: 5 }],
      ]);
      assert.deepStrictEqual(target.outputs.map((entry: any) => [entry.id, entry.symbol, entry.bounds]), [
        ["processVariable", "Main.PV", { minimum: -20, maximum: 20 }],
        ["pv-secondary", "Main.PV2", { minimum: -3, maximum: 3 }],
      ]);

      const selected = api.resolveControlStudioBinding(discovered, {
        loopId: loop.id,
        controllerInstance: "Main",
        inputIds: ["mv-secondary", "manipulatedVariable"],
        outputIds: ["pv-secondary", "processVariable"],
      });
      assert.strictEqual(selected.kind, "accepted");
      assert.deepStrictEqual(selected.inputs.map((entry: any) => entry.id), ["manipulatedVariable", "mv-secondary"], "declared channel order is retained");
      assert.deepStrictEqual(selected.outputs.map((entry: any) => entry.id), ["processVariable", "pv-secondary"], "declared channel order is retained");
      assert.notStrictEqual(selected.loopDefinition, loop, "selection returns an isolated loop snapshot");
      assert.strictEqual(loop.controller.instance, "Main", "discovery and selection do not mutate the configured loop");

      const incomplete = api.resolveControlStudioBinding(discovered, {
        loopId: loop.id, controllerInstance: "Main", inputIds: ["manipulatedVariable"], outputIds: ["processVariable"],
      });
      assert.strictEqual(incomplete.kind, "rejected", "a partial MIMO selection must not leave the full configured loop active");

      const wrongController = api.resolveControlStudioBinding(discovered, {
        loopId: loop.id, controllerInstance: "Main.Other", inputIds: ["manipulatedVariable"], outputIds: ["processVariable"],
      });
      assert.strictEqual(wrongController.kind, "rejected");
      const guessedSignal = api.resolveControlStudioBinding(discovered, {
        loopId: loop.id, controllerInstance: "Main", inputIds: ["Main.MV"], outputIds: ["processVariable"],
      });
      assert.strictEqual(guessedSignal.kind, "rejected");
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("rejects a configured loop when the compiled controller identity or signal identity changed", async () => {
    const api = loadApi();
    assert.ok(api, "host binding discovery service must be available");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-bindings-invalid-"));
    try {
      const loop = pidLoop();
      fs.writeFileSync(path.join(root, "control-loop.json"), `${JSON.stringify(loop)}\n`);
      const changed = catalog();
      changed.controlSymbols.controllers[0].type = "OtherPID";
      changed.processImage.commands[0].address = "%QL16";
      const discovered = await api.discoverControlStudioBindings(root, loop.runtimeId, changed);
      assert.strictEqual(discovered.targets.length, 0);
      assert.ok(discovered.rejected.some((entry: any) => /controller|compiled|signal|address/i.test(entry.reason)));
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("lists a loop whose measurement is a compiled variable without an address, such as a PID's own (Specification 35 §17.1)", async () => {
    const api = loadApi();
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-bindings-unaddressed-"));
    try {
      const loop = pidLoop();
      const { address: _address, ...measurement } = loop.signals.processVariable;
      loop.signals.processVariable = { ...measurement, symbol: "Main.StateValue.ProcessValue" };
      fs.writeFileSync(path.join(root, "control-loop.json"), `${JSON.stringify(loop)}\n`);
      const compiled = catalog();
      const missing = await api.discoverControlStudioBindings(root, loop.runtimeId, compiled);
      assert.ok(missing.rejected.some((entry: any) => /Main\.StateValue\.ProcessValue is not a compiled LREAL variable/.test(entry.reason)),
        JSON.stringify(missing.rejected));
      compiled.controlSymbols.symbols.push({ symbol: "Main.StateValue.ProcessValue", type: "LREAL" });
      const discovered = await api.discoverControlStudioBindings(root, loop.runtimeId, compiled);
      assert.deepStrictEqual(discovered.rejected, []);
      assert.deepStrictEqual(discovered.targets[0].outputs.map((entry: any) => [entry.symbol, entry.address]), [["Main.StateValue.ProcessValue", undefined]]);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("rejects duplicate loop IDs instead of exposing an ambiguous target", async () => {
    const api = loadApi();
    assert.ok(api, "host binding discovery service must be available");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-bindings-duplicate-"));
    try {
      const loop = pidLoop();
      fs.writeFileSync(path.join(root, "control-loop.json"), `${JSON.stringify(loop)}\n`);
      fs.mkdirSync(path.join(root, "control-loops"));
      fs.writeFileSync(path.join(root, "control-loops", "same-id.json"), `${JSON.stringify(loop)}\n`);
      const discovered = await api.discoverControlStudioBindings(root, loop.runtimeId, catalog());
      assert.strictEqual(discovered.targets.length, 0, "duplicate IDs must not leave one selectable target");
      assert.ok(discovered.rejected.some((entry: any) => /duplicate.*id/i.test(entry.reason)));
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("scopes binding list/select replies and opens a selected loop through a new session callback", async () => {
    const api = loadSessionApi();
    assert.ok(api, "host binding session service must be available");
    const loop = pidLoop();
    const scope = { projectId: loop.projectId, runtimeId: loop.runtimeId, sessionId: "existing-session" };
    const project = { id: loop.projectId, name: "Commissioning project", rootUri: "/workspace/commissioning" };
    const target = hostTarget(loop);
    let chosenRoot: string | undefined;
    let opened: any;
    const host = api.createControlStudioBindingSession({
      scope,
      workspaceProjects: async () => [project],
      selectWorkspaceProject: async (projects: readonly any[]) => projects[0],
      discover: async (rootUri: string) => { chosenRoot = rootUri; return { targets: [target], rejected: [] }; },
      openNewSession: async (value: unknown) => { opened = value; },
    });
    const listed = await host.handle({ schemaVersion: 1, kind: "control-studio.binding.list", scope });
    assert.strictEqual(listed.kind, "control-studio.binding.list.reply");
    assert.deepStrictEqual(listed.scope, scope);
    assert.strictEqual(chosenRoot, project.rootUri);
    assert.strictEqual(listed.targets[0].loopId, loop.id);
    const selected = await host.handle({ schemaVersion: 1, kind: "control-studio.binding.select", scope,
      selection: { loopId: loop.id, controllerInstance: "Main", inputIds: ["manipulatedVariable"], outputIds: ["processVariable"] } });
    assert.strictEqual(selected.kind, "control-studio.binding.select.reply");
    assert.strictEqual(selected.status, "accepted");
    assert.notStrictEqual(selected.sessionId, scope.sessionId, "selection opens a new scoped session");
    assert.strictEqual(opened.sessionId, selected.sessionId);
    assert.strictEqual(opened.request.loopDefinition.id, loop.id);
    assert.notStrictEqual(opened.request.loopDefinition, loop, "new-session open receives an isolated loop snapshot");
    const wrongScope = await host.handle({ schemaVersion: 1, kind: "control-studio.binding.select",
      scope: { ...scope, sessionId: "other-session" }, selection: { loopId: loop.id, controllerInstance: "Main", inputIds: ["manipulatedVariable"], outputIds: ["processVariable"] } });
    assert.strictEqual(wrongScope.status, "rejected");
    assert.strictEqual(opened.sessionId, selected.sessionId, "a stale scope cannot open another session");
  });
});

function loadApi(): any {
  try { return require("../../controlStudio/hostBindingDiscovery"); }
  catch { return undefined; }
}

function loadSessionApi(): any {
  try { return require("../../controlStudio/hostBindingSession"); }
  catch { return undefined; }
}
