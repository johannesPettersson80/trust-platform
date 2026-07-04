import React from "react";

interface SfcCodePanelProps {
  code: string | null;
  errors: string[];
  isGenerating?: boolean;
  onCopy?: () => void;
}

/**
 * Code Panel - Displays generated Structured Text code in real-time
 */
export const SfcCodePanel: React.FC<SfcCodePanelProps> = ({
  code,
  errors,
  isGenerating = false,
  onCopy,
}) => {
  const handleCopyCode = () => {
    if (code) {
      navigator.clipboard.writeText(code);
      onCopy?.();
    }
  };

  return (
    <div
      style={{
        position: "absolute",
        top: 0,
        right: 0,
        bottom: 0,
        width: "400px",
        display: "flex",
        flexDirection: "column",
        background: "var(--trust-overlay)",
        borderLeft: "1px solid var(--trust-border)",
        zIndex: 10,
      }}
    >
      {/* Header */}
      <div
        style={{
          padding: "12px 14px",
          borderBottom: "1px solid var(--trust-border)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          background: "var(--trust-overlay)",
        }}
      >
        <h3
          style={{
            margin: 0,
            fontSize: "13px",
            fontWeight: 600,
            color: "var(--trust-text)",
          }}
        >
          Generated ST Code
        </h3>
        {code && (
          <button
            onClick={handleCopyCode}
            style={{
              padding: "4px 12px",
              fontSize: "11px",
              border: "1px solid var(--trust-accent)",
              borderRadius: "var(--trust-radius)",
              background: "var(--trust-accent)",
              color: "var(--trust-on-accent)",
              cursor: "pointer",
            }}
            title="Copy code to clipboard"
          >
            Copy
          </button>
        )}
      </div>

      {/* Code Display */}
      <div
        style={{
          flex: 1,
          overflow: "auto",
          padding: code ? "12px" : "0",
        }}
      >
        {isGenerating ? (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              height: "100%",
              padding: "20px",
              textAlign: "center",
            }}
          >
            <p
              style={{
                margin: 0,
                fontSize: "13px",
                color: "var(--trust-text-muted)",
              }}
            >
              Generating Structured Text...
            </p>
          </div>
        ) : code ? (
          <pre
            style={{
              margin: 0,
              fontFamily: "var(--vscode-editor-font-family, monospace)",
              fontSize: "12px",
              lineHeight: "1.5",
              color: "var(--trust-text)",
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
            }}
          >
            <code>{code}</code>
          </pre>
        ) : (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              height: "100%",
              padding: "20px",
              textAlign: "center",
            }}
          >
            <div>
              <p
                style={{
                  margin: 0,
                  fontSize: "13px",
                  color: "var(--trust-text-muted)",
                }}
              >
                Structured Text code will appear here
              </p>
              <p
                style={{
                  margin: "8px 0 0 0",
                  fontSize: "11px",
                  color: "var(--trust-text-muted)",
                  opacity: 0.7,
                }}
              >
                Preview ST shows generated code here. Generate ST writes the companion file.
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Errors/Warnings */}
      {errors.length > 0 && (
        <div
          style={{
            borderTop: "1px solid var(--trust-border)",
            padding: "12px",
            background: "var(--vscode-inputValidation-warningBackground, #5a4d00)",
            maxHeight: "150px",
            overflow: "auto",
          }}
        >
          <h4
            style={{
              margin: "0 0 8px 0",
              fontSize: "12px",
              fontWeight: 600,
              color: "var(--vscode-inputValidation-warningForeground, var(--vscode-foreground, #cca700))",
            }}
          >
            Warnings ({errors.length})
          </h4>
          <ul
            style={{
              margin: 0,
              paddingLeft: "20px",
              fontSize: "11px",
              color: "var(--vscode-inputValidation-warningForeground, var(--vscode-foreground, #cca700))",
            }}
          >
            {errors.map((error, index) => (
              <li key={index} style={{ marginBottom: "4px" }}>
                {error}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
};
