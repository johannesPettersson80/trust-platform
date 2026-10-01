import * as assert from "assert";
import { CoalescedTask } from "../../controlStudio/coalescedTask";

const pause = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

suite("Control Studio coalesced draft retention", () => {
  test("retains a burst of settings edits once, after the edits pause", async () => {
    const task = new CoalescedTask(40);
    const runs: string[] = [];
    task.schedule(() => runs.push("first"));
    task.schedule(() => runs.push("second"));
    task.schedule(() => runs.push("third"));
    assert.deepStrictEqual(runs, [], "an edit does not retain the whole study while the engineer is still typing");
    await pause(80);
    assert.deepStrictEqual(runs, ["third"], "one retention of the latest state after the burst");
  });

  test("a cancelled request never runs", async () => {
    const task = new CoalescedTask(20);
    const runs: string[] = [];
    task.schedule(() => runs.push("edit"));
    task.cancel();
    await pause(50);
    assert.deepStrictEqual(runs, []);
  });
});
