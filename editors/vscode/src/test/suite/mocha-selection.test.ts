import * as assert from "assert";
import { EventEmitter } from "events";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

type MochaSelectionModule = {
  configuredMochaGrep?: (
    environment: Readonly<Record<string, string | undefined>>
  ) => RegExp | undefined;
  attachConfiguredMochaEvidence?: (
    runner: EventEmitter,
    environment: Readonly<Record<string, string | undefined>>
  ) => boolean;
};

function loadMochaSelection(): MochaSelectionModule {
  try {
    return require("./mochaSelection") as MochaSelectionModule;
  } catch {
    return {};
  }
}

suite("VS Code test selection", () => {
  test("uses an opt-in grep while preserving the unfiltered default", () => {
    const selection = loadMochaSelection();
    assert.strictEqual(
      typeof selection.configuredMochaGrep,
      "function",
      "expected the VS Code test harness to expose configurable Mocha selection"
    );

    const configuredMochaGrep = selection.configuredMochaGrep!;
    assert.strictEqual(configuredMochaGrep({}), undefined);
    assert.strictEqual(
      configuredMochaGrep({ TRUST_VSCODE_TEST_GREP: "   " }),
      undefined
    );

    const selected = configuredMochaGrep({
      TRUST_VSCODE_TEST_GREP: "ladder|statechart",
    });
    assert.ok(selected);
    assert.strictEqual(selected.source, "ladder|statechart");
    assert.ok(selected.test("ladder emits Structured Text"));
    assert.ok(selected.test("statechart enters the initial state"));
    assert.ok(!selected.test("diagnostics publish parser errors"));

    assert.strictEqual(
      typeof selection.attachConfiguredMochaEvidence,
      "function",
      "expected the VS Code test harness to expose machine-readable evidence"
    );
    const evidenceRoot = fs.mkdtempSync(
      path.join(os.tmpdir(), "trust-vscode-mocha-evidence-")
    );
    try {
      const evidencePath = path.join(evidenceRoot, "nested", "evidence.json");
      const runner = new EventEmitter();
      const attached = selection.attachConfiguredMochaEvidence!(
        runner,
        {
          TRUST_VSCODE_TEST_EVIDENCE: evidencePath,
          TRUST_VSCODE_TEST_GREP: "selected test$",
          TRUST_VSCODE_TEST_SOURCE_COMMIT: "abc123",
        }
      );
      assert.strictEqual(attached, true);
      runner.emit("pass", {
        title: "selected test",
        duration: 7,
        fullTitle: () => "suite selected test",
      });
      runner.emit(
        "fail",
        {
          title: "failed test",
          duration: 11,
          fullTitle: () => "suite failed test",
        },
        new Error("expected failure")
      );
      runner.emit("end");

      const evidence = JSON.parse(fs.readFileSync(evidencePath, "utf8"));
      assert.strictEqual(evidence.schema_version, 1);
      assert.strictEqual(evidence.source_commit, "abc123");
      assert.strictEqual(evidence.selector, "selected test$");
      assert.strictEqual(evidence.test_count, 2);
      assert.strictEqual(evidence.passed_count, 1);
      assert.strictEqual(evidence.failed_count, 1);
      assert.deepStrictEqual(
        evidence.results.map(
          (result: { full_title: string; status: string; duration_ms: number }) => [
            result.full_title,
            result.status,
            result.duration_ms,
          ]
        ),
        [
          ["suite selected test", "passed", 7],
          ["suite failed test", "failed", 11],
        ]
      );
    } finally {
      fs.rmSync(evidenceRoot, { recursive: true, force: true });
    }
  });
});
