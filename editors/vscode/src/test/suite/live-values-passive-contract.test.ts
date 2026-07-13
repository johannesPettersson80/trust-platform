import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

function extensionRoot(): string {
  return path.resolve(__dirname, "../../..");
}

function source(relativePath: string): string {
  return fs.readFileSync(path.join(extensionRoot(), "src", relativePath), "utf8");
}

suite("Live Values passive lifecycle contract", () => {
  test("the shipped panel has one production HTML owner", () => {
    const root = extensionRoot();
    const host = source("ioPanel.ts");
    const html = source("io-panel/html.ts");
    const packageJson = fs.readFileSync(path.join(root, "package.json"), "utf8");
    const buildScript = fs.readFileSync(
      path.join(root, "scripts", "build-io-panel.js"),
      "utf8"
    );

    assert.ok(
      host.includes('import { ioPanelHtml } from "./io-panel/html"') &&
        host.includes("panel.webview.html = ioPanelHtml(panel.webview, context.extensionUri)") &&
        html.includes("export function ioPanelHtml(") &&
        !host.includes("function getHtml("),
      "Live Values must keep one explicit HTML owner outside the host controller"
    );
    assert.ok(
      !host.includes("io-panel/view") &&
        buildScript.includes('"src/ioPanel.webview.js"') &&
        buildScript.includes('"src/ioPanelAdsRows.webview.js"') &&
        !buildScript.includes("io-panel/view"),
      "the host and panel build must use the split shipped sources without reactivating the retired view"
    );
    assert.ok(
      packageJson.includes('"clean:out"') &&
        packageJson.includes("npm run clean:out && tsc -p ./"),
      "production compilation must remove stale out/io-panel/view artifacts before packaging"
    );
  });

  test("rendered HTML and accepted messages cannot start, stop, or retarget a runtime", () => {
    const host = source("ioPanel.ts");
    const webview = source("ioPanel.webview.js");
    const messageHandler = host.slice(
      host.indexOf("function handleWebviewMessage"),
      host.indexOf("function collectSettingsSnapshot")
    );

    for (const retired of [
      'id="runtimeStart"',
      'id="modeSimulate"',
      'id="modeOnline"',
      'case "startDebug"',
      'case "compileAndStart"',
      'case "stopDebug"',
      'case "runtimeStart"',
      'case "runtimeSetMode"',
    ]) {
      assert.ok(
        !host.includes(retired),
        `production Live Values must not expose ${retired}`
      );
    }
    for (const retired of [
      "runtimeStart",
      "modeSimulate",
      "modeOnline",
      'type: "runtimeStart"',
      'type: "runtimeSetMode"',
    ]) {
      assert.ok(
        !webview.includes(retired),
        `shipped Live Values script must not retain ${retired}`
      );
    }
    assert.ok(
      !host.includes("runtimeLifecycleService.startRuntime(") &&
        !host.includes("runtimeLifecycleService.stopRuntime(") &&
        !host.includes("runtimeLifecycleService.setRuntimeMode(") &&
        !host.includes('executeCommand<boolean>(\n      "trust-lsp.debug.start"') &&
        !host.includes('executeCommand<boolean>(\n      "trust-lsp.debug.stop"'),
      "forged Live Values messages must not reach a hidden lifecycle function"
    );

    for (const supported of [
      'case "refresh"',
      'case "writeInput"',
      'case "forceInput"',
      'case "releaseInput"',
      'case "releaseAllForces"',
      'case "requestSettings"',
      'case "saveSettings"',
    ]) {
      assert.ok(
        messageHandler.includes(supported),
        `${supported} must remain available on passive Live Values`
      );
    }
  });
});
