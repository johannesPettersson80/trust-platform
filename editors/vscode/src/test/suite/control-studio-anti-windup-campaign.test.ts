import * as assert from "assert";

suite("Control Studio anti-windup campaign", () => {
  test("reviews an unapplied back-calculation candidate as one complete typed record", async () => {
    const api = campaignApi();
    assert.strictEqual(typeof api.AntiWindupCampaign, "function",
      "the saturation finding needs a campaign owner before any candidate can be staged");
    const reviewed: any[] = [];
    const client = {
      status: async () => ({ values: { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0 },
        program_revision: "program", binding_revision: "binding", active_revision: "active", status: "idle",
        previous_revision: null, previous_values: null, reason: null }),
      review: async (values: Record<string, number>) => { reviewed.push(values); return { observed: await client.status(), values }; },
    };
    const campaign = new api.AntiWindupCampaign(loop(), client);
    const result = await campaign.review(4);
    assert.deepStrictEqual(result.values, { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0.25 });
    assert.strictEqual(result.trackingTimeSeconds, 4);
    assert.strictEqual(result.backCalculationGain, 0.25);
    assert.deepStrictEqual(reviewed, [result.values], "candidate review must retain the full typed parameter record");
  });

  test("rejects a tracked comparison if the recorded baseline already has tracking enabled", async () => {
    const api = campaignApi();
    const client = {
      status: async () => ({ values: { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0.1 },
        program_revision: "program", binding_revision: "binding", active_revision: "active", status: "idle",
        previous_revision: null, previous_values: null, reason: null }),
      review: async () => { throw new Error("must not review"); },
    };
    await assert.rejects(() => new api.AntiWindupCampaign(loop(), client).review(4), /untracked baseline/i);
  });

  test("stages the reviewed candidate only in simulation and rolls the exact baseline record back", async () => {
    const api = campaignApi();
    let phase = "baseline";
    const baseline = { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0 };
    const candidate = { ...baseline, "Main.BackCalculationGain": 0.25 };
    const status = () => ({ values: phase === "baseline" ? baseline : candidate,
      program_revision: "program", binding_revision: "binding", active_revision: phase === "baseline" ? "base" : "tracked",
      status: "applied", previous_revision: phase === "baseline" ? null : "base",
      previous_values: phase === "baseline" ? null : baseline, reason: null });
    const calls: string[] = [];
    const client = {
      status: async () => status(),
      review: async (values: Record<string, number>) => { calls.push(`review:${values["Main.BackCalculationGain"]}`); return { observed: status(), values }; },
      apply: async (review: any) => { calls.push(`apply:${review.values["Main.BackCalculationGain"]}`); phase = "tracked"; return status(); },
      reviewRollback: async () => { calls.push("review-rollback"); return { observed: status(), values: baseline }; },
      rollback: async (review: any) => { calls.push(`rollback:${review.values["Main.BackCalculationGain"]}`); phase = "baseline"; return status(); },
    };
    const campaign = new api.AntiWindupCampaign(loop(), client);
    await campaign.review(4);
    assert.deepStrictEqual((await campaign.stage()).values, candidate);
    assert.deepStrictEqual((await campaign.rollback()).values, baseline);
    assert.deepStrictEqual(calls, ["review:0.25", "apply:0.25", "review-rollback", "rollback:0"]);
  });
});

function campaignApi(): any {
  try { return require("../../controlStudio/antiWindupCampaign"); } catch { return {}; }
}

function loop(): any {
  return { id: "loop:anti-windup", parameterBinding: { loop_id: "loop:anti-windup", task_name: "MainTask", mode: { symbol: "Main.Manual", manual_value: true }, parameters: [
    { symbol: "Main.Kp", iec_type: "LREAL", unit: "V/m", convention: "proportional gain", minimum: 0, maximum: 100 },
    { symbol: "Main.Ki", iec_type: "LREAL", unit: "V/(m*s)", convention: "integral gain", minimum: 0, maximum: 100 },
    { symbol: "Main.Kd", iec_type: "LREAL", unit: "V*s/m", convention: "derivative gain", minimum: 0, maximum: 100 },
    { symbol: "Main.BackCalculationGain", iec_type: "LREAL", unit: "1/s", convention: "back-calculation gain", minimum: 0, maximum: 100 },
  ] }, controller: { law: { kind: "pid", parameterRecordSymbol: "Main.Parameters", parameterRevisionSymbol: "Main.ParameterRevision",
    proportionalGainSymbol: "Main.Kp", integralGainSymbol: "Main.Ki", derivativeGainSymbol: "Main.Kd",
    antiWindup: { method: "back-calculation", trackingGainSymbol: "Main.BackCalculationGain" } } } };
}
