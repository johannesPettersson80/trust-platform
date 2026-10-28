// The ADS client's summary as `trust-lsp.devicesTopology` answers it with the connection
// (crates/trust-devices/src/canvas/views.rs; specification 25 §5.13): its devices with the tags of
// each port, one sentence about its state with its tone, and its settings. The inspector draws it.

export interface AdsClientPortSummary {
  readonly port: number;
  readonly tagCount: number;
}

export interface AdsClientDeviceSummary {
  readonly address: string;
  readonly amsNetId: string;
  readonly ports: readonly AdsClientPortSummary[];
}

export interface AdsClientSummaryModel {
  readonly devices: readonly AdsClientDeviceSummary[];
  readonly status: string;
  readonly statusKind: "ok" | "neutral" | "error";
  readonly enabled: boolean;
  readonly configPath: string;
  readonly updateIntervalMs: number;
}
