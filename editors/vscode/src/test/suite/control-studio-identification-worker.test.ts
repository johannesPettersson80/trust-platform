import * as assert from "assert";
import { openControlStudioCommand } from "../../controlStudio/openCommand";

suite("Control Studio identification worker", () => {

  test("the public Open command waits for asynchronous editor restoration", async () => {
    let opened = false;
    await openControlStudioCommand({} as any, { project: "explicit-request" }, async () => {
      await new Promise(resolve => setTimeout(resolve, 0)); opened = true;
    });
    assert.strictEqual(opened, true, "command completion must include the restored editor");
  });
});
