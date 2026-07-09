import React, { useEffect, useState } from "react";

import {
  adsPortDraftIsStale,
  adsTargetNetId,
  adsTargetPort,
  parseAdsPortInput,
  withAdsTargetPort,
} from "./adsTargetPort";

export function AdsBrowseTargetControls({
  target,
  loading,
  onBrowse,
  onDraftStaleChange,
}: {
  target: Record<string, unknown>;
  loading: boolean;
  onBrowse: (target: Record<string, unknown>) => void;
  onDraftStaleChange: (stale: boolean) => void;
}) {
  const targetPort = adsTargetPort(target);
  const [portDraft, setPortDraft] = useState(String(targetPort));
  const parsedPort = parseAdsPortInput(portDraft);
  const netId = adsTargetNetId(target);

  useEffect(() => {
    setPortDraft(String(targetPort));
    onDraftStaleChange(false);
  }, [onDraftStaleChange, targetPort]);

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
            <option value="301">I/O</option>
            <option value="501">Motion</option>
            <option value="851">PLC runtime 1</option>
            <option value="852">PLC runtime 2</option>
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
          {loading ? "Browsing…" : "Browse symbols"}
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
        Each ADS port is a separate server and symbol namespace. The server must support Symbol
        Upload.
      </p>
    </div>
  );
}

const SECTION: React.CSSProperties = {
  padding: "10px 14px",
  borderBottom: "1px solid var(--vscode-editorWidget-border, #2a2f3a)",
};
const SERVER_IDENTITY: React.CSSProperties = {
  display: "flex",
  alignItems: "baseline",
  gap: 8,
  minWidth: 0,
};
const LABEL: React.CSSProperties = {
  color: "var(--vscode-descriptionForeground, #7f8794)",
  fontSize: 10.5,
  fontWeight: 600,
};
const NET_ID: React.CSSProperties = {
  color: "var(--vscode-foreground, #eef1f5)",
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
