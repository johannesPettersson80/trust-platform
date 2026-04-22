import * as assert from "assert";

import {
  formatPlcopenCommandFailure,
  formatPlcopenLaunchError,
} from "../../plcopenRuntimeErrors";

suite("PLCopen runtime error formatting", () => {
  test("import launch errors distinguish missing trust-runtime binary", () => {
    const message = formatPlcopenLaunchError(
      "import",
      "spawn trust-runtime ENOENT"
    );

    assert.ok(message.includes("could not start"));
    assert.ok(message.includes("trust-runtime binary was not found"));
    assert.ok(message.includes("trust-lsp.runtime.cli.path"));
  });

  test("import command failures distinguish malformed XML", () => {
    const message = formatPlcopenCommandFailure(
      "import",
      1,
      "failed to parse PLCopen XML '/tmp/input.xml': expected a quote not '>'"
    );

    assert.ok(message.includes("input XML is malformed"));
    assert.ok(message.includes("failed to parse PLCopen XML"));
  });

  test("export command failures remain generic", () => {
    const message = formatPlcopenCommandFailure(
      "export",
      2,
      "invalid project folder '/tmp/demo': missing src/ directory"
    );

    assert.strictEqual(
      message,
      "PLCopen export failed (exit 2). invalid project folder '/tmp/demo': missing src/ directory"
    );
  });
});
