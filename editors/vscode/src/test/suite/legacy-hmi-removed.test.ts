import * as assert from "assert";
import * as vscode from "vscode";

// Specification 39, decision 12: the legacy HMI preview, its commands, its setting and its
// language-model tools are removed; the HMI Builder replaces them.
const LEGACY_COMMANDS = [
  "trust-lsp.hmi.openPreview",
  "trust-lsp.hmi.refreshFromDescriptor",
  "trust-lsp.hmi.init",
];

const LEGACY_TOOLS = [
  "trust_hmi_get_bindings",
  "trust_hmi_get_layout",
  "trust_hmi_apply_patch",
  "trust_hmi_plan_intent",
  "trust_hmi_trace_capture",
  "trust_hmi_generate_candidates",
  "trust_hmi_validate",
  "trust_hmi_preview_snapshot",
  "trust_hmi_run_journey",
  "trust_hmi_explain_widget",
  "trust_hmi_init",
];

suite("Legacy HMI removal", () => {
  suiteSetup(async () => {
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
  });

  test("the legacy HMI commands are not registered", async () => {
    const commands = new Set(await vscode.commands.getCommands(true));
    const present = LEGACY_COMMANDS.filter((command) => commands.has(command));
    assert.deepStrictEqual(present, []);
  });

  test("the legacy HMI language-model tools are not registered", () => {
    const tools = new Set(vscode.lm.tools.map((tool) => tool.name));
    const present = LEGACY_TOOLS.filter((tool) => tools.has(tool));
    assert.deepStrictEqual(present, []);
  });

  test("the legacy HMI refresh setting is not contributed", () => {
    const setting = vscode.workspace.getConfiguration("trust").inspect("hmi.refreshIntervalMs");
    assert.strictEqual(setting?.defaultValue, undefined);
  });
});
