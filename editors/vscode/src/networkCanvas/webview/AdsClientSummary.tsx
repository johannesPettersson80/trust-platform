import React from "react";

import type {
  AdsClientDeviceSummary,
  AdsClientSummaryModel,
} from "./adsClientSummaryModel";
import { t } from "./theme";

export function AdsClientSummary({
  model,
  onEdit,
}: {
  model: AdsClientSummaryModel;
  onEdit?: () => void;
}) {
  const messageClass = model.statusKind === "ok"
    ? "trust-message trust-message--ok"
    : model.statusKind === "error"
      ? "trust-message trust-message--error"
      : "trust-message";
  return (
    <>
      <div className={messageClass} style={{ marginBottom: 14 }}>
        {model.status}
      </div>

      {model.devices.length === 0 ? (
        <p className="trust-empty" style={{ padding: 0, textAlign: "left" }}>
          No ADS device is configured.
        </p>
      ) : (
        model.devices.map((device, index) => (
          <DeviceCard
            key={`${device.address}:${device.amsNetId}`}
            device={device}
            showHeading={model.devices.length > 1}
            index={index}
          />
        ))
      )}

      <details data-role="ads-advanced-settings" style={ADVANCED}>
        <summary style={ADVANCED_SUMMARY}>Advanced settings</summary>
        <div style={{ paddingTop: 10 }}>
          <SummaryRow label="Enabled" value={model.enabled ? "On" : "Off"} />
          <SummaryRow label="Config file" value={model.configPath} />
          <SummaryRow
            label="Update interval"
            value={`${model.updateIntervalMs} ms`}
          />
          {onEdit && (
            <button
              type="button"
              onClick={onEdit}
              className="trust-button"
              style={{ width: "100%", marginTop: 5 }}
            >
              Edit advanced settings
            </button>
          )}
        </div>
      </details>
    </>
  );
}

function DeviceCard({
  device,
  showHeading,
  index,
}: {
  device: AdsClientDeviceSummary;
  showHeading: boolean;
  index: number;
}) {
  return (
    <section style={DEVICE_CARD}>
      {showHeading && (
        <div style={DEVICE_HEADING}>ADS device {index + 1}</div>
      )}
      <SummaryRow label="Address" value={device.address} />
      <SummaryRow label="AMS Net ID" value={device.amsNetId} />
      <div style={{ marginTop: 11 }}>
        <div style={PORT_LABEL}>ADS ports</div>
        {device.ports.length === 0 ? (
          <span style={{ color: t.textMuted, fontSize: 12 }}>No ports configured</span>
        ) : (
          <div style={PORTS}>
            {device.ports.map((port) => (
              <span key={port.port} data-ads-port={port.port} style={PORT_CHIP}>
                <strong style={{ fontSize: 12 }}>{port.port}</strong>
                <span style={{ color: t.textMuted, fontSize: 10.5 }}>
                  {port.tagCount} {port.tagCount === 1 ? "tag" : "tags"}
                </span>
              </span>
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function SummaryRow({ label, value }: { label: string; value: string }) {
  return (
    <div style={SUMMARY_ROW}>
      <span style={{ color: t.textMuted }}>{label}</span>
      <span style={{ color: t.text, overflowWrap: "anywhere" }}>{value}</span>
    </div>
  );
}

const DEVICE_CARD: React.CSSProperties = {
  padding: "12px 13px",
  marginBottom: 10,
  border: `1px solid ${t.border}`,
  borderRadius: 8,
  background: t.surface,
};
const DEVICE_HEADING: React.CSSProperties = {
  marginBottom: 9,
  color: t.text,
  fontSize: 12,
  fontWeight: 600,
};
const SUMMARY_ROW: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "92px 1fr",
  gap: 10,
  marginBottom: 7,
  fontSize: 12,
  lineHeight: 1.45,
};
const PORT_LABEL: React.CSSProperties = {
  marginBottom: 6,
  color: t.textMuted,
  fontSize: 11,
};
const PORTS: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: 6,
};
const PORT_CHIP: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "baseline",
  gap: 6,
  padding: "4px 8px",
  border: `1px solid ${t.border}`,
  borderRadius: 999,
  background: t.surfaceRaised,
  color: t.text,
};
const ADVANCED: React.CSSProperties = {
  marginTop: 12,
  padding: "9px 11px",
  border: `1px solid ${t.border}`,
  borderRadius: 8,
  background: t.surface,
};
const ADVANCED_SUMMARY: React.CSSProperties = {
  color: t.textMuted,
  cursor: "pointer",
  fontSize: 11.5,
  fontWeight: 600,
};
