import React from "react";

interface CodePanelProps {
  code: string | null;
  errors: string[];
}

/**
 * Code Panel - Displays generated Structured Text code
 */
export const CodePanel: React.FC<CodePanelProps> = ({ code, errors }) => {
  const handleCopyCode = () => {
    if (code) {
      navigator.clipboard.writeText(code);
    }
  };

  return (
    <div className="code-panel">
      <div className="code-panel-header">
        <h3>Generated Structured Text (ST)</h3>
        {code && (
          <button
            className="copy-button trust-button trust-button--primary"
            onClick={handleCopyCode}
          >
            Copy
          </button>
        )}
      </div>

      <div className="code-panel-content">
        {code ? (
          <pre className="code-display">
            <code>{code}</code>
          </pre>
        ) : (
          <div className="code-placeholder">
            <p>Click "Generate Code" to see the ST output</p>
          </div>
        )}
      </div>

      {errors.length > 0 && (
        <div className="code-panel-errors">
          <h4>Warnings:</h4>
          <ul>
            {errors.map((error, index) => (
              <li key={index} className="error-item">
                {error}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
};
