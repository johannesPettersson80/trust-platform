import {
  assert,
  readSrc,
  readSrcSet,
} from "./ux-shell-contract-fixtures";

suite("Phase 6 — Update running simulation (simulator-only)", () => {
  test("Update running simulation is sim-only, gated on a real source change, wired to the update command", () => {
    const src = readSrcSet("trustHomeView.ts", "trustHomeFailures.ts");
    // sim-only + running + an actual change
    assert.ok(
      /selected\.kind === "simulator"/.test(src) &&
        /selected\.status === "running"/.test(src) &&
        /this\.sourceChanged/.test(src),
      "canApply must require simulator + running + a real source change"
    );
    // wired to the existing debug adapter update request, not a fake
    assert.ok(
      src.includes("trust-lsp.debug.reload"),
      "Update running simulation must drive the update command"
    );
    assert.ok(
      src.includes("isReloadSuccess") &&
        src.includes("Running simulation updated.") &&
        src.includes("Update failed:"),
      "Update running simulation must expose success/failure status instead of silently hiding failures"
    );
    assert.ok(
      src.includes("Fix the errors shown in Problems, then try again.") &&
        src.includes("summarizeReloadMessage"),
      "Update running simulation must summarize compiler failures for the compact sidebar instead of dumping raw paths"
    );
    assert.ok(
      /if \(isReloadSuccess\(result\)\)[\s\S]*this\.sourceChanged = false/.test(src) &&
        /else[\s\S]*this\.sourceChanged = true/.test(src),
      "Update running simulation must clear pending state only after a successful update and keep retry visible on failure"
    );
    // change detection is save-based (honest), and reset on Start/Apply
    assert.ok(
      src.includes("onDidSaveTextDocument") && src.includes("markSourceChanged"),
      "source-change must be detected from an actual ST save"
    );
  });
  test("debug reload LM tool reports command failure honestly", () => {
    const src = readSrc("lm-tools/debugTools.ts");
    assert.ok(
      src.includes("executeCommand<CommandResult>") &&
        src.includes("trust-lsp.debug.reload"),
      "the LM reload tool must inspect the structured reload command result"
    );
    assert.ok(
      src.includes("result.ok === false") &&
        src.includes("Failed to update running simulation:"),
      "the LM reload tool must not report success when Update failed"
    );
  });
});
