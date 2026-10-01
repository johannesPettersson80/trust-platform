import * as assert from "assert";
import { decodeWorkflowResult } from "../../controlStudio/workflowResult";

const units = { sp: "°C", pv: "°C", mv: "%" };
const primary = [{ id: "P1.SetpointC", unit: "°C" }, { id: "P1.TemperatureC", unit: "°C" }, { id: "P1.AppliedPowerPct", unit: "%" }];

function withCommissioning(channels: readonly { id: string; unit: string }[]): unknown {
  return { schemaVersion: 4, createdAt: "2026-09-27T12:00:00.000Z", resultId: "heater", resultRevision: 1, projectId: "project:heater",
    source: "Simulation", loop: { id: "heater-loop", revision: "sha256:loop", topology: "closed-loop" },
    controller: { instance: "P1", revision: "sha256:program" }, software: { extensionVersion: "0.24.65", numericalAdapters: {} }, units,
    recording: { rawSamples: [0, 1].map(i => ({ elapsedSeconds: i / 10, observedAtMs: 1000 + i * 100, actualCycleSeconds: 0.1, scan: 80 + i,
      source: "Simulation", loopRevision: "sha256:loop", sp: 35, pv: 35, requestedMv: 75, appliedMv: 75, units, quality: "good",
      freshnessMs: 0, controllerMode: "Manual", outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, safetyState: "Unavailable", events: [] })),
      selectedIntervals: [], acquisition: { method: "runtime_scan_buffer", completeness: "complete", programRevision: "sha256:program",
        observedSampleCount: 2, scanRange: { first: 80, last: 81 }, missingScanRanges: [], droppedObservedSamples: 0,
        actualCycleTimingAvailable: true, freshnessAvailable: true, reason: "Every runtime scan retained." } },
    commissioning: { status: "failed", reason: "The response left the declared band.", modelRevision: "sha256:model", target: 38,
      recoveryConfirmed: true, channels, transfers: [],
      criteria: { target: 38, durationSeconds: 60, settlingTimeSeconds: 30, maximumError: 0.2, taskPeriodMs: 100,
        processRange: { minimum: 0, maximum: 80 }, outputLimits: { minimum: 0, maximum: 100 } } },
    evidence: { observation: "Observed", model: "Not evaluated" }, conclusion: "Commissioning attempted." };
}

suite("Control Studio commissioning result storage", () => {
  test("keeps a commissioning result that recorded the loop's additional signals", () => {
    assert.ok(decodeWorkflowResult(withCommissioning(primary)), "Setpoint, measurement and command alone are accepted");
    assert.ok(decodeWorkflowResult(withCommissioning([...primary, { id: "P1.DisturbanceKw", unit: "kW" }, { id: "P1.IntegralPct", unit: "%" }])),
      "A recording of the loop's declared disturbance and diagnostics has more than three distinct channels");
    assert.strictEqual(decodeWorkflowResult(withCommissioning([...primary, primary[1]])), undefined, "A channel listed twice is rejected");
  });
});
