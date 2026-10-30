import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { createRequire } from "module";
import * as vm from "vm";
import * as vscode from "vscode";

/** Execute the real lifecycle module with only the editor's event transport
 * replaced. No runtime values or lifecycle ordering are implemented here. */
function harness() {
  const callbacks: Record<string, (event: any) => void> = {};
  const noop = () => ({ dispose() {} });
  const subscribe = (name: string) => (callback: (event: any) => void) => { callbacks[name] = callback; return noop(); };
  const debug: any = { activeDebugSession: undefined,
    onDidReceiveDebugSessionCustomEvent: subscribe("io"), onDidStartDebugSession: subscribe("start"),
    onDidTerminateDebugSession: subscribe("end"), onDidChangeActiveDebugSession: subscribe("active") };
  const api = { EventEmitter: vscode.EventEmitter, debug, workspace: { onDidChangeConfiguration: noop } };
  const filename = path.resolve(__dirname, "../../runtimeLifecycle.js");
  const nativeRequire = createRequire(filename);
  const module = { exports: {} as any };
  vm.runInNewContext(fs.readFileSync(filename, "utf8"), { module, exports: module.exports,
    require: (name: string) => name === "vscode" ? api : nativeRequire(name),
    __filename: filename, __dirname: path.dirname(filename), process, console, setTimeout, clearTimeout,
    setInterval, clearInterval }, { filename });
  const service = module.exports.runtimeLifecycleService;
  const subscriptions: vscode.Disposable[] = [];
  service.register({ subscriptions });
  const type = nativeRequire("./debug/configuration").DEBUG_TYPE;
  const start = (id: string) => {
    const session = { id, type, configuration: {}, customRequest: async () => ({}) };
    debug.activeDebugSession = session; callbacks.start(session); return session;
  };
  const send = (session: unknown, scan: number) => callbacks.io({ session, event: "stIoState",
    body: { scan, inputs: [{ address: "%IL0", value: String(scan) }], outputs: [], memory: [] } });
  return { service, start, send, dispose: () => subscriptions.forEach(subscription => subscription.dispose()) };
}

suite("Runtime lifecycle I/O observations", () => {
  test("ignores delayed lower scans without replacing a newer same-session snapshot", () => {
    const h = harness();
    try {
      const session = h.start("runtime-a");
      let changes = 0; h.service.onDidIoStateChange(() => changes++);
      h.send(session, 337); const current = h.service.lastIoState;
      assert.strictEqual(current.scan, 337);
      h.send(session, 336);
      assert.strictEqual(h.service.lastIoState, current, "delayed I/O must not replace the newer values or refresh their identity");
      assert.strictEqual(changes, 1, "an ignored delayed event must not publish another observation");
      h.send(session, 338); assert.strictEqual(h.service.lastIoState.scan, 338);
    } finally { h.dispose(); }
  });

  test("keeps I/O with its tracked session and accepts a lower scan in a new session", () => {
    const h = harness();
    try {
      const first = h.start("runtime-a"); h.send(first, 337);
      h.send({ id: "foreign", type: first.type }, 999);
      assert.strictEqual(h.service.lastIoState.scan, 337, "an untracked session must not supply active I/O");
      const next = h.start("runtime-b");
      assert.strictEqual(h.service.lastIoState.scan, undefined, "new runtime has no old-runtime scan");
      h.send(next, 1); assert.strictEqual(h.service.lastIoState.scan, 1);
      h.send(first, 338); assert.strictEqual(h.service.lastIoState.scan, 1, "old session cannot replace the active runtime");
    } finally { h.dispose(); }
  });
});
