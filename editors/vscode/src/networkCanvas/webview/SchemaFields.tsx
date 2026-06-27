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
  const inputClass = error ? "trust-input trust-input--error" : "trust-input";
  return (
    <div className="trust-field">
      <label>
        {field.label}
        {field.required && <span className="trust-field__required"> *</span>}
      </label>
      {field.options && field.options.length > 0 ? (
        <select value={value} onChange={(e) => onChange(e.target.value)} className={inputClass}>
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
          className={inputClass}
          style={{ resize: "vertical", fontFamily: "var(--trust-mono)" }}
        />
      ) : field.type === "bool" || field.type === "boolean" ? (
        <select value={value} onChange={(e) => onChange(e.target.value)} className={inputClass}>
          <option value="false">false</option>
          <option value="true">true</option>
        </select>
      ) : (
        <input
          type={field.secret ? "password" : field.type === "number" ? "number" : "text"}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className={inputClass}
        />
      )}
      {error ? (
        <div className="trust-field__message trust-field__message--error">{error}</div>
      ) : field.help ? (
        <div className="trust-field__message">{field.help}</div>
      ) : null}
    </div>
  );
}
