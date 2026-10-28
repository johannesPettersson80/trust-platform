import * as assert from "assert";
import { getVsCodeApi } from "../../visual/runtime/webview/vscodeApi";

const CACHE_KEY = "TrustHmi_lsp_vscode_api__";

suite("Visual webview VS Code API singleton", () => {
  const originalAcquire = (globalThis as any).acquireVsCodeApi;

  teardown(() => {
    if (originalAcquire === undefined) {
      delete (globalThis as any).acquireVsCodeApi;
    } else {
      (globalThis as any).acquireVsCodeApi = originalAcquire;
    }
    delete (globalThis as any)[CACHE_KEY];
  });

  test("acquires VS Code API only once per webview runtime", () => {
    let acquireCalls = 0;
    const api = {
      postMessage: () => undefined,
      getState: () => undefined,
      setState: () => undefined,
    };

    (globalThis as any).acquireVsCodeApi = () => {
      acquireCalls += 1;
      return api;
    };
    delete (globalThis as any)[CACHE_KEY];

    const first = getVsCodeApi();
    const second = getVsCodeApi();

    assert.strictEqual(first, api);
    assert.strictEqual(second, api);
    assert.strictEqual(acquireCalls, 1);
  });

  test("returns a stable no-op API when VS Code API is unavailable", () => {
    delete (globalThis as any).acquireVsCodeApi;
    delete (globalThis as any)[CACHE_KEY];

    const first = getVsCodeApi();
    const second = getVsCodeApi();

    assert.strictEqual(first, second);
    assert.strictEqual(typeof first.postMessage, "function");
    assert.doesNotThrow(() => first.postMessage({ type: "noop" }));
  });
});
