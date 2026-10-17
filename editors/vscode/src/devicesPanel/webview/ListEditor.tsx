// One list field of a form, drawn from the Rust view model (design §4.5 "D3: forms and the Add
// dialog", specification 25 §5.4 and §5.9). Rust decides an item's title and the number after it,
// what shows while the list is empty, whether an item can be added, what one item holds, each
// property's label, its choices, and the properties the item may set instead. This component draws
// those and reports every change of the value through `onChange`; it decides no label and no rule.

import React from "react";

import type {
  FieldKind,
  FormField,
  ListEditor as ListEditorModel,
  ListProperty,
} from "./formTypes";
import {
  listAddStyle,
  listEditorStyle,
  listEmptyStyle,
  listItemHeaderStyle,
  listItemStyle,
  listItemTitleStyle,
  listPropertyGridStyle,
  listPropertyInputStyle,
  listPropertyLabelLineStyle,
  listPropertyLabelStyle,
  listPropertyRequiredStyle,
  listPropertyRowStyle,
  listPropertyStyle,
  listRemoveStyle,
} from "./styles";

/** Whether `value` is a JSON object rather than a scalar or an array. */
function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** A deep copy of a list item, so editing an item never changes the template it came from. */
function cloneItem(item: unknown): unknown {
  if (item === null || typeof item !== "object") {
    return item;
  }
  return JSON.parse(JSON.stringify(item)) as unknown;
}

/** A value as the text an input shows: an absent or null value is empty. */
function textValue(value: unknown): string {
  if (value === undefined || value === null) {
    return "";
  }
  return String(value);
}

/**
 * One list field (specification 25 §5.4 and §5.9): each item under its numbered title, with one
 * input per property for an object item and the way to set an alternative beside it; the button
 * that appends `item_template`; a remove button on each item; and `empty_text` while there are no
 * items. Every change of the value is reported to `onChange`.
 */
export function ListEditor({
  field,
  onChange,
}: {
  /** The list field, as the view gives it. */
  field: FormField;
  /** Called with the new value whenever an item is added, changed or removed. */
  onChange: (value: unknown) => void;
}) {
  const editor = field.list;
  if (editor === undefined) {
    return null;
  }
  const items: unknown[] = Array.isArray(field.value) ? field.value : [];
  const replace = (index: number, item: unknown) => {
    onChange(items.map((current, at) => (at === index ? item : current)));
  };
  const remove = (index: number) => {
    onChange(items.filter((_, at) => at !== index));
  };
  const add = () => {
    onChange([...items, cloneItem(editor.item_template)]);
  };
  return (
    <div data-role="list-editor" data-field={field.id} style={listEditorStyle}>
      {items.length === 0 ? (
        <div data-role="list-empty" style={listEmptyStyle}>
          {editor.empty_text}
        </div>
      ) : (
        items.map((item, index) => (
          <div key={index} data-role="list-item" style={listItemStyle}>
            <div data-role="list-item-header" style={listItemHeaderStyle}>
              <span data-role="list-item-title" style={listItemTitleStyle}>
                {`${editor.item_title} ${index + 1}`}
              </span>
              <button
                type="button"
                data-role="list-remove"
                aria-label="Remove item"
                onClick={() => remove(index)}
                style={listRemoveStyle}
              >
                ✕
              </button>
            </div>
            {isPlainObject(item) ? (
              <ObjectItem item={item} editor={editor} onChange={(next) => replace(index, next)} />
            ) : (
              <ItemInput
                kind={editor.item_kind}
                value={item}
                onChange={(next) => replace(index, next)}
              />
            )}
          </div>
        ))
      )}
      <button
        type="button"
        data-role="list-add"
        disabled={!editor.can_add}
        onClick={add}
        style={listAddStyle(editor.can_add)}
      >
        {editor.add_label}
      </button>
    </div>
  );
}

