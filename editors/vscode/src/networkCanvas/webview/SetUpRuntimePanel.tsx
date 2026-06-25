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
    <aside style={PANEL} aria-label="Set up a runtime">
      <div style={HEADER}>
        <div style={{ flex: 1, fontSize: 12, fontWeight: 700, color: "var(--vscode-foreground, #cfd6e0)" }}>
          Set up runtime
        </div>
        <button onClick={onClose} aria-label="Close" style={ICON}>
          ✕
        </button>
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: 12 }}>
        {available.map((option) => (
          <button
            key={option.id}
            onClick={() => choose(option.id)}
            style={OPTION}
          >
            <span style={OPTION_LABEL}>{option.label}</span>
            <span style={OPTION_DETAIL}>{option.detail}</span>
          </button>
        ))}

        {comingSoon.length > 0 && (
          <>
            <div style={SECTION}>More ways to run (coming soon)</div>
            {comingSoon.map((option) => (
              <button
                key={option.id}
                disabled
                title={option.reason}
                style={{ ...OPTION, opacity: 0.5, cursor: "default" }}
              >
                <span style={OPTION_LABEL}>{option.label}</span>
                <span style={OPTION_DETAIL}>{option.reason}</span>
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
  left: 0,
  bottom: 0,
  width: 252,
  background: "var(--vscode-editorHoverWidget-background, rgba(16,19,26,.97))",
  borderRight: "1px solid var(--vscode-editorWidget-border, #2a2f3a)",
  zIndex: 7,
  display: "flex",
  flexDirection: "column",
};
const HEADER: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  padding: "11px 12px",
  borderBottom: "1px solid var(--vscode-editorWidget-border, #2a2f3a)",
};
const OPTION: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 3,
  width: "100%",
  textAlign: "left",
  background: "var(--vscode-input-background, #10141b)",
  border: "1px solid var(--vscode-input-border, #343b47)",
  borderRadius: 7,
  color: "var(--vscode-foreground, #eef1f5)",
  padding: "9px 11px",
  marginBottom: 8,
  cursor: "pointer",
};
const OPTION_LABEL: React.CSSProperties = { fontSize: 12, fontWeight: 650 };
const OPTION_DETAIL: React.CSSProperties = { fontSize: 11, color: "var(--vscode-descriptionForeground, #9aa6b6)", lineHeight: 1.4 };
const SECTION: React.CSSProperties = {
  fontSize: 10,
  textTransform: "uppercase",
  letterSpacing: "0.04em",
  color: "var(--vscode-descriptionForeground, #7a8595)",
  margin: "8px 0 8px",
};
const ICON: React.CSSProperties = {
  border: "none",
  background: "transparent",
  color: "var(--vscode-descriptionForeground, #949cab)",
  fontSize: 14,
  cursor: "pointer",
  padding: 0,
};
