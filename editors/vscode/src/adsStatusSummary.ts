// The `ads.status` answer of a runtime (specification 11, "ADS status summary"). Its `view` is the
// summary the tools show, which the runtime computes; this module only reads it.

export interface AdsConnectionStatus {
  name: string;
  state: string;
  point_count: number;
  degraded_points: number;
  summary: string;
}

/** The summary of the report that the runtime answers with it. */
export interface AdsStatusView {
  text: string;
  overall: string;
  device_count: number;
  degraded_count: number;
}

export interface AdsStatusReport {
  overall: string;
  summary: string;
  connections: AdsConnectionStatus[];
  view?: AdsStatusView;
}

export interface AdsStatusSummary {
  text: string;
  overall: string;
  deviceCount: number;
  degradedCount: number;
}

/** The runtime's summary of its ADS status; a report without one has no summary to show. */
export function summarizeAdsStatus(
  status: AdsStatusReport | undefined
): AdsStatusSummary {
  const view = status?.view;
  if (!view) {
    return {
      text: "ADS status unavailable",
      overall: "unknown",
      deviceCount: 0,
      degradedCount: 0,
    };
  }
  return {
    text: view.text,
    overall: view.overall,
    deviceCount: view.device_count,
    degradedCount: view.degraded_count,
  };
}
