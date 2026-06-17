import * as assert from "assert";

import {
  summarizeAdsStatus,
  type AdsStatusReport,
} from "../../adsStatusSummary";

suite("ADS status summary", function () {
  test("summarizes device and degraded counts for runtime pane and ADS panel", () => {
    const summary = summarizeAdsStatus(statusReport());

    assert.strictEqual(summary.text, "ADS: 2 devices · 1 degraded");
    assert.strictEqual(summary.deviceCount, 2);
    assert.strictEqual(summary.degradedCount, 1);
    assert.strictEqual(summary.overall, "degraded");
  });

  test("handles missing and empty status reports", () => {
    assert.strictEqual(
      summarizeAdsStatus(undefined).text,
      "ADS status unavailable"
    );
    assert.strictEqual(
      summarizeAdsStatus({
        overall: "disabled",
        summary: "ADS is not configured.",
        connections: [],
      }).text,
      "ADS is not configured."
    );
  });
});

function statusReport(): AdsStatusReport {
  return {
    overall: "degraded",
    summary: "2 ADS devices, 1 degraded.",
    connections: [
      {
        name: "line1",
        state: "connected",
        point_count: 4,
        degraded_points: 0,
        summary: "Connected.",
      },
      {
        name: "line2",
        state: "reconnecting",
        point_count: 3,
        degraded_points: 1,
        summary: "Reconnecting.",
      },
    ],
  };
}
