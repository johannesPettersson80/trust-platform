import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { engineerPrediction } from "../../controlStudio/engineerPrediction";

const LESSON = "examples/control/02-derivative-action-and-dynamic-response/32-proportional-position-stiffness";

function lessonFile(name: string): any {
  const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
  return JSON.parse(fs.readFileSync(path.join(root, LESSON, name), "utf8"));
}

suite("Control Studio integrating plant prediction", () => {
  test("predicts position control of a supplied shaft model", () => {
    // J θ'' + B θ' = u with J = 1 kg·m² and B = 1 N·m·s/rad: θ/u = 1/(s² + s), an integrator with a 1 s lag.
    const task = { referenceChange: 1, maximumError: 0.02, settlingBand: 0.01, settlingTimeSeconds: 4, durationSeconds: 8 };
    const response = (proportionalGain: number) => {
      const prediction = engineerPrediction(lessonFile("shaft-model.json"), lessonFile("control-loop.json"), { proportionalGain }, task);
      assert.ok(prediction, `Tune must predict Kp ${proportionalGain} on the supplied shaft`);
      return prediction!.timeResponse;
    };
    const peak = (points: ReturnType<typeof response>) => Math.max(...points.map(point => point.candidatePv));
    // P control with the torque held for each 20 ms period: peaks 1.46 and 1.72 rad (continuous 1.44 and 1.67).
    const low = response(4), high = response(16);
    assert.ok(Math.abs(peak(low) - 1.46) < 0.01, `Kp 4 peak ${peak(low)}`);
    assert.ok(Math.abs(peak(high) - 1.72) < 0.01, `Kp 16 peak ${peak(high)}`);
    assert.strictEqual(low[1].candidateMv, 4, "the first command is Kp · 1 rad");
    assert.ok(Math.abs(low[low.length - 1].timeSeconds - 8) < 1e-9, "the prediction covers the declared 8 s");
  });
});
