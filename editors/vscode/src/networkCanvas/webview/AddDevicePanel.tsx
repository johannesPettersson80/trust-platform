import React, { useEffect, useMemo, useState } from "react";
import type {
  CommApplyResponse,
  CommFieldSchema,
  CommProtocolSchema,
  CommSchemaResponse,
} from "../../communication/schemaForm";

interface Props {
  schema?: CommSchemaResponse;
  applyResult?: CommApplyResponse;
  reachable: boolean;
  setupMessage?: string;
  target?: { id: string; name: string };
  preselectProtocol?: string;
  preselectParams?: Record<string, unknown>; // prefill from a discovered candidate (§0.5 Browse→Add)
  post: (message: unknown) => void;
  onClose: () => void;
}

const PANEL_STYLE: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 360,
  maxWidth: "92vw",
  background: "var(--vscode-editorHoverWidget-background, rgba(18,21,28,.98))",
  borderLeft: "1px solid var(--vscode-editorWidget-border, #2a2f3a)",
  boxShadow: "-18px 0 50px rgba(0,0,0,.45)",
  zIndex: 8,
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
};

function defaultsFor(protocol: CommProtocolSchema): Record<string, string> {
  const values: Record<string, string> = {};
  for (const field of protocol.fields) {
    if (field.default === undefined || field.default === null) {
      values[field.id] = "";
    } else if (typeof field.default === "object") {
      values[field.id] = JSON.stringify(field.default, null, 0);
    } else {
      values[field.id] = String(field.default);
    }
  }
  return values;
}

// Overlay discovered/prefill params over the schema defaults (Browse → Add, §0.5).
function valuesWithPrefill(
  protocol: CommProtocolSchema,
  prefill?: Record<string, unknown>
): Record<string, string> {
  const values = defaultsFor(protocol);
  if (prefill) {
    for (const field of protocol.fields) {
      if (field.id in prefill) {
        const v = prefill[field.id];
        values[field.id] =
          v === null || v === undefined ? "" : typeof v === "object" ? JSON.stringify(v) : String(v);
      }
    }
  }
  return values;
}

function coerce(field: CommFieldSchema, raw: string): unknown {
  const t = field.type;
  if (t === "number") {
    const n = Number(raw);
    return Number.isFinite(n) ? n : raw;
  }
  if (t === "bool" || t === "boolean") {
    return raw === "true";
  }
  if (t === "json_object") {
    try {
      return JSON.parse(raw);
    } catch {
      return raw;
    }
  }
  return raw;
}

