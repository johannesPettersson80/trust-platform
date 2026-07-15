import React, { useEffect, useMemo, useRef, useState } from "react";
import type {
  CommApplyResponse,
  CommProtocolSchema,
  CommSchemaResponse,
} from "../../communication/schemaForm";
import { visibleSchemaFields } from "../../communication/schemaForm";
import { coerce, Field } from "./SchemaFields";
import { t } from "./theme";

interface Props {
  schema?: CommSchemaResponse;
  applyResult?: CommApplyResponse;
  reachable: boolean;
  setupMessage?: string;
  target?: { id: string; name: string };
  preselectProtocol?: string;
  preselectParams?: Record<string, unknown>; // prefill from a discovered candidate (§0.5 Browse→Add)
  post: (message: unknown) => void;
  onValidationStale?: () => void;
  onClose: () => void;
  onSaved?: (nodeId?: string) => void;
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

export function AddDevicePanel({ schema, applyResult, reachable, setupMessage, target, preselectProtocol, preselectParams, post, onValidationStale, onClose, onSaved }: Props) {
  const protocols = useMemo(() => schema?.protocols ?? [], [schema]);
  const [protocolId, setProtocolId] = useState<string>(preselectProtocol ?? "");
  const [values, setValues] = useState<Record<string, string>>({});
  const preselectParamsKey = useMemo(
    () => JSON.stringify(preselectParams ?? null),
    [preselectParams]
  );
  const lastInitializedKey = useRef<string>("");

  const protocol = protocols.find((p) => p.id === protocolId);
  const applyResultSignature = useMemo(
    () =>
      applyResult
        ? JSON.stringify({
            applied: applyResult.applied,
            lifecycle_effect: applyResult.lifecycle_effect,
            message: applyResult.message,
            field_errors: applyResult.field_errors ?? [],
          })
        : "",
    [applyResult]
  );
  const [editedAfterApplyResult, setEditedAfterApplyResult] = useState(false);
  const [clearedFieldErrors, setClearedFieldErrors] = useState<Set<string>>(
    () => new Set()
  );

  // Pick the dropped protocol (or the first available) when a schema arrives.
  useEffect(() => {
    if (protocols.length > 0 && !protocols.some((p) => p.id === protocolId)) {
      const wanted = protocols.find((p) => p.id === preselectProtocol);
      setProtocolId((wanted ?? protocols[0]).id);
    }
  }, [protocols, protocolId, preselectProtocol]);

  // Reset field values only when the selected protocol or prefill changes. The schema/meta stream can
  // refresh while the drawer is open; that must not wipe fields the user is actively editing.
  useEffect(() => {
    if (!protocol) {
      return;
    }
    const initKey = `${protocol.id}\n${preselectParamsKey}`;
    if (lastInitializedKey.current !== initKey) {
      lastInitializedKey.current = initKey;
      setValues(valuesWithPrefill(protocol, preselectParams));
    }
  }, [protocol, preselectParams, preselectParamsKey]);

  useEffect(() => {
    setEditedAfterApplyResult(false);
    setClearedFieldErrors(new Set());
  }, [applyResultSignature]);

  const rawFieldErrors = new Map(
    (applyResult?.field_errors ?? []).map((e) => [e.field, e.message])
  );
  const fieldErrors = new Map(
    [...rawFieldErrors].filter(([field]) => !clearedFieldErrors.has(field))
  );
  const visibleApplyResult = editedAfterApplyResult ? undefined : applyResult;
  const visibleFields = protocol ? visibleSchemaFields(protocol, values) : [];

  const updateField = (fieldId: string, value: string) => {
    setValues((prev) => ({ ...prev, [fieldId]: value }));
    if (applyResult) {
      setEditedAfterApplyResult(true);
      onValidationStale?.();
      setClearedFieldErrors((prev) => {
        if (prev.has(fieldId)) {
          return prev;
        }
        const next = new Set(prev);
        next.add(fieldId);
        return next;
      });
    }
  };

  // Track that THIS panel issued a Save (not a Test), so the close-on-success effect only fires for a
  // real save the user just triggered — not a stale applyResult left over from before the panel opened.
  const savingRef = useRef(false);
  const submit = (type: string) => {
    if (!protocol) {
      return;
    }
    const params: Record<string, unknown> = {};
    for (const field of visibleSchemaFields(protocol, values)) {
      params[field.id] = coerce(field, values[field.id] ?? "");
    }
    const action =
      protocol.supports_multi_instance && protocol.actions.includes("add") ? "add" : "upsert";
    setEditedAfterApplyResult(false);
    setClearedFieldErrors(new Set());
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
      if (onSaved) {
        onSaved(applyResult.instance_id);
      } else {
        onClose();
      }
    }
  }, [applyResult, onClose, onSaved]);

  const ok = visibleApplyResult && (visibleApplyResult.applied || visibleApplyResult.lifecycle_effect === "test_ok");
  const blocked = visibleApplyResult && visibleApplyResult.lifecycle_effect === "blocked";
  const lifecycleDetail =
    visibleApplyResult?.lifecycle_effect &&
    !["blocked", "test_ok"].includes(visibleApplyResult.lifecycle_effect)
      ? visibleApplyResult.lifecycle_effect
      : undefined;

  return (
    <aside className="trust-inspector" style={PANEL_STYLE} aria-label="Add device">
      <header className="trust-inspector__header">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__eyebrow">Devices & Connections / Add device or connection</div>
          <div className="trust-inspector__title">{protocol ? `Add ${protocol.title}` : "Add device"}</div>
          {target?.name && <div className="trust-inspector__eyebrow" style={{ marginTop: 2 }}>on {target.name}</div>}
        </div>
        <button onClick={onClose} aria-label="Close" style={iconBtn}>✕</button>
      </header>

      <div className="trust-section trust-section--grow" style={{ paddingBottom: 18 }}>
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

            {visibleFields.map((field) => (
              <Field
                key={field.id}
                field={field}
                value={values[field.id] ?? ""}
                error={fieldErrors.get(field.id)}
                onChange={(v) => updateField(field.id, v)}
              />
            ))}
          </>
        )}
      </div>

      {/* Pinned between the scroll body and the footer so the apply/validation result —
          especially its 2nd line — is never hidden behind the Save/Cancel footer. */}
      {visibleApplyResult && (
        <div
          className={`trust-message ${ok ? "trust-message--ok" : blocked ? "trust-message--error" : ""}`}
          style={{ margin: "0 14px 10px" }}
        >
          {visibleApplyResult.message || (ok ? "Applied." : "")}
          {lifecycleDetail && (
            <div className="trust-message__detail">{lifecycleDetail}</div>
          )}
        </div>
      )}

      {protocols.length > 0 && (
        <footer className="trust-section" style={{ display: "flex", flexWrap: "wrap", gap: 8, borderBottom: "none", borderTop: `1px solid ${t.border}`, background: t.surface }}>
          <button onClick={() => submit("commSave")} className="trust-button trust-button--primary" style={{ flex: 1 }}>Save</button>
          <button onClick={onClose} className="trust-button">Cancel</button>
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

const iconBtn: React.CSSProperties = {
  border: "none",
  background: "transparent",
  color: "var(--trust-text-muted)",
  fontSize: 14,
  cursor: "pointer",
  padding: 0,
};
