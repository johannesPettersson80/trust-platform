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
    assert.ok(message.includes("trust.runtime.executablePath"));
  });

  test("import command failures show trust-runtime's text as it is", () => {
    // trust-runtime says that the input XML is malformed (specification 25 §11.3); the extension adds no rule of its own.
    const message = formatPlcopenCommandFailure(
      "import",
      1,
      "Error: failed to parse PLCopen XML '/tmp/input.xml': the input XML is malformed\n\nCaused by:\n    unexpected end of stream"
    );

    assert.strictEqual(
      message,
      "PLCopen import failed (exit 1). Error: failed to parse PLCopen XML '/tmp/input.xml': the input XML is malformed Caused by: unexpected end of stream"
    );
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
