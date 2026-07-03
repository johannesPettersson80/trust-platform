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

export function coerce(field: CommFieldSchema, raw: string): unknown {
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
  if (t === "json_array") {
    try {
      const parsed = JSON.parse(raw || "[]");
      return Array.isArray(parsed) ? parsed : raw;
    } catch {
      return raw;
    }
  }
  return raw;
}

type JsonObject = Record<string, unknown>;

function parseArray(raw: string): { ok: true; value: unknown[] } | { ok: false; value: unknown[] } {
  try {
    const parsed = JSON.parse(raw || "[]");
    return Array.isArray(parsed) ? { ok: true, value: parsed } : { ok: false, value: [] };
  } catch {
    return { ok: false, value: [] };
  }
}

function firstTemplate(field: CommFieldSchema, items: unknown[]): unknown {
  const fromItems = items.find((item) => item !== undefined && item !== null);
  if (fromItems !== undefined) {
    return cloneValue(fromItems);
  }
  if (Array.isArray(field.default) && field.default.length > 0) {
    return cloneValue(field.default[0]);
  }
  if (/connections/i.test(field.id)) {
    return undefined;
  }
  return "";
}

function cloneValue(value: unknown): unknown {
  if (Array.isArray(value) || isPlainObject(value)) {
    return JSON.parse(JSON.stringify(value)) as unknown;
  }
  return value;
}

function isPlainObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function labelForKey(key: string): string {
  return key
    .replace(/_/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase());
}

function itemName(field: CommFieldSchema): string {
  switch (field.id) {
    case "connect":
      return "peer address";
    case "producer_instances":
      return "ST producer block";
    case "wan_allow_write":
      return "WAN write rule";
    case "link_transports":
      return "link transport";
    case "cpu_affinity":
      return "CPU index";
    default:
      break;
  }
  if (field.id === "expose") {
    return "global";
  }
  if (field.id === "modules") {
    return "module";
  }
  if (field.id === "mock_inputs") {
    return "mock frame";
  }
  if (field.id === "selected_channels") {
    return "channel";
  }
  const label = field.label.toLowerCase();
  if (label.endsWith("ies")) {
    return label.slice(0, -3) + "y";
  }
  if (label.endsWith("s")) {
    return label.slice(0, -1);
  }
  return label || "item";
}

function optionLabel(field: CommFieldSchema, value: string): string {
  if ((field.type === "bool" || field.type === "boolean") && (value === "true" || value === "false")) {
    return value === "true" ? "On" : "Off";
  }
  if (field.id === "scheduler") {
    switch (value) {
      case "fifo":
        return "FIFO";
      case "rr":
        return "Round robin";
      case "other":
        return "Default";
      default:
        return value;
    }
  }
  if (field.id === "source") {
    switch (value) {
      case "heartbeat":
        return "Heartbeat";
      case "st-fb":
        return "ST producer";
      default:
        return value;
    }
  }
  if (field.id === "fence_mode") {
    switch (value) {
      case "fenced":
        return "Fenced";
      case "unfenced":
        return "Unfenced";
      default:
        return value;
    }
  }
  if (field.id === "profile") {
    switch (value) {
      case "dev":
        return "Development";
      case "plant":
        return "Plant";
      case "wan":
        return "WAN";
      default:
        return value;
    }
  }
  if (field.id === "on_error") {
    switch (value) {
      case "fault":
        return "Stop with fault";
      case "warn":
        return "Warn and continue";
      case "ignore":
        return "Ignore";
      default:
        return value;
    }
  }
  return value;
}

function emptyArrayMessage(field: CommFieldSchema, canAdd: boolean): string {
  if (/connections/i.test(field.id)) {
    return "No connections yet. Use Discover or Browse to add one instead of typing JSON.";
  }
  if (field.id === "expose") {
    return canAdd
      ? "No globals selected yet. Use Choose globals to pick project variables, or add a pattern manually."
      : "No globals selected yet.";
  }
  return canAdd
    ? `No ${field.label.toLowerCase()} yet. Add an item below.`
    : `No ${field.label.toLowerCase()} yet.`;
}

function valueKind(value: unknown): "number" | "boolean" | "text" {
  if (typeof value === "number") {
    return "number";
  }
  if (typeof value === "boolean") {
    return "boolean";
  }
  return "text";
}

function stringValue(value: unknown): string {
  if (value === undefined || value === null) {
    return "";
  }
  return String(value);
}

function parseEditedValue(raw: string, previous: unknown): unknown {
  switch (valueKind(previous)) {
    case "number": {
      const parsed = Number(raw);
      return Number.isFinite(parsed) ? parsed : raw;
    }
    case "boolean":
      return raw === "true";
    case "text":
    default:
      return raw;
  }
}

function objectKeys(item: JsonObject, template: JsonObject): string[] {
  return Array.from(new Set([...Object.keys(template), ...Object.keys(item)]));
}

function BooleanControl({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <span className="trust-checkbox">
      <input
        type="checkbox"
        aria-label={label}
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>{checked ? "On" : "Off"}</span>
    </span>
  );
}

