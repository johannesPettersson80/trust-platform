import React, { useEffect, useState } from "react";

import {
  adsPortDraftIsStale,
  adsTargetNetId,
  adsTargetPort,
  confirmedAdsBrowseRetryTarget,
  parseAdsPortInput,
  withAdsTargetPort,
} from "./adsTargetPort";
import { adsServicePresentation } from "./discoverPaneModel";

export function AdsBrowseTargetControls({
  target,
  loading,
  browseFailed,
  onBrowse,
  onDraftStaleChange,
}: {
  target: Record<string, unknown>;
  loading: boolean;
  browseFailed: boolean;
  onBrowse: (target: Record<string, unknown>) => void;
  onDraftStaleChange: (stale: boolean) => void;
}) {
  const targetPort = adsTargetPort(target);
  const [portDraft, setPortDraft] = useState(String(targetPort));
  const parsedPort = parseAdsPortInput(portDraft);
  const netId = adsTargetNetId(target);
  const confirmedByDiscovery = target.ads_port_confirmed === true;
  const retryTarget = confirmedAdsBrowseRetryTarget(
    target,
    loading,
    browseFailed
  );

  useEffect(() => {
    setPortDraft(String(targetPort));
    onDraftStaleChange(false);
  }, [onDraftStaleChange, targetPort]);

  if (confirmedByDiscovery) {
    const service = adsServicePresentation(targetPort);
    return (
      <div
        data-role="ads-confirmed-service"
        className="trust-section"
        style={SECTION}
      >
        <div>
          <span style={LABEL}>Selected ADS service</span>
          <strong style={{ display: "block", marginTop: 2, color: "var(--trust-text)" }}>
            {service.primary} ({service.secondary})
          </strong>
        </div>
        <p className="trust-help" style={{ marginTop: 6 }}>
          {loading
            ? "Loading the ADS service selected in Discover…"
            : "Using the ADS service selected in Discover."}
        </p>
        {retryTarget && (
          <button
            type="button"
            data-role="ads-retry-confirmed-browse"
            className="trust-button"
            onClick={() => onBrowse(retryTarget)}
          >
            Retry browse
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="trust-section" style={SECTION}>
      <div style={SERVER_IDENTITY}>
        <span style={LABEL}>AMS Net ID</span>
        <code style={NET_ID}>{netId}</code>
      </div>
      <div style={PORT_ROW}>
        <label style={{ flex: 1, minWidth: 0 }}>
          <span style={LABEL}>ADS port</span>
          <input
            data-role="ads-browse-port"
            type="number"
            min={1}
            max={65535}
            step={1}
            list="trust-common-ads-ports"
            value={portDraft}
            aria-invalid={Boolean(parsedPort.error)}
            aria-describedby={parsedPort.error ? "ads-browse-port-error" : undefined}
            onChange={(event) => {
              const draft = event.target.value;
              setPortDraft(draft);
              onDraftStaleChange(adsPortDraftIsStale(draft, target));
            }}
            className="trust-input"
            style={{ marginTop: 4 }}
          />
          <datalist id="trust-common-ads-ports">
            <option value="301">Common ADS service (301)</option>
            <option value="501">Common ADS service (501)</option>
            <option value="851">PLC runtime 1</option>
            <option value="852">PLC runtime 2</option>
            <option value="853">PLC runtime 3</option>
            <option value="854">PLC runtime 4</option>
          </datalist>
        </label>
        <button
          data-role="browse-ads-symbols"
          disabled={loading || Boolean(parsedPort.error)}
          title={parsedPort.error}
          className="trust-button trust-button--primary"
          style={{ alignSelf: "flex-end" }}
          onClick={() => {
            if (parsedPort.port) {
              onBrowse(withAdsTargetPort(target, parsedPort.port));
            }
          }}
        >
          {loading ? "Browsing…" : "Browse variables"}
        </button>
      </div>
      {parsedPort.error && (
        <div
          id="ads-browse-port-error"
          className="trust-field__message trust-field__message--error"
          style={{ marginTop: 5 }}
        >
          {parsedPort.error}
        </div>
      )}
      <p className="trust-help" style={{ marginTop: 6 }}>
        Each ADS service port exposes a separate variable namespace. The service must support the
        ADS Symbol Upload capability.
      </p>
    </div>
  );
}

const SECTION: React.CSSProperties = {
  padding: "10px 14px",
  borderBottom: "1px solid var(--trust-border)",
};
const SERVER_IDENTITY: React.CSSProperties = {
  display: "flex",
  alignItems: "baseline",
  gap: 8,
  minWidth: 0,
};
const LABEL: React.CSSProperties = {
  color: "var(--trust-text-muted)",
  fontSize: 10.5,
  fontWeight: 600,
};
const NET_ID: React.CSSProperties = {
  color: "var(--trust-text)",
  fontSize: 11,
  overflow: "hidden",
  textOverflow: "ellipsis",
};
const PORT_ROW: React.CSSProperties = {
  display: "flex",
  alignItems: "end",
  gap: 8,
  marginTop: 8,
};
