import Mocha from "mocha";

export function run(): Promise<void> {
  const mocha = new Mocha({
    ui: "tdd",
    color: true,
  });

  mocha.suite.emit("pre-require", global, "nofile", mocha);
  require("./diagnostics.test");
  require("./check-program.integration.test");
  require("./debug-io.integration.test");
  require("./hmi.integration.test");
  require("./lsp.integration.test");
  require("./runtime-default-settings.integration.test");
  require("./lm-tools-contract.test");
  require("./runtime-controls-contract.test");
  require("./ux-shell-contract.test");
  require("./opcua-client-model.test");
  require("./new-project.test");
  require("./plcopen-export.test");
  require("./plcopen-import.test");
  require("./plcopen-ld-interop.test");
  require("./plcopen-runtime-errors.test");
  require("./blockly-engine.test");
  require("./ladder-engine.test");
  require("./ladder-schema.test");
  require("./ladder-editor-ops.test");
  require("./ladder-runtime-io-panel.test");
  require("./visual-companion.test");
  require("./visual-runtime-controller.test");
  require("./visual-runtime-panel-bridge.test");
  require("./visual-right-pane-resize.test");
  require("./visual-webview-vscode-api.test");
  require("./statechart-editor.lifecycle.test");
  require("./statechart-engine.test");
  require("./sfc-engine.test");
  require("./statechart-runtime-client.test");
  require("./runtime-shared-utils.test");
  require("./runtime-control-client.test");
  require("./runtime-target.test");
  require("./network-canvas.test");
  require("./network-canvas-github-issues.test");
  require("./network-canvas-session-model.test");
  require("./ads-status-summary.test");
  require("./libraries-model.test");
  require("./library-code-actions.test");
  require("./snippets.test");
  require("./st-tests.integration.test");

  return new Promise((resolve, reject) => {
    mocha.run((failures: number) => {
      if (failures > 0) {
        reject(new Error(`${failures} test(s) failed.`));
      } else {
        resolve();
      }
    });
  });
}
