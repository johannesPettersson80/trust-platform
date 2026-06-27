import React, { useEffect, useMemo, useRef, useState } from "react";
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
  zIndex: 8,
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

  // Track that THIS panel issued a Save (not a Test), so the close-on-success effect only fires for a
  // real save the user just triggered — not a stale applyResult left over from before the panel opened.
  const savingRef = useRef(false);
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
    if (type === "commSave") {
      savingRef.current = true;
    }
    post({ type, protocol: protocol.id, params, action, runtimeId: target?.id, target: target?.id });
  };

  // On a successful Save, close the panel: this clears the draft preview (no stale "DRAFT" lingering next
  // to the saved device) and the real device now on the canvas is the success signal. Done synchronously
  // so a follow-up poll updating applyResult can't cancel it. (A Test keeps the panel open; only an applied
  // Save closes it.)
  useEffect(() => {
    if (savingRef.current && applyResult?.applied) {
      savingRef.current = false;
      onClose();
    }
  }, [applyResult, onClose]);

  const ok = applyResult && (applyResult.applied || applyResult.lifecycle_effect === "test_ok");
  const blocked = applyResult && applyResult.lifecycle_effect === "blocked";

  return (
    <aside className="trust-inspector" style={PANEL_STYLE} aria-label="Add device">
      <header className="trust-inspector__header">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__title">{protocol ? `Add ${protocol.title}` : "Add device"}</div>
          {target?.name && <div className="trust-inspector__eyebrow" style={{ marginTop: 2 }}>on {target.name}</div>}
        </div>
        <button onClick={onClose} aria-label="Close" style={iconBtn}>✕</button>
      </header>

      <div className="trust-section trust-section--grow">
        {protocols.length === 0 ? (
          <p className="trust-empty">
            {setupMessage ?? "Device catalog unavailable (needs a newer trust-runtime)."}
          </p>
        ) : (
          <>
            <div className="trust-field">
              <label>Protocol</label>
              <select
                className="trust-input"
                value={protocolId}
                onChange={(e) => setProtocolId(e.target.value)}
              >
                {protocols.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.title}
                  </option>
                ))}
              </select>
            </div>
            {protocol?.purpose && (
              <p className="trust-help" style={{ marginBottom: 14 }}>{protocol.purpose}</p>
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
                className={`trust-message ${ok ? "trust-message--ok" : blocked ? "trust-message--error" : ""}`}
              >
                {applyResult.message || (ok ? "Applied." : "")}
                {applyResult.lifecycle_effect && applyResult.lifecycle_effect !== "blocked" && (
                  <div className="trust-message__detail">{applyResult.lifecycle_effect}</div>
                )}
              </div>
            )}
          </>
        )}
      </div>

      {protocols.length > 0 && (
        <footer className="trust-section" style={{ display: "flex", flexWrap: "wrap", gap: 8, borderBottom: "none" }}>
          <button onClick={() => submit("commSave")} className="trust-button trust-button--primary" style={{ flex: 1 }}>Save</button>
          {protocol?.supports_test && reachable && (
            <button onClick={() => submit("commTest")} className="trust-button">Test</button>
          )}
          {reachable && (
            <button onClick={() => submit("commApplyLive")} className="trust-button" style={{ flexBasis: "100%" }}>
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
  return (
    <div className="trust-field">
      <label>
        {field.label}
        {field.required && <span className="trust-field__required"> *</span>}
      </label>
      {field.options && field.options.length > 0 ? (
        <select value={value} onChange={(e) => onChange(e.target.value)} className={error ? "trust-input trust-input--error" : "trust-input"}>
          {field.options.map((o) => (
            <option key={o} value={o}>{o}</option>
          ))}
        </select>
      ) : field.type === "json_object" ? (
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          rows={3}
          className={error ? "trust-input trust-input--error" : "trust-input"}
          style={{ resize: "vertical", fontFamily: "var(--trust-mono)" }}
        />
      ) : field.type === "bool" || field.type === "boolean" ? (
        <select value={value} onChange={(e) => onChange(e.target.value)} className={error ? "trust-input trust-input--error" : "trust-input"}>
          <option value="false">false</option>
          <option value="true">true</option>
        </select>
      ) : (
        <input
          type={field.secret ? "password" : field.type === "number" ? "number" : "text"}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className={error ? "trust-input trust-input--error" : "trust-input"}
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

const iconBtn: React.CSSProperties = {
  border: "none",
  background: "transparent",
  color: "var(--trust-text-muted)",
  fontSize: 14,
  cursor: "pointer",
  padding: 0,
};
