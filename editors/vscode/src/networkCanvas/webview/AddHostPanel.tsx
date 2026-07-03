import React, { useState } from "react";

// §0.4 host slot: add another machine to the fleet view. A host is client-side fleet membership —
// it writes the runtime's control endpoint to `trust-lsp.runtime.fleetEndpoints` (extension config);
// the canvas then fetches + merges that runtime's topology. (Network discovery is a later slice.)
export function AddHostPanel({
  post,
  onClose,
  onSaved,
}: {
  post: (message: unknown) => void;
  onClose: () => void;
  onSaved?: () => void;
}) {
  const [endpoint, setEndpoint] = useState("");
  const [authToken, setAuthToken] = useState("");
  const submit = () => {
    const value = endpoint.trim();
    if (!value) {
      return;
    }
    post({ type: "addHost", endpoint: value, authToken: authToken.trim() });
    onSaved?.();
    onClose();
  };

  return (
    <aside className="trust-inspector" style={PANEL} aria-label="Connect existing runtime">
      <div className="trust-inspector__header">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__eyebrow">Devices &amp; Connections / Runtime setup</div>
          <div className="trust-inspector__title">Connect existing runtime</div>
        </div>
        <button onClick={onClose} aria-label="Close" className="trust-button" style={CLOSE_BUTTON}>
          ✕
        </button>
      </div>
      <div className="trust-section trust-section--grow">
        <p className="trust-help" style={{ marginBottom: 12 }}>
          Add a truST runtime that is already running on another computer or controller.
          truST checks the address before it is shown as connected.
        </p>
        <div className="trust-field">
          <label htmlFor="runtime-endpoint">Runtime address</label>
          <input
            id="runtime-endpoint"
            value={endpoint}
            onChange={(e) => setEndpoint(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                submit();
              }
            }}
            placeholder="10.0.0.5:5680"
            className="trust-input"
            autoFocus
          />
          <div className="trust-field__message">
            Host name or IP plus port. Advanced: tcp://host:port or unix:///path/to/socket.
          </div>
        </div>
        <div className="trust-field">
          <label htmlFor="runtime-auth-token">Runtime auth token (optional)</label>
          <input
            id="runtime-auth-token"
            value={authToken}
            onChange={(e) => setAuthToken(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                submit();
              }
            }}
            placeholder="Optional"
            className="trust-input"
            type="password"
          />
          <div className="trust-field__message">
            Paste the token configured for that runtime. Leave this empty when the runtime does not require one. VS Code stores it securely and never shows it on the canvas.
          </div>
        </div>
        <p className="trust-help" style={{ marginTop: 12 }}>
          If you do not know the address, use Discover instead.
        </p>
      </div>
      <div className="trust-section" style={{ display: "flex", gap: 8 }}>
        <button
          onClick={submit}
          disabled={!endpoint.trim()}
          className="trust-button trust-button--primary"
          style={{ flex: 1 }}
        >
          Add runtime
        </button>
      </div>
    </aside>
  );
}

const PANEL: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 300,
  zIndex: 7,
};
const CLOSE_BUTTON: React.CSSProperties = {
  minHeight: 24,
  padding: 0,
  width: 26,
};
