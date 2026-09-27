import * as assert from "assert";
import { engineerRequirements } from "../../controlStudio/engineerVerificationPlan";

const base = {
  referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16,
};

suite("Control Studio channel requirement parsing", () => {
  test("retains bounded per-output channel requirements", () => {
    const channelRequirements = {
      "output-1": { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8 },
      "output-2": { referenceChange: 2, maximumError: 0.5, settlingBand: 0.1, settlingTimeSeconds: 6 },
    };
    const parsed = engineerRequirements({ ...base, channelRequirements } as any) as any;
    assert.deepStrictEqual(parsed.channelRequirements, channelRequirements);
  });

  test("rejects malformed or oversized channel requirement maps", () => {
    assert.throws(() => engineerRequirements({ ...base, channelRequirements: { bad: { referenceChange: 1, maximumError: 0.1, settlingBand: 0.2, settlingTimeSeconds: 2 } } } as any), /requirement|settling|error/i);
    const oversized = Object.fromEntries(Array.from({ length: 17 }, (_, index) => [`output-${index}`, { referenceChange: 1, maximumError: 1, settlingBand: 0.1, settlingTimeSeconds: 1 }]));
    assert.throws(() => engineerRequirements({ ...base, channelRequirements: oversized } as any), /channel|requirement|limit/i);
  });

  test("requires global validation duration to cover every channel settling time", () => {
    assert.throws(() => engineerRequirements({ ...base, durationSeconds: 6, settlingTimeSeconds: 2, channelRequirements: {
      "output-1": { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8 },
    } } as any), /duration|settling/i);
  });
});
