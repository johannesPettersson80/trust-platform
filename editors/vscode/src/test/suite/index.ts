import Mocha from "mocha";
import {
  attachConfiguredMochaEvidence,
  configuredMochaGrep,
} from "./mochaSelection";

export function run(): Promise<void> {
  const mocha = new Mocha({
    ui: "tdd",
    color: true,
  });
  const selectedTests = configuredMochaGrep(process.env);
  if (selectedTests) {
    mocha.grep(selectedTests);
  }

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
  require("./control-engineering-fixtures.test");
  require("./control-studio-numerics.test");
  require("./control-lesson-schema.test");
  require("./control-lesson-source-structure.test");
  require("./control-curriculum-catalog.test");
  require("./control-lesson-pid-production.test");
  require("./control-lesson-selection-modelling.test");
  require("./control-lesson-state-space.test");
  require("./control-lesson-tuning-methods.test");
  require("./control-lesson-industrial-structures.test");
  require("./control-lesson-capstones.test");
  require("./control-curriculum-readmes.test");
  require("./runtime-target.test");
  require("./network-canvas.test");
  require("./network-canvas-message-router.test");
  require("./plant-contract.test");
  require("./offline-plant-session.test");
  require("./plant-service.test");
  require("./plant-topology.test");
  require("./plant-picker.test");
  require("./plant-editor-model.test");
  require("./plant-scenario-events.test");
  require("./plant-signal-catalog.test");
  require("./control-studio-handoff.test");
  require("./control-studio-loop.test");
  require("./control-studio-loop-monitor.test");
  require("./control-studio-command.test");
  require("./control-studio-experiment.test");
  require("./control-studio-identify-tune.test");
  require("./control-studio-validate-apply.test");
  require("./control-studio-chart.test");
  require("./control-studio-sidebar.test");
  require("./control-studio-plant-monitor.test");
  require("./network-canvas-fleet-identity.test");
  require("./ads-status-summary.test");
  require("./connector-status-contract.test");
  require("./libraries-model.test");
  require("./library-code-actions.test");
  require("./snippets.test");
  require("./st-tests.integration.test");
  require("./mocha-selection.test");

  return new Promise((resolve, reject) => {
    const runner = mocha.run((failures: number) => {
      if (failures > 0) {
        reject(new Error(`${failures} test(s) failed.`));
      } else {
        resolve();
      }
    });
    attachConfiguredMochaEvidence(runner, process.env);
  });
}
