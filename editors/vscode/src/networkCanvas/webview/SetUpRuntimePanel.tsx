import React from "react";

import {
  setUpRuntimeOptions,
  V1_SETUP_CAPS,
  type SetUpRuntimeOptionId,
} from "./setUpRuntime";

// §0.6.0 host empty-state primary "Set up runtime…": a small wizard. It shows ONLY the options the
// backend can do today (Connect / Run-local); not-yet-built options (Install / Docker) live under an
// explicit "coming soon" area, disabled-with-reason — never live dead buttons (§0.6.12).
export function SetUpRuntimePanel({
  onConnect,
  onRunLocal,
  onClose,
}: {
  onConnect: () => void;
  onRunLocal: () => void;
  onClose: () => void;
}) {
  const options = setUpRuntimeOptions(V1_SETUP_CAPS);
  const available = options.filter((option) => option.available);
  const comingSoon = options.filter((option) => !option.available);

  const choose = (id: SetUpRuntimeOptionId) => {
    if (id === "connect") {
      onConnect();
    } else if (id === "local") {
      onRunLocal();
    }
  };

  return (
    <aside className="trust-inspector" style={PANEL} aria-label="Set up a runtime">
      <div className="trust-inspector__header">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__eyebrow">Devices &amp; Connections / Runtime setup</div>
          <div className="trust-inspector__title">
            Set up runtime
          </div>
        </div>
        <button
          onClick={onClose}
          aria-label="Close"
          className="trust-button"
          style={CLOSE_BUTTON}
        >
          ✕
        </button>
      </div>
      <div className="trust-section trust-section--grow">
        <div className="trust-button-grid trust-button-grid--single">
          {available.map((option) => (
            <button
              key={option.id}
              onClick={() => choose(option.id)}
              className="trust-button trust-button--active"
              style={OPTION_BUTTON}
            >
              <span style={OPTION_LABEL}>{option.label}</span>
              <span className="trust-help" style={OPTION_DETAIL}>
                {option.detail}
              </span>
            </button>
          ))}
        </div>

        {comingSoon.length > 0 && (
          <>
            <div className="trust-divider">
              <div className="trust-section__title">More ways to run (coming soon)</div>
            </div>
            {comingSoon.map((option) => (
              <button
                key={option.id}
                disabled
                title={option.reason}
                className="trust-button"
                style={OPTION_BUTTON}
              >
                <span style={OPTION_LABEL}>{option.label}</span>
                <span className="trust-help" style={OPTION_DETAIL}>
                  {option.reason}
                </span>
              </button>
            ))}
          </>
        )}
      </div>
    </aside>
  );
}

const PANEL: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 252,
  zIndex: 7,
};
const OPTION_BUTTON: React.CSSProperties = {
  alignItems: "stretch",
  flexDirection: "column",
  gap: 3,
  justifyContent: "flex-start",
  marginBottom: 8,
  minHeight: 54,
  textAlign: "left",
  whiteSpace: "normal",
  width: "100%",
};
const OPTION_LABEL: React.CSSProperties = { fontSize: 12, fontWeight: 650 };
const OPTION_DETAIL: React.CSSProperties = {
  fontSize: 11,
  lineHeight: 1.4,
  margin: 0,
  textAlign: "left",
};
const CLOSE_BUTTON: React.CSSProperties = {
  minHeight: 24,
  padding: 0,
  width: 26,
};
