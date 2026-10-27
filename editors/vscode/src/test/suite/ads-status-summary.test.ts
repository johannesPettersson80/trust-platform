import * as assert from "assert";

import {
  summarizeAdsStatus,
  type AdsStatusReport,
} from "../../adsStatusSummary";

// The summary's rules (device and degraded counts, the texts) are the runtime's `ads.status` answer
// and are proven by the Rust test `status_view_counts_devices_and_degraded_devices`
// (crates/trust-runtime/src/control/ads_handlers/status.rs). These tests prove only that Live Values
// shows the runtime's summary as it answers it.
suite("ADS status summary", function () {
  test("Live Values shows the summary the runtime answers with its ADS status", () => {
    const summary = summarizeAdsStatus(statusReport());

    assert.deepStrictEqual(summary, {
      text: "ADS: 2 devices · 1 degraded",
      overall: "degraded",
      deviceCount: 2,
      degradedCount: 1,
    });
  });

  test("a missing report, or one without the runtime's summary, has no summary to show", () => {
    assert.strictEqual(
      summarizeAdsStatus(undefined).text,
      "ADS status unavailable"
    );
    const withoutView: AdsStatusReport = { ...statusReport(), view: undefined };
    assert.strictEqual(
      summarizeAdsStatus(withoutView).text,
      "ADS status unavailable",
      "the extension does not compute the summary itself"
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
    view: {
      text: "ADS: 2 devices · 1 degraded",
      overall: "degraded",
      device_count: 2,
      degraded_count: 1,
    },
  };
}
