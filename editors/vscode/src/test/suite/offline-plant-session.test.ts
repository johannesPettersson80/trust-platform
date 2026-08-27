import * as assert from "assert";
import { EventEmitter } from "events";

import {
  OfflinePlantCommandSession,
  type PlantCommandProcess,
} from "../../networkCanvas/plant/offlineSession";

class FakePlantProcess extends EventEmitter implements PlantCommandProcess {
  readonly stdout = new EventEmitter();
  readonly stderr = new EventEmitter();
  readonly writes: string[] = [];
  readonly stdin = {
    write: (data: string): boolean => {
      this.writes.push(data);
      return true;
    },
  };
  killed = false;

  kill(): boolean {
    this.killed = true;
    return true;
  }
}

suite("Offline plant command session", () => {
  test("correlates split JSON-line responses and decodes them before resolving", async () => {
    const process = new FakePlantProcess();
    const session = new OfflinePlantCommandSession(process);
    const pending = session.request("plant.capabilities", {
      contract: "trust.plant",
      schema_version: 1,
    });
    assert.strictEqual(process.writes.length, 1);
    const request = JSON.parse(process.writes[0]) as { id: number };
    const response = JSON.stringify({
      id: request.id,
      ok: true,
      result: {
        contract: "trust.plant",
        schema_version: 1,
        operation: "plant.capabilities",
        project_id: "project:sha256:abc",
        model_kinds: ["first_order"],
        limits: { max_trace_rows_per_plant: 100_000 },
        operations: ["plant.capabilities"],
      },
    });
    process.stdout.emit("data", Buffer.from(response.slice(0, 17)));
    process.stdout.emit("data", Buffer.from(`${response.slice(17)}\n`));

    const result = await pending;
    assert.strictEqual(result.operation, "plant.capabilities");
    assert.strictEqual(result.project_id, "project:sha256:abc");
    session.dispose();
    assert.strictEqual(process.killed, true);
  });
});
