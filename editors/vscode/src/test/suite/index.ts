import Mocha from "mocha";

export function run(): Promise<void> {
  const grep = process.env.ST_VSCODE_TEST_GREP?.trim();
  const mocha = new Mocha({
    ui: "tdd",
    color: true,
    ...(grep ? { grep } : {}),
  });

  mocha.suite.emit("pre-require", global, "nofile", mocha);
  require("./diagnostics.test");
  require("./check-program.integration.test");
  require("./debug-io.integration.test");
  require("./debug-log-redaction.test");
  require("./hmi.integration.test");
  require("./lsp.integration.test");
  require("./runtime-default-settings.integration.test");
  require("./live-values-passive-contract.test");
  require("./ads-live-values.test");
  require("./ads-import-project.test");
  require("./product-runtime-identity.test");
  require("./lm-tools-contract.test");
  require("./runtime-controls-contract.test");
  require("./runtime-controls-surface-contract.test");
  require("./runtime-operation-lock-contract-policy.test");
  require("./runtime-operation-lock-contract-authority.test");
  require("./runtime-operation-lock-contract-presentation.test");
  require("./runtime-start-recovery-contract.test");
  require("./runtime-operation-lock-start.test");
  require("./runtime-operation-lock-conflicts.test");
  require("./runtime-lifecycle-ux-contract.test");
  require("./ux-shell-palette.test");
  require("./ux-shell-navigation.test");
  require("./ux-shell-examples.test");
  require("./ux-shell-packaged-runtime.test");
  require("./ux-shell-live-values-actions.test");
  require("./ux-shell-live-values-lifecycle.test");
  require("./ux-shell-devices.test");
  require("./ux-shell-backend-gating.test");
  require("./ux-shell-update-simulation.test");
  require("./ux-shell-runtime-auth.test");
  require("./ux-shell-compile.test");
  require("./ux-shell-libraries.test");
  require("./ux-shell-visual-theme.test");
  require("./ux-shell-visual-editors.test");
  require("./ux-shell-visual-devices.test");
  require("./ux-shell-visual-canvas.test");
  require("./ux-shell-ads-lifecycle-contract.test");
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
  require("./network-canvas-model.test");
  require("./network-canvas-protocols.test");
  require("./network-canvas-fleet.test");
  require("./network-canvas-managed.test");
  require("./network-canvas-picker.test");
  require("./network-canvas-expose.test");
  require("./network-canvas-lifecycle-failures.test");
  require("./network-canvas-github-issues.test");
  require("./network-canvas-session-model.test");
  require("./windows-ads-discovery-contract.test");
  require("./windows-ads-zero-input-contract.test");
  require("./ads-service-probe-safety-runtime.test");
  require("./ads-service-probe-safety-session.test");
  require("./ads-service-probe-safety-recovery.test");
  require("./ads-service-probe-ux-contract.test");
  require("./windows-runtime-control-migration.test");
  require("./windows-runtime-control-preflight.test");
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
