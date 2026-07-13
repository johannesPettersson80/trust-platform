import * as assert from "assert";
import * as fs from "fs";
import * as vscode from "vscode";

import { runtimeLifecycleService, RuntimeLifecycleService } from "../../runtimeLifecycle";
import type { CheckProgramResponse } from "../../checkProgramModel";
import { waitForSessionStable } from "../../runtimeSessionReadiness";
import { buildRuntimeTomlSource } from "../../newProject";
import {
  getSelectedRuntimeId,
  setSelectedRuntimeId,
} from "../../selectedRuntime";
import { SIMULATOR_RUNTIME_ID } from "../../trustHomeModel";
import { TEST_RUN_SIDEBAR_ACTION_COMMAND } from "../../trustHomeView";
import { __testEnsureConfigurationEntryAuto } from "../../debug";
import {
  closeAllEditorsAndWait,
  deleteFileIfExistsStrict,
  waitForStructuredTextEviction,
} from "./workspace-cleanup";
import {
  SOURCE_FAILURE,
  VALID,
  closeLiveValuesTabs,
  deferred,
  fakeSession,
  fixtureRoot,
  liveValuesTabs,
  localHarness,
  stubSimulatorReadiness,
} from "./runtime-operation-lock-fixtures";

suite("Runtime lifecycle operation lock", () => {
  test("Extension Host sidebar Start preserves the editor and never opens Live Values", async function () {
    this.timeout(90_000);
    const extension = vscode.extensions.getExtension(
      "trust-platform.trust-lsp",
    );
    assert.ok(extension, "Expected the truST extension in the test host.");
    await extension!.activate();
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder, "Expected an Extension Host test workspace.");
    const adapterPath = process.env.ST_DEBUG_TEST_BIN?.trim();
    assert.ok(adapterPath, "ST_DEBUG_TEST_BIN must identify the tested adapter.");

    const previousFiles = new Map<string, Uint8Array | undefined>();
    const remember = async (relative: string): Promise<vscode.Uri> => {
      const uri = vscode.Uri.joinPath(folder!.uri, ...relative.split("/"));
      try {
        previousFiles.set(relative, await vscode.workspace.fs.readFile(uri));
      } catch {
        previousFiles.set(relative, undefined);
      }
      return uri;
    };
    const projectToml = await remember("trust-lsp.toml");
    const runtimeToml = await remember("runtime.toml");
    const ioToml = await remember("io.toml");
    const mainSource = await remember("src/Main.st");
    const configSource = await remember("src/config.st");
    const staleConfigSource = await remember("configuration.st");
    const debugConfig = vscode.workspace.getConfiguration("trust", folder.uri);
    const previousAdapterPath = debugConfig.inspect<string>(
      "debugAdapter.executablePath",
    )?.workspaceValue;
    const previousSelectedRuntime = getSelectedRuntimeId();

    try {
      const accepted = runtimeLifecycleService.acceptedDebugSession();
      if (accepted) {
        await runtimeLifecycleService.stopRuntime();
      } else {
        const active = vscode.debug.activeDebugSession;
        if (active?.type === "structured-text") {
          await vscode.debug.stopDebugging(active);
        }
      }
      const stoppedDeadline = Date.now() + 5_000;
      while (
        (runtimeLifecycleService.phase() !== "stopped" ||
          runtimeLifecycleService.operationState() !== undefined) &&
        Date.now() < stoppedDeadline
      ) {
        await new Promise((resolve) => setTimeout(resolve, 25));
      }
      assert.strictEqual(
        runtimeLifecycleService.phase(),
        "stopped",
        "the action fixture must begin from the literal Start state",
      );
      await closeLiveValuesTabs();
      await vscode.workspace.fs.createDirectory(
        vscode.Uri.joinPath(folder.uri, "src"),
      );
      await vscode.workspace.fs.writeFile(
        projectToml,
        Buffer.from('include_paths = ["src"]\n', "utf8"),
      );
      await vscode.workspace.fs.writeFile(
        runtimeToml,
        Buffer.from(buildRuntimeTomlSource(process.platform), "utf8"),
      );
      await vscode.workspace.fs.writeFile(
        ioToml,
        Buffer.from('[io]\ndriver = "simulated"\nparams = {}\n', "utf8"),
      );
      await vscode.workspace.fs.writeFile(
        mainSource,
        Buffer.from("PROGRAM Main\nEND_PROGRAM\n", "utf8"),
      );
      await vscode.workspace.fs.writeFile(
        configSource,
        Buffer.from(
          [
            "CONFIGURATION Config",
            "RESOURCE MainRes ON PLC",
            "    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);",
            "    PROGRAM Main WITH MainTask : Main;",
            "END_RESOURCE",
            "END_CONFIGURATION",
            "",
          ].join("\n"),
          "utf8",
        ),
      );
      await vscode.workspace.fs.writeFile(
        staleConfigSource,
        Buffer.from(
          [
            "CONFIGURATION StaleConfig",
            "RESOURCE MainRes ON PLC",
            "    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);",
            "    PROGRAM Main WITH MainTask : Main;",
            "END_RESOURCE",
            "END_CONFIGURATION",
            "",
          ].join("\n"),
          "utf8",
        ),
      );
      await debugConfig.update(
        "debugAdapter.executablePath",
        adapterPath,
        vscode.ConfigurationTarget.Workspace,
      );
      await setSelectedRuntimeId(SIMULATOR_RUNTIME_ID);

      const staleDocument = await vscode.workspace.openTextDocument(
        staleConfigSource,
      );
      await vscode.window.showTextDocument(staleDocument, {
        preview: false,
        preserveFocus: false,
      });
      assert.strictEqual(
        (await __testEnsureConfigurationEntryAuto())?.toString(),
        staleConfigSource.toString(),
        "the fixture must persist a stale configuration choice first",
      );

      const document = await vscode.workspace.openTextDocument(configSource);
      await vscode.window.showTextDocument(document, {
        preview: false,
        preserveFocus: false,
      });
      const editorBefore = vscode.window.activeTextEditor?.document.uri.toString();
      assert.strictEqual(editorBefore, configSource.toString());
      assert.strictEqual(
        (await __testEnsureConfigurationEntryAuto())?.toString(),
        configSource.toString(),
        "with two configs in one folder, the active CONFIGURATION must override the stale stored target without a modal",
      );
      const staleTab = vscode.window.tabGroups.all
        .flatMap((group) => [...group.tabs])
        .find(
          (tab) =>
            tab.input instanceof vscode.TabInputText &&
            tab.input.uri.toString() === staleConfigSource.toString(),
        );
      if (staleTab) {
        await vscode.window.tabGroups.close(staleTab, true);
      }
      // The runtime currently accepts one CONFIGURATION declaration per
      // launched source set. The selection regression above intentionally
      // exercises two candidates; remove the non-selected fixture before
      // proving the independent sidebar lifecycle/focus contract.
      await deleteFileIfExistsStrict(staleConfigSource);
      await waitForStructuredTextEviction([staleConfigSource]);
      assert.strictEqual(liveValuesTabs().length, 0);

      await vscode.commands.executeCommand(TEST_RUN_SIDEBAR_ACTION_COMMAND);
      const runningDeadline = Date.now() + 5_000;
      while (
        runtimeLifecycleService.phase() !== "running" &&
        Date.now() < runningDeadline
      ) {
        await new Promise((resolve) => setTimeout(resolve, 25));
      }

      const finalSnapshot = await runtimeLifecycleService.snapshot();
      assert.strictEqual(
        runtimeLifecycleService.phase(),
        "running",
        finalSnapshot.failure?.message ?? "sidebar Start did not reach Running",
      );
      assert.strictEqual(
        vscode.window.activeTextEditor?.document.uri.toString(),
        editorBefore,
        "the exact sidebar action path must not steal editor focus",
      );
      assert.strictEqual(
        liveValuesTabs().length,
        0,
        "Start must not reveal or create a Live Values tab",
      );
      assert.strictEqual(
        runtimeLifecycleService.acceptedDebugSession()?.configuration.program,
        configSource.fsPath,
        "with two configurations in one folder, the active CONFIGURATION editor must override stale stored selection without a popup",
      );
    } finally {
      if (runtimeLifecycleService.acceptedDebugSession()) {
        await runtimeLifecycleService.stopRuntime();
      } else if (vscode.debug.activeDebugSession?.type === "structured-text") {
        await vscode.debug.stopDebugging(vscode.debug.activeDebugSession);
      }
      await closeLiveValuesTabs();
      const cleanupUris = [...previousFiles]
        .filter(
          ([relative, previous]) =>
            previous === undefined && /\.(?:st|pou)$/i.test(relative),
        )
        .map(([relative]) =>
          vscode.Uri.joinPath(folder.uri, ...relative.split("/")),
        );
      await closeAllEditorsAndWait(cleanupUris);
      await debugConfig.update(
        "debugAdapter.executablePath",
        previousAdapterPath,
        vscode.ConfigurationTarget.Workspace,
      );
      await setSelectedRuntimeId(previousSelectedRuntime);
      const removedStructuredText: vscode.Uri[] = [];
      for (const [relative, previous] of previousFiles) {
        const uri = vscode.Uri.joinPath(folder.uri, ...relative.split("/"));
        if (previous) {
          await vscode.workspace.fs.writeFile(uri, previous);
        } else if (await deleteFileIfExistsStrict(uri)) {
          if (/\.(?:st|pou)$/i.test(uri.path)) {
            removedStructuredText.push(uri);
          }
        }
      }
      await waitForStructuredTextEviction(removedStructuredText);
    }
  });

  test("startup stability requires continuous exact-session presence", async () => {
    const session = fakeSession("launch", "stability-attempt");
    let tracked = true;
    let observations = 0;
    setTimeout(() => {
      tracked = false;
    }, 12);

    const stable = await waitForSessionStable(
      session,
      () => {
        observations += 1;
        return tracked;
      },
      60,
      4,
    );
    assert.strictEqual(stable, false);
    assert.ok(
      observations > 1,
      "acceptance must dwell instead of succeeding on the first present observation",
    );
  });

  test("ordinary I/O requests reject pending sessions until lifecycle acceptance", async () => {
    let calls = 0;
    const pending = {
      ...fakeSession("launch", "pending-io"),
      customRequest: async () => {
        calls += 1;
        return undefined;
      },
    } as vscode.DebugSession;
    const service = new RuntimeLifecycleService();
    const subject = service as unknown as {
      sessions: Map<string, vscode.DebugSession>;
      acceptedSessions: Set<string>;
      getStructuredTextSession: () => vscode.DebugSession | undefined;
    };
    subject.sessions.set(pending.id, pending);
    subject.getStructuredTextSession = () => pending;

    const beforeAcceptance = await service.requestIoState();
    assert.strictEqual(beforeAcceptance.ok, false);
    assert.strictEqual(calls, 0);

    subject.acceptedSessions.add(pending.id);
    const afterAcceptance = await service.requestIoState();
    assert.strictEqual(afterAcceptance.ok, true);
    assert.strictEqual(calls, 1);
  });

  test("deferred target selection keeps an owned session Starting until atomic acceptance", async () => {
    const root = fixtureRoot();
    const selectionEntered = deferred<void>();
    const releaseSelection = deferred<void>();
    let activeSession: vscode.DebugSession | undefined;
    let service!: RuntimeLifecycleService;
    try {
      service = new RuntimeLifecycleService(
        async (attemptId) => {
          activeSession = fakeSession("launch", attemptId);
          (
            service as unknown as {
              sessions: Map<string, vscode.DebugSession>;
            }
          ).sessions.set(activeSession.id, activeSession);
          return true;
        },
        undefined,
        async () => {
          selectionEntered.resolve(undefined);
          await releaseSelection.promise;
        },
        5,
      );
      const subject = service as unknown as {
        acceptedSessions: Set<string>;
        getStructuredTextSession: () => vscode.DebugSession | undefined;
        runtimeConfigTarget: () => vscode.Uri;
        setRuntimeMode: () => Promise<void>;
      };
      subject.getStructuredTextSession = () => activeSession;
      subject.runtimeConfigTarget = () => vscode.Uri.file(root);
      subject.setRuntimeMode = async () => undefined;
      stubSimulatorReadiness(service);

      const start = service.startLocalSimulator(async () => VALID);
      await selectionEntered.promise;
      assert.ok(activeSession);
      const duringSelection = await service.snapshot();
      assert.strictEqual(service.phase(), "starting");
      assert.strictEqual(service.acceptedDebugSession(), undefined);
      assert.strictEqual(subject.acceptedSessions.has(activeSession!.id), false);
      assert.strictEqual(duringSelection.starting, true);
      assert.strictEqual(duringSelection.status.running, false);
      assert.strictEqual(duringSelection.status.runtimeState, "stopped");
      assert.strictEqual(duringSelection.activeTarget, undefined);

      releaseSelection.resolve(undefined);
      const result = await start;
      assert.strictEqual(result.ok, true);
      assert.strictEqual(service.phase(), "running");
      assert.strictEqual(service.acceptedDebugSession(), activeSession);
    } finally {
      releaseSelection.resolve(undefined);
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("a session lost while accepted-target selection is pending never reaches Running", async () => {
    const root = fixtureRoot();
    const selectionEntered = deferred<void>();
    const releaseSelection = deferred<void>();
    let activeSession: vscode.DebugSession | undefined;
    let service!: RuntimeLifecycleService;
    try {
      service = new RuntimeLifecycleService(
        async (attemptId) => {
          activeSession = fakeSession("launch", attemptId);
          (
            service as unknown as {
              sessions: Map<string, vscode.DebugSession>;
            }
          ).sessions.set(activeSession.id, activeSession);
          return true;
        },
        undefined,
        async () => {
          selectionEntered.resolve(undefined);
          await releaseSelection.promise;
        },
        5,
      );
      const subject = service as unknown as {
        sessions: Map<string, vscode.DebugSession>;
        acceptedSessions: Set<string>;
        getStructuredTextSession: () => vscode.DebugSession | undefined;
        runtimeConfigTarget: () => vscode.Uri;
        setRuntimeMode: () => Promise<void>;
      };
      subject.getStructuredTextSession = () => activeSession;
      subject.runtimeConfigTarget = () => vscode.Uri.file(root);
      subject.setRuntimeMode = async () => undefined;
      stubSimulatorReadiness(service);

      const start = service.startLocalSimulator(async () => VALID);
      await selectionEntered.promise;
      assert.ok(activeSession);
      const duringSelection = await service.snapshot();
      assert.strictEqual(service.phase(), "starting");
      assert.strictEqual(service.acceptedDebugSession(), undefined);
      assert.strictEqual(subject.acceptedSessions.has(activeSession!.id), false);
      assert.strictEqual(duringSelection.status.running, false);
      subject.sessions.delete(activeSession!.id);
      activeSession = undefined;
      releaseSelection.resolve(undefined);

      const result = await start;
      assert.strictEqual(result.ok, false);
      assert.strictEqual(service.phase(), "stopped");
      assert.strictEqual(service.acceptedDebugSession(), undefined);
      assert.strictEqual(service.operationState(), undefined);
    } finally {
      releaseSelection.resolve(undefined);
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("external session acceptance also stays pending through target selection", async () => {
    const selectionEntered = deferred<void>();
    const releaseSelection = deferred<void>();
    const session = fakeSession("launch", "external-selection");
    const service = new RuntimeLifecycleService(
      async () => true,
      undefined,
      async () => {
        selectionEntered.resolve(undefined);
        await releaseSelection.promise;
      },
      5,
    );
    const subject = service as unknown as {
      sessions: Map<string, vscode.DebugSession>;
      acceptedSessions: Set<string>;
      starting: boolean;
      getStructuredTextSession: () => vscode.DebugSession | undefined;
      acceptExternalSession: (candidate: vscode.DebugSession) => Promise<void>;
    };
    subject.sessions.set(session.id, session);
    subject.starting = true;
    subject.getStructuredTextSession = () => session;
    stubSimulatorReadiness(service);

    const acceptance = subject.acceptExternalSession(session);
    await selectionEntered.promise;
    const duringSelection = await service.snapshot();
    assert.strictEqual(service.phase(), "starting");
    assert.strictEqual(service.acceptedDebugSession(), undefined);
    assert.strictEqual(subject.acceptedSessions.has(session.id), false);
    assert.strictEqual(duringSelection.status.running, false);

    releaseSelection.resolve(undefined);
    await acceptance;
    assert.strictEqual(service.phase(), "running");
    assert.strictEqual(service.acceptedDebugSession(), session);
  });

  test("held Start validation owns one operation and source failure returns cleanly to Stopped", async () => {
    const root = fixtureRoot();
    try {
      const { service, dapCalls } = localHarness(root);
      const validation = deferred<CheckProgramResponse>();
      let validatorCalls = 0;
      const first = service.startLocalSimulator(async () => {
        validatorCalls += 1;
        return validation.promise;
      });
      await Promise.resolve();

      assert.strictEqual(service.phase(), "starting");
      assert.deepStrictEqual(service.transitionTarget(), { kind: "simulator" });
      assert.strictEqual(service.operationState()?.kind, "local_start");
      assert.strictEqual(validatorCalls, 1);
      assert.strictEqual(dapCalls(), 0);

      const second = await service.startLocalSimulator(async () => {
        validatorCalls += 1;
        return VALID;
      });
      assert.strictEqual(second.ok, false);
      assert.strictEqual(
        validatorCalls,
        1,
        "double Start must not validate twice",
      );
      assert.strictEqual(dapCalls(), 0, "double Start must not launch DAP");

      const compile = await service.runExclusiveOperation(
        "compile",
        { kind: "simulator" },
        async () => "must not run",
      );
      assert.strictEqual(compile.acquired, false);

      validation.resolve(SOURCE_FAILURE);
      const result = await first;
      assert.ok(!result.ok && "validationRejected" in result);
      assert.strictEqual(service.phase(), "stopped");
      assert.strictEqual(service.operationState(), undefined);
      assert.strictEqual(service.localFailure(), undefined);
      assert.strictEqual(dapCalls(), 0);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("a missing Compile report fails closed before DAP launch", async () => {
    const root = fixtureRoot();
    try {
      const { service, dapCalls } = localHarness(root);
      const result = await service.startLocalSimulator(async () => undefined);

      assert.strictEqual(result.ok, false);
      assert.ok(!result.ok && "failure" in result);
      if (!result.ok && "failure" in result) {
        assert.match(result.failure.message, /did not return a validation report/);
      }
      assert.strictEqual(dapCalls(), 0);
      assert.strictEqual(service.phase(), "stopped");
      assert.strictEqual(service.operationState(), undefined);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("held successful Start carries the same operation through accepted Running", async () => {
    const root = fixtureRoot();
    try {
      const { service, dapCalls } = localHarness(root);
      const validation = deferred<CheckProgramResponse>();
      let validatorCalls = 0;
      const start = service.startLocalSimulator(async () => {
        validatorCalls += 1;
        return validation.promise;
      });
      await Promise.resolve();
      assert.strictEqual(service.phase(), "starting");
      validation.resolve(VALID);
      const result = await start;
      assert.strictEqual(result.ok, true);
      assert.strictEqual(validatorCalls, 1);
      assert.strictEqual(dapCalls(), 1);
      assert.strictEqual(service.phase(), "running");
      assert.deepStrictEqual(service.activeTarget(), { kind: "simulator" });
      assert.strictEqual(service.operationState(), undefined);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("a tracked exact session is terminated when its Start command rejects", async () => {
    const root = fixtureRoot();
    try {
      let subject!: {
        sessions: Map<string, vscode.DebugSession>;
        getStructuredTextSession: () => vscode.DebugSession | undefined;
        runtimeConfigTarget: () => vscode.Uri;
        setRuntimeMode: () => Promise<void>;
        terminateUnacceptedSession: (
          session: vscode.DebugSession,
        ) => Promise<void>;
      };
      let announced: vscode.DebugSession | undefined;
      let terminated = 0;
      const service = new RuntimeLifecycleService(async (attemptId) => {
        announced = fakeSession("launch", attemptId);
        subject.sessions.set(announced.id, announced);
        throw new Error("startDebugging rejected after announcing session");
      });
      subject = service as unknown as typeof subject;
      subject.getStructuredTextSession = () => announced;
      subject.runtimeConfigTarget = () => vscode.Uri.file(root);
      subject.setRuntimeMode = async () => undefined;
      subject.terminateUnacceptedSession = async (session) => {
        terminated += 1;
        subject.sessions.delete(session.id);
        announced = undefined;
      };

      const result = await service.startLocalSimulator(async () => VALID);
      assert.strictEqual(result.ok, false);
      assert.strictEqual(terminated, 1);
      assert.strictEqual(subject.sessions.size, 0);
      assert.strictEqual(service.phase(), "stopped");
      assert.strictEqual(service.operationState(), undefined);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
});