/** An object item: one input per property the item sets, with its alternatives beside it. */
function ObjectItem({
  item,
  editor,
  onChange,
}: {
  item: Record<string, unknown>;
  editor: ListEditorModel;
  onChange: (item: Record<string, unknown>) => void;
}) {
  const properties = editor.properties.filter((property) =>
    Object.prototype.hasOwnProperty.call(item, property.key),
  );
  return (
    <div data-role="list-item-properties" style={listPropertyGridStyle}>
      {properties.map((property) => (
        <PropertyField
          key={property.key}
          property={property}
          item={item}
          properties={editor.properties}
          onChange={onChange}
        />
      ))}
    </div>
  );
}

/** One property of an object item and, when it has them, the way to set an alternative instead. */
function PropertyField({
  property,
  item,
  properties,
  onChange,
}: {
  property: ListProperty;
  item: Record<string, unknown>;
  properties: ListProperty[];
  onChange: (item: Record<string, unknown>) => void;
}) {
  const choices = [property.key, ...property.alternatives]
    .map((key) => properties.find((candidate) => candidate.key === key))
    .filter((candidate): candidate is ListProperty => candidate !== undefined);
  return (
    <div data-role="list-property" data-property={property.key} style={listPropertyStyle}>
      <div
        data-role="list-property-label-line"
        data-required={property.required ? "true" : "false"}
        style={listPropertyLabelLineStyle}
      >
        <span data-role="list-property-label" style={listPropertyLabelStyle}>
          {property.label}
        </span>
        {property.required ? (
          <span data-role="list-property-required" style={listPropertyRequiredStyle}>
            {" *"}
          </span>
        ) : null}
      </div>
      <div style={listPropertyRowStyle}>
        <PropertyInput
          property={property}
          value={item[property.key]}
          onChange={(next) => onChange({ ...item, [property.key]: next })}
        />
        {choices.length > 1 ? (
          <select
            data-role="list-alternative"
            aria-label="Property the item sets"
            value={property.key}
            onChange={(event) => {
              const chosen = event.target.value;
              const next: Record<string, unknown> = { ...item };
              delete next[property.key];
              next[chosen] = "";
              onChange(next);
            }}
            style={listPropertyInputStyle}
          >
            {choices.map((choice) => (
              <option key={choice.key} value={choice.key}>
                {choice.label}
              </option>
            ))}
          </select>
        ) : null}
      </div>
    </div>
  );
}

/** The input of one property, drawn as its kind: a checkbox, a choice of its options or a field. */
function PropertyInput({
  property,
  value,
  onChange,
}: {
  property: ListProperty;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  if (property.kind === "boolean") {
    return (
      <input
        type="checkbox"
        data-role="list-property-input"
        data-property={property.key}
        checked={value === true}
        onChange={(event) => onChange(event.target.checked)}
      />
    );
  }
  if (property.kind === "choice") {
    return (
      <select
        data-role="list-property-input"
        data-property={property.key}
        value={textValue(value)}
        onChange={(event) => onChange(event.target.value)}
        style={listPropertyInputStyle}
      >
        {property.options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    );
  }
  return (
    <input
      type={property.kind === "number" ? "number" : "text"}
      data-role="list-property-input"
      data-property={property.key}
      value={textValue(value)}
      onChange={(event) => onChange(event.target.value)}
      style={listPropertyInputStyle}
    />
  );
}

/** The input of a scalar item, drawn as its kind. */
function ItemInput({
  kind,
  value,
  onChange,
}: {
  kind: FieldKind;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  if (kind === "boolean") {
    return (
      <input
        type="checkbox"
        data-role="list-item-input"
        checked={value === true}
        onChange={(event) => onChange(event.target.checked)}
      />
    );
  }
  return (
    <input
      type={kind === "number" ? "number" : "text"}
      data-role="list-item-input"
      value={textValue(value)}
      onChange={(event) => onChange(event.target.value)}
      style={listPropertyInputStyle}
    />
  );
}
