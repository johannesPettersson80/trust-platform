import React from "react";
import type { CommFieldSchema, CommProtocolSchema } from "../../communication/schemaForm";

// Shared schema-driven form bits used by the editable inspector (and the add flow):
// turn a protocol schema + a params object into editable string values and back.

export function defaultsFor(protocol: CommProtocolSchema): Record<string, string> {
  const values: Record<string, string> = {};
  for (const field of protocol.fields) {
    values[field.id] = stringifyDefault(field.default);
  }
  return values;
}

// Pre-fill from a node's CURRENT config params (edit), falling back to schema defaults (add).
export function valuesFor(
  protocol: CommProtocolSchema,
  params?: Record<string, unknown>
): Record<string, string> {
  const values = defaultsFor(protocol);
  if (params) {
    for (const field of protocol.fields) {
      if (field.id in params) {
        values[field.id] = stringifyDefault(params[field.id]);
      }
    }
  }
  return values;
}

export function buildParams(
  protocol: CommProtocolSchema,
  values: Record<string, string>
): Record<string, unknown> {
  const params: Record<string, unknown> = {};
  for (const field of protocol.fields) {
    params[field.id] = coerce(field, values[field.id] ?? "");
  }
  return params;
}

function stringifyDefault(value: unknown): string {
  if (value === undefined || value === null) {
    return "";
  }
  if (typeof value === "object") {
    return JSON.stringify(value, null, 0);
  }
  return String(value);
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

export function Field({
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
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </select>
      ) : field.type === "json_object" ? (
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          rows={3}
          style={{ ...common, resize: "vertical", fontFamily: "monospace" }}
        />
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

export const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: 11,
  color: "var(--vscode-foreground, #cfd6e0)",
  marginBottom: 4,
  fontWeight: 600,
};
export const inputStyle: React.CSSProperties = {
  width: "100%",
  background: "var(--vscode-input-background, #10141b)",
  border: "1px solid var(--vscode-input-border, #343b47)",
  borderRadius: 7,
  color: "var(--vscode-foreground, #eef1f5)",
  padding: "7px 9px",
  fontSize: 12,
};
