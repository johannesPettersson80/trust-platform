import * as assert from "assert";
import { StateMachineEngine } from "../../statechart/stateMachineEngine";

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

type ForceCall = { address: string; value: unknown };

class MockRuntimeClient {
  public readonly forced: ForceCall[] = [];

  constructor(
    private readonly readResult: unknown = true,
    private readonly forceDelayMs = 0
  ) {}

  isConnected(): boolean {
    return true;
  }

  async forceIo(address: string, value: unknown): Promise<void> {
    if (this.forceDelayMs > 0) {
      await delay(this.forceDelayMs);
    }
    this.forced.push({ address, value });
  }

  async unforceIo(_address: string): Promise<void> {
    // No-op for tests.
  }

  async readIo(_address: string): Promise<unknown> {
    if (this.readResult instanceof Error) {
      throw this.readResult;
    }
    return this.readResult;
  }
}

suite("StateMachineEngine", function () {
  this.timeout(10000);

  test("awaits hardware actions before transition completes", async () => {
    const runtime = new MockRuntimeClient(true, 15);
    const config = {
      id: "order-check",
      initial: "Idle",
      states: {
        Idle: {
          entry: ["idleEntry"],
          exit: ["idleExit"],
          on: {
            GO: {
              target: "Running",
              actions: ["transitionAction"],
            },
          },
        },
        Running: {
          entry: ["runningEntry"],
        },
      },
      actionMappings: {
        idleEntry: {
          action: "WRITE_OUTPUT",
          address: "%QX0.0",
          value: false,
        },
        idleExit: {
          action: "WRITE_OUTPUT",
          address: "%QX0.1",
          value: false,
        },
        transitionAction: {
          action: "WRITE_OUTPUT",
          address: "%QX0.2",
          value: true,
        },
        runningEntry: {
          action: "WRITE_OUTPUT",
          address: "%QX0.3",
          value: true,
        },
      },
    };

    const engine = new StateMachineEngine(
      JSON.stringify(config),
      "hardware",
      runtime as any
    );

    await engine.initialize();
    assert.deepStrictEqual(runtime.forced.map((call) => call.address), ["%QX0.0"]);
    runtime.forced.length = 0;

    const transitioned = await engine.sendEvent("GO");
    assert.strictEqual(transitioned, true);
    assert.strictEqual(engine.getCurrentState(), "Running");
    assert.deepStrictEqual(runtime.forced.map((call) => call.address), [
      "%QX0.1",
      "%QX0.2",
      "%QX0.3",
    ]);
  });

  test("fails closed when guard expression is invalid in hardware mode", async () => {
    const runtime = new MockRuntimeClient(true);
    const config = {
      id: "guard-invalid",
      initial: "Idle",
      states: {
        Idle: {
          on: {
            GO: {
              target: "Running",
              guard: "INVALID_GUARD",
            },
          },
        },
        Running: {},
      },
    };

    const engine = new StateMachineEngine(
      JSON.stringify(config),
      "hardware",
      runtime as any
    );

    await engine.initialize();
    const transitioned = await engine.sendEvent("GO");
    assert.strictEqual(transitioned, false);
    assert.strictEqual(engine.getCurrentState(), "Idle");
  });

  test("fails closed when guard I/O read errors in hardware mode", async () => {
    const runtime = new MockRuntimeClient(new Error("io read failed"));
    const config = {
      id: "guard-read-error",
      initial: "Idle",
      states: {
        Idle: {
          on: {
            GO: {
              target: "Running",
              guard: "%IX0.0 == TRUE",
            },
          },
        },
        Running: {},
      },
    };

    const engine = new StateMachineEngine(
      JSON.stringify(config),
      "hardware",
      runtime as any
    );

    await engine.initialize();
    const transitioned = await engine.sendEvent("GO");
    assert.strictEqual(transitioned, false);
    assert.strictEqual(engine.getCurrentState(), "Idle");
  });
});
