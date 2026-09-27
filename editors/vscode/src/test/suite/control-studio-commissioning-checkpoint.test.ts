import * as assert from "assert";

suite("Control Studio commissioning checkpoint decoder", () => {
  test("accepts runtime recordings retained in rawSamples while monitor samples are empty", () => {
    const { decodeCommissioningCheckpoint, completedCommissioningMatchesCheckpoint } = require("../../controlStudio/commissioningCheckpoint") as typeof import("../../controlStudio/commissioningCheckpoint");
    const loop = {
      id: "loop:1", revision: "loop-revision", projectId: "project:1", runtimeId: "runtime:1",
      parameterBinding: { loop_id: "loop:1", task_name: "MainTask", parameters: [{ symbol: "Main.Kp", iec_type: "LREAL", unit: "1", convention: "gain", minimum: 0, maximum: 10 }], mode: { symbol: "Main.Manual", manual_value: true } },
    } as any;
    const model = { revision: "model:1", source: "provided", validation: "unvalidated", evidence: [], delaySeconds: 0,
      kind: "transfer-function", numerator: [1], denominator: [1, 1],
      inputs: [{ id: "u", unit: "1", operatingRange: { minimum: -10, maximum: 10 } }],
      outputs: [{ id: "y", unit: "1", operatingRange: { minimum: -10, maximum: 10 } }] };
    const checkpoint: any = {
      id: "checkpoint:1", loopRevision: "loop-revision", stage: "completed", model,
      task: { referenceChange: 1, maximumError: 0.2, settlingBand: 0.1, settlingTimeSeconds: 1, durationSeconds: 2 },
      origin: { supports_request_id: true, transaction_id: "parameter:1", program_revision: "program:1", binding_revision: "binding:1", active_revision: "active:1", values: { "Main.Kp": 1 }, status: "applied", previous_revision: null, previous_values: null, reason: null },
      candidate: { "Main.Kp": 2 }, parameterOperation: { id: "parameter:1", program: "program:1", binding: "binding:1" },
      references: { "Main.SP": 0 }, binding: { loop_id: "loop:1" }, recoveryTracking: {}, tolerance: 0.1,
      recordingResult: { samples: [], rawSamples: [{}], evidence: { method: "runtime_scan_buffer", completeness: "incomplete", observedSampleCount: 1, missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: false, freshnessAvailable: false, reason: "retained" } },
    };
    assert.ok(decodeCommissioningCheckpoint(checkpoint, loop));
    const mismatched = structuredClone(checkpoint);
    mismatched.recordingResult.evidence.observedSampleCount = 0;
    assert.strictEqual(decodeCommissioningCheckpoint(mismatched, loop), undefined);
    const receipt = { status: "passed", recoveryConfirmed: true, modelRevision: model.revision, parameters: { program_revision: "program:1", binding_revision: "binding:1", transaction_id: "parameter:1", values: { "Main.Kp": 2 } },
      transfers: [{ transaction: { id: "transfer:1", program_revision: "program:1", binding_revision: "transfer:1", confirmation: "output-image" } }] } as any;
    checkpoint.transfer = { id: "transfer:1", program: "program:1", binding: "transfer:1" };
    assert.strictEqual(completedCommissioningMatchesCheckpoint(receipt, checkpoint), true);
    assert.strictEqual(completedCommissioningMatchesCheckpoint(receipt, { ...checkpoint, stage: "recovered" }), false);
    const olderReceipt = structuredClone(receipt);
    olderReceipt.transfers[0].transaction.id = "older-transfer";
    assert.strictEqual(completedCommissioningMatchesCheckpoint(olderReceipt, checkpoint), false,
      "an earlier operation with the same model and gains is not this checkpoint's receipt");
    assert.strictEqual(completedCommissioningMatchesCheckpoint({ ...receipt, modelRevision: "model:other" }, checkpoint), false);
  });
});
