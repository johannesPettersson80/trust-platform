import * as assert from "assert";
import { describeError, withRetries } from "../../acceptance/download";

// Specification 22, "Acceptance journeys on the packaged extension": the runner tries the VS Code download three times
// and prints a failure's whole value (checklist G102: the update service's lookup rejected with an empty error).
suite("Acceptance runner retries (specification 22)", () => {
  test("a download that fails twice is tried a third time, each failure logged before its pause", async () => {
    let calls = 0;
    const events: string[] = [];
    const value = await withRetries(
      async () => {
        calls += 1;
        if (calls < 3) throw new Error(`lookup ${calls}`);
        return "code";
      },
      3,
      async (failed) => {
        events.push(`pause ${failed}`);
      },
      (line) => events.push(line),
    );
    assert.strictEqual(value, "code");
    assert.strictEqual(calls, 3);
    assert.deepStrictEqual(events, [
      "attempt 1 of 3 failed: lookup 1",
      "pause 1",
      "attempt 2 of 3 failed: lookup 2",
      "pause 2",
    ]);
  });

  test("three failures end with the third, after two pauses", async () => {
    let calls = 0;
    let pauses = 0;
    await assert.rejects(
      withRetries(
        async () => {
          calls += 1;
          throw new Error(`lookup ${calls}`);
        },
        3,
        async () => {
          pauses += 1;
        },
        () => undefined,
      ),
      /lookup 3/,
    );
    assert.strictEqual(calls, 3);
    assert.strictEqual(pauses, 2);
  });

  test("an error without a message is described by its stack, any other value by its JSON", () => {
    const empty = new Error("");
    assert.strictEqual(describeError(empty), empty.stack);
    assert.strictEqual(describeError({ statusCode: 503 }), '{"statusCode":503}');
    assert.strictEqual(describeError("down"), '"down"');
    assert.strictEqual(describeError(new Error("lookup failed")), "lookup failed");
  });
});