function JsonArrayField({
  field,
  value,
  inputClass,
  onChange,
}: {
  field: CommFieldSchema;
  value: string;
  inputClass: string;
  onChange: (v: string) => void;
}) {
  const parsed = parseArray(value);
  const items = parsed.value;
  const template = firstTemplate(field, items);
  const canAdd = template !== undefined;
  const updateItems = (next: unknown[]) => onChange(JSON.stringify(next));
  const addItem = () => {
    if (template === undefined) {
      return;
    }
    updateItems([...items, cloneValue(template)]);
  };
  const removeItem = (index: number) => {
    updateItems(items.filter((_, itemIndex) => itemIndex !== index));
  };
  const updateItem = (index: number, next: unknown) => {
    updateItems(items.map((item, itemIndex) => (itemIndex === index ? next : item)));
  };

  if (!parsed.ok) {
    return (
      <div className="trust-array">
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          rows={3}
          className={inputClass}
          style={{ resize: "vertical", fontFamily: "var(--trust-mono)" }}
        />
        <div className="trust-field__message trust-field__message--error">
          Existing value is not a valid list yet. Fix it here or reset the field.
        </div>
      </div>
    );
  }

  return (
    <div className="trust-array" data-field-type="json_array">
      {items.length === 0 ? (
        <div className="trust-array__empty">{emptyArrayMessage(field, canAdd)}</div>
      ) : (
        items.map((item, index) => (
          <JsonArrayItem
            key={index}
            index={index}
            itemLabel={itemName(field)}
            item={item}
            template={template}
            onRemove={() => removeItem(index)}
            onChange={(next) => updateItem(index, next)}
          />
        ))
      )}
      {canAdd && (
        <button type="button" className="trust-button trust-array__add" onClick={addItem}>
          Add {itemName(field)}
        </button>
      )}
    </div>
  );
}

function JsonArrayItem({
  index,
  itemLabel,
  item,
  template,
  onRemove,
  onChange,
}: {
  index: number;
  itemLabel: string;
  item: unknown;
  template: unknown;
  onRemove: () => void;
  onChange: (next: unknown) => void;
}) {
  if (isPlainObject(item)) {
    const objectTemplate = isPlainObject(template) ? template : {};
    const keys = objectKeys(item, objectTemplate);
    return (
      <div className="trust-array__item">
        <div className="trust-array__item-header">
          <span>{itemLabel.charAt(0).toUpperCase() + itemLabel.slice(1)} {index + 1}</span>
          <button type="button" className="trust-button trust-array__remove" onClick={onRemove}>
            Remove
          </button>
        </div>
        <div className="trust-array__grid">
          {keys.map((key) => {
            const current = item[key] ?? objectTemplate[key] ?? "";
            const kind = valueKind(current);
            return (
              <label key={key} className="trust-array__property">
                <span>{labelForKey(key)}</span>
                {kind === "boolean" ? (
                  <BooleanControl
                    checked={Boolean(current)}
                    label={labelForKey(key)}
                    onChange={(checked) => onChange({ ...item, [key]: checked })}
                  />
                ) : (
                  <input
                    className="trust-input"
                    type={kind === "number" ? "number" : "text"}
                    value={stringValue(current)}
                    onChange={(e) => onChange({ ...item, [key]: parseEditedValue(e.target.value, current) })}
                  />
                )}
              </label>
            );
          })}
        </div>
      </div>
    );
  }

  const kind = valueKind(item);
  return (
    <div className="trust-array__item">
      <div className="trust-array__item-header">
        <span>{itemLabel.charAt(0).toUpperCase() + itemLabel.slice(1)} {index + 1}</span>
        <button type="button" className="trust-button trust-array__remove" onClick={onRemove}>
          Remove
        </button>
      </div>
      {kind === "boolean" ? (
        <BooleanControl
          checked={Boolean(item)}
          label={itemLabel}
          onChange={(checked) => onChange(checked)}
        />
      ) : (
        <input
          className="trust-input"
          type={kind === "number" ? "number" : "text"}
          value={stringValue(item)}
          onChange={(e) => onChange(parseEditedValue(e.target.value, item))}
        />
      )}
    </div>
  );
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
  const isBooleanField = field.type === "bool" || field.type === "boolean";
  return (
    <div className="trust-field">
      <label>
        {field.label}
        {field.required && <span className="trust-field__required"> *</span>}
      </label>
      {isBooleanField ? (
        <BooleanControl
          checked={value === "true"}
          label={field.label}
          onChange={(checked) => onChange(String(checked))}
        />
      ) : field.options && field.options.length > 0 ? (
        <select value={value} onChange={(e) => onChange(e.target.value)} className={inputClass}>
          {field.options.map((o) => (
            <option key={o} value={o}>
              {optionLabel(field, o)}
            </option>
          ))}
        </select>
      ) : field.type === "json_array" ? (
        <JsonArrayField field={field} value={value} inputClass={inputClass} onChange={onChange} />
      ) : field.type === "json_object" ? (
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          rows={3}
          className={inputClass}
          style={{ resize: "vertical", fontFamily: "var(--trust-mono)" }}
        />
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