export function AddDevicePanel({ schema, applyResult, reachable, setupMessage, target, preselectProtocol, preselectParams, post, onClose }: Props) {
  const protocols = useMemo(() => schema?.protocols ?? [], [schema]);
  const [protocolId, setProtocolId] = useState<string>(preselectProtocol ?? "");
  const [values, setValues] = useState<Record<string, string>>({});

  const protocol = protocols.find((p) => p.id === protocolId);

  // Pick the dropped protocol (or the first available) when a schema arrives.
  useEffect(() => {
    if (protocols.length > 0 && !protocols.some((p) => p.id === protocolId)) {
      const wanted = protocols.find((p) => p.id === preselectProtocol);
      setProtocolId((wanted ?? protocols[0]).id);
    }
  }, [protocols, protocolId, preselectProtocol]);

  // Reset field values when the selected protocol changes (prefilled from a discovered candidate).
  useEffect(() => {
    if (protocol) {
      setValues(valuesWithPrefill(protocol, preselectParams));
    }
  }, [protocol, preselectParams]);

  const fieldErrors = new Map(
    (applyResult?.field_errors ?? []).map((e) => [e.field, e.message])
  );

  const submit = (type: string) => {
    if (!protocol) {
      return;
    }
    const params: Record<string, unknown> = {};
    for (const field of protocol.fields) {
      params[field.id] = coerce(field, values[field.id] ?? "");
    }
    const action =
      protocol.supports_multi_instance && protocol.actions.includes("add") ? "add" : "upsert";
    post({ type, protocol: protocol.id, params, action, runtimeId: target?.id, target: target?.id });
  };

  const ok = applyResult && (applyResult.applied || applyResult.lifecycle_effect === "test_ok");
  const blocked = applyResult && applyResult.lifecycle_effect === "blocked";

  return (
    <aside style={PANEL_STYLE} aria-label="Add device">
      <header style={{ display: "flex", alignItems: "center", gap: 8, padding: "12px 14px", borderBottom: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <strong style={{ display: "block", fontSize: 14 }}>{protocol ? `Add ${protocol.title}` : "Add device"}</strong>
          {target?.name && <span style={{ fontSize: 10.5, color: "var(--vscode-descriptionForeground, #7f8794)" }}>on {target.name}</span>}
        </div>
        <button onClick={onClose} aria-label="Close" style={iconBtn}>✕</button>
      </header>

      <div style={{ flex: 1, overflow: "auto", padding: 14 }}>
        {protocols.length === 0 ? (
          <p style={{ color: "var(--vscode-descriptionForeground, #949cab)", fontSize: 12 }}>
            {setupMessage ?? "Device catalog unavailable (needs a newer trust-runtime)."}
          </p>
        ) : (
          <>
            <label style={labelStyle}>Protocol</label>
            <select
              value={protocolId}
              onChange={(e) => setProtocolId(e.target.value)}
              style={inputStyle}
            >
              {protocols.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.title}
                </option>
              ))}
            </select>
            {protocol?.purpose && (
              <p style={{ color: "var(--vscode-descriptionForeground, #7f8794)", fontSize: 11, margin: "6px 0 14px" }}>{protocol.purpose}</p>
            )}

            {protocol?.fields.map((field) => (
              <Field
                key={field.id}
                field={field}
                value={values[field.id] ?? ""}
                error={fieldErrors.get(field.id)}
                onChange={(v) => setValues((prev) => ({ ...prev, [field.id]: v }))}
              />
            ))}

            {applyResult && (
              <div
                style={{
                  marginTop: 12,
                  padding: "9px 11px",
                  borderRadius: 8,
                  fontSize: 12,
                  border: `1px solid ${ok ? "var(--vscode-charts-green, #46c265)77" : blocked ? "var(--vscode-errorForeground, #f0584f)77" : "var(--vscode-input-border, #343b47)"}`,
                  background: ok ? "rgba(70,194,101,.12)" : blocked ? "rgba(240,88,79,.1)" : "var(--vscode-editorWidget-background, rgba(20,24,32,.7))",
                  color: ok ? "var(--vscode-charts-green, #bff0cc)" : blocked ? "var(--vscode-errorForeground, #ffcfcb)" : "var(--vscode-foreground, #cfd6e0)",
                }}
              >
                {applyResult.message || (ok ? "Applied." : "")}
                {applyResult.lifecycle_effect && applyResult.lifecycle_effect !== "blocked" && (
                  <div style={{ color: "var(--vscode-descriptionForeground, #949cab)", marginTop: 3 }}>{applyResult.lifecycle_effect}</div>
                )}
              </div>
            )}
          </>
        )}
      </div>

      {protocols.length > 0 && (
        <footer style={{ display: "flex", flexWrap: "wrap", gap: 8, padding: 12, borderTop: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
          <button onClick={() => submit("commSave")} style={{ ...primaryBtn, flex: 1 }}>Save</button>
          {protocol?.supports_test && reachable && (
            <button onClick={() => submit("commTest")} style={secondaryBtn}>Test</button>
          )}
          {reachable && (
            <button onClick={() => submit("commApplyLive")} style={{ ...secondaryBtn, flexBasis: "100%" }}>
              Apply to running runtime
            </button>
          )}
        </footer>
      )}
    </aside>
  );
}

function Field({
  field,
  value,
  error,
  onChange,
}: {
  field: CommFieldSchema;
  value: string;
  error?: string;
  onChange: (v: string) => void;
}) {
  const border = error ? "1px solid var(--vscode-errorForeground, #f0584f)88" : "1px solid var(--vscode-input-border, #343b47)";
  const common = { ...inputStyle, border };
  return (
    <div style={{ marginBottom: 12 }}>
      <label style={labelStyle}>
        {field.label}
        {field.required && <span style={{ color: "var(--vscode-errorForeground, #f0584f)" }}> *</span>}
      </label>
      {field.options && field.options.length > 0 ? (
        <select value={value} onChange={(e) => onChange(e.target.value)} style={common}>
          {field.options.map((o) => (
            <option key={o} value={o}>{o}</option>
          ))}
        </select>
      ) : field.type === "json_object" ? (
        <textarea value={value} onChange={(e) => onChange(e.target.value)} rows={3} style={{ ...common, resize: "vertical", fontFamily: "monospace" }} />
      ) : field.type === "bool" || field.type === "boolean" ? (
        <select value={value} onChange={(e) => onChange(e.target.value)} style={common}>
          <option value="false">false</option>
          <option value="true">true</option>
        </select>
      ) : (
        <input
          type={field.secret ? "password" : field.type === "number" ? "number" : "text"}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          style={common}
        />
      )}
      {error ? (
        <div style={{ color: "var(--vscode-errorForeground, #ffcfcb)", fontSize: 10.5, marginTop: 3 }}>{error}</div>
      ) : field.help ? (
        <div style={{ color: "var(--vscode-descriptionForeground, #7f8794)", fontSize: 10.5, marginTop: 3 }}>{field.help}</div>
      ) : null}
    </div>
  );
}

const labelStyle: React.CSSProperties = { display: "block", fontSize: 11, color: "var(--vscode-foreground, #cfd6e0)", marginBottom: 4, fontWeight: 600 };
const inputStyle: React.CSSProperties = {
  width: "100%",
  background: "var(--vscode-input-background, #10141b)",
  border: "1px solid var(--vscode-input-border, #343b47)",
  borderRadius: 7,
  color: "var(--vscode-foreground, #eef1f5)",
  padding: "7px 9px",
  fontSize: 12,
};
const primaryBtn: React.CSSProperties = {
  border: "1px solid var(--vscode-focusBorder, #2f81f7)",
  background: "var(--vscode-focusBorder, #2f81f7)",
  color: "var(--vscode-button-foreground, #fff)",
  borderRadius: 7,
  padding: "8px 13px",
  fontSize: 12,
  fontWeight: 650,
  cursor: "pointer",
};
const secondaryBtn: React.CSSProperties = {
  border: "1px solid var(--vscode-input-border, #343b47)",
  background: "transparent",
  color: "var(--vscode-foreground, #cfd6e0)",
  borderRadius: 7,
  padding: "8px 13px",
  fontSize: 12,
  cursor: "pointer",
};
const iconBtn: React.CSSProperties = {
  border: "none",
  background: "transparent",
  color: "var(--vscode-descriptionForeground, #949cab)",
  fontSize: 14,
  cursor: "pointer",
};
