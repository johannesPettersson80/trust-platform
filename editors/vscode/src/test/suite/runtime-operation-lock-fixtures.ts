import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import type { CheckProgramResponse } from "../../checkProgramModel";
import { LIFECYCLE_START_ATTEMPT_FIELD } from "../../debug/startAttempt";
import { RuntimeLifecycleService } from "../../runtimeLifecycle";
import type { RuntimeLifecycleResult } from "../../runtimeLifecycleModel";

export const VALID: CheckProgramResponse = {
  version: 1,
  ok: true,
  status: "ok",
  errors: 0,
  warnings: 0,
  issues: [],
};

export const SOURCE_FAILURE: CheckProgramResponse = {
  version: 1,
  ok: false,
  status: "failed",
  errors: 1,
  warnings: 0,
  issues: [
    {
      severity: "error",
      code: "compile",
      file: "src/program.st",
      message: "test source error",
    },
  ],
};

export function deferred<T>(): {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
} {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

export function fixtureRoot(): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-operation-lock-"));
  fs.writeFileSync(
    path.join(root, "runtime.toml"),
    [
      "[runtime.control]",
      'endpoint = "tcp://127.0.0.1:9902"',
      'auth_token = "already-secured-test-token-1234567890"',
      "",
    ].join("\n"),
  );
  return root;
}

export function fakeSession(
  request: "launch" | "attach",
  attemptId = "test-attempt",
  endpoint = "tcp://remote.test:5680",
): vscode.DebugSession {
  return {
    id: `${request}-session`,
    type: "structured-text",
    name: request === "launch" ? "truST Simulator" : "truST Remote",
    workspaceFolder: undefined,
    configuration: {
      type: "structured-text",
      request,
      name: "test",
      [LIFECYCLE_START_ATTEMPT_FIELD]: attemptId,
      ...(request === "attach" ? { endpoint, targetLabel: "Remote PLC" } : {}),
    },
    customRequest: async () => undefined,
    getDebugProtocolBreakpoint: async () => undefined,
  } as vscode.DebugSession;
}

export function stubSimulatorReadiness(
  service: RuntimeLifecycleService,
): void {
  const subject = service as unknown as {
    liveValues: {
      waitForSimulatorSessionReady: () => Promise<RuntimeLifecycleResult>;
    };
  };
  subject.liveValues.waitForSimulatorSessionReady = async () => ({
    ok: true,
    message: "ready",
  });
}

export function localHarness(root: string): {
  readonly service: RuntimeLifecycleService;
  readonly dapCalls: () => number;
} {
  let dapCallCount = 0;
  let activeSession: vscode.DebugSession | undefined;
  const service = new RuntimeLifecycleService(
    async (attemptId) => {
      dapCallCount += 1;
      activeSession = fakeSession("launch", attemptId);
      (
        service as unknown as { sessions: Map<string, vscode.DebugSession> }
      ).sessions.set(activeSession.id, activeSession);
      return true;
    },
    undefined,
    undefined,
    5,
  );
  const subject = service as unknown as Record<string, unknown>;
  subject.getStructuredTextSession = () => activeSession;
  subject.runtimeConfigTarget = () => vscode.Uri.file(root);
  subject.setRuntimeMode = async () => undefined;
  stubSimulatorReadiness(service);
  subject.waitForSessionStillPresent = async () => true;
  subject.terminateUnacceptedSession = async () => {
    activeSession = undefined;
  };
  return {
    service,
    dapCalls: () => dapCallCount,
  };
}

export function acceptSession(
  service: RuntimeLifecycleService,
  session: vscode.DebugSession,
): void {
  const subject = service as unknown as {
    acceptedSessions: Set<string>;
    getStructuredTextSession: () => vscode.DebugSession | undefined;
  };
  subject.getStructuredTextSession = () => session;
  subject.acceptedSessions.add(session.id);
}

export function liveValuesTabs(): vscode.Tab[] {
  return vscode.window.tabGroups.all
    .flatMap((group) => [...group.tabs])
    .filter(
      (tab) =>
        tab.label === "Live Values" ||
        (tab.input instanceof vscode.TabInputWebview &&
          tab.input.viewType === "trust-io-panel"),
    );
}

export async function closeLiveValuesTabs(): Promise<void> {
  const tabs = liveValuesTabs();
  if (tabs.length > 0) {
    await vscode.window.tabGroups.close(tabs, true);
  }
}
