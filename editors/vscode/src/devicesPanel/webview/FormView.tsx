// A whole form, drawn from the Rust view model (design §4.5 "D3: forms and the Add dialog",
// specification 25 §5.4, §5.9 and §5.13). Rust gives the title, the breadcrumb, the owner line, the
// purpose, the note, the sections and every field with its label, help, placeholder, value and
// choices; this component draws them in order and keeps what the user types. It decides no label
// and no value. It evaluates one rule, for display only: a field's declared `visible_when`
// predicate, which hides the field while the user types and which Rust evaluates again on submit
// (§5.9). It draws the notice of its own pane, the fields its notice refused, and the form's
// buttons, and it draws the notice and its field errors as stale after the user edits a field
// (§5.9, J09).

import React from "react";

import { InspectorAction } from "./Inspector";
import { ListEditor } from "./ListEditor";
import type { Action } from "./types";
import type { Form, FormField, Notice, PendingAction } from "./formTypes";
import {
  fieldErrorStyle,
  fieldHelpStyle,
  fieldInputStyle,
  fieldLabelLineStyle,
  fieldLabelStyle,
  fieldRequiredStyle,
  fieldStyle,
  fieldTextareaStyle,
  formButtonsStyle,
  formHeaderStyle,
  formNoteStyle,
  formNoticeActionsStyle,
  formNoticeDetailStyle,
  formNoticeStyle,
  formNoticeTextStyle,
  formOwnerStyle,
  formPendingStyle,
  formPurposeStyle,
  formStyle,
  inspectorBreadcrumbStyle,
  inspectorTitleStyle,
  sectionHeadingStyle,
  sectionStyle,
} from "./styles";

/** The values the user has typed, keyed by field identity (J07). */
export type FormValues = Record<string, unknown>;

/** The pane name Rust gives a form's notice, so a notice is routed to its own form (§5.13). */
export function formPaneName(form: Form): string {
  return `form:${form.id}`;
}

/** A value as the text an input shows: an absent or null value is empty. */
function textValue(value: unknown): string {
  if (value === undefined || value === null) {
    return "";
  }
  return String(value);
}

/** An object value as the JSON text its area shows. */
function objectText(value: unknown): string {
  if (value === undefined || value === null) {
    return "";
  }
  return JSON.stringify(value, null, 2) ?? "";
}

/**
 * A field's starting value: the field's own value, chosen by Rust, except a secret, which starts
 * empty and never shows a stored value (specification 25 §5.4 and §5.9).
 */
function startValue(field: FormField): unknown {
  if (field.kind === "secret") {
    return "";
  }
  return field.value;
}

/** The values a form starts from, keyed by field identity. */
function initialValues(form: Form): FormValues {
  const values: FormValues = {};
  for (const section of form.sections) {
    for (const field of section.fields) {
      values[field.id] = startValue(field);
    }
  }
  return values;
}

/** Whether a field is shown for the values so far (specification 25 §5.9). */
function isVisible(field: FormField, values: FormValues): boolean {
  const rule = field.visible_when;
  if (rule === undefined) {
    return true;
  }
  return String(values[rule.field] ?? "") === rule.equals;
}

/**
 * Draws a form's header, its sections in order and every field, then its notice and its buttons, and
 * reports the values the user types. The values start from each field's value and are kept until the
 * form's `id` changes: a new form with the same `id` keeps them, another `id` starts again from its
 * values (J07). Every change is reported through `onValues`, keyed by field `id`, each as typed.
 * Choosing a button reports it, with the values typed so far, through `onAction` unchanged. It draws
 * only the notice and the pending action that name its own pane (§5.13), and, after the user edits a
 * field, it hides the stale notice and its field errors until the next notice (§5.9, J09).
 */
export function FormView({
  form,
  notice,
  pending,
  onValues,
  onAction,
}: {
  /** The form, as the view gives it. */
  form: Form;
  /** The result of an earlier action, shown in its own pane when it names this form (specification 25 §5.13). */
  notice?: Notice;
  /** What a slow action reports while it runs, shown in its own pane until a later reply. */
  pending?: PendingAction;
  /** Called with the values whenever they change, and once when they start. */
  onValues: (values: FormValues) => void;
  /** Called with a chosen button, and the values typed so far, unchanged. */
  onAction?: (action: Action, values: FormValues) => void;
}) {
  const [values, setValues] = React.useState<FormValues>(() => initialValues(form));
  const [startedId, setStartedId] = React.useState(form.id);
  // The notice and its field errors are drawn as stale once the user edits a field, until the next
  // notice arrives (§5.9, J09).
  const [editedSinceNotice, setEditedSinceNotice] = React.useState(false);
  if (startedId !== form.id) {
    setStartedId(form.id);
    setValues(initialValues(form));
  }
  // Report through the latest callback so a re-rendered host handler cannot loop, and report the
  // starting values too, so the shell can submit a form the user has not touched.
  const report = React.useRef(onValues);
  report.current = onValues;
  React.useEffect(() => {
    report.current(values);
  }, [values]);
  React.useEffect(() => {
    setEditedSinceNotice(false);
  }, [notice]);
  const change = (id: string, value: unknown) => {
    setValues({ ...values, [id]: value });
    setEditedSinceNotice(true);
  };
  // Only a notice or a pending action that names this form's pane belongs here; any other pane's
  // result is not this pane's to draw (specification 25 §5.13).
  const own = (pane: string) => pane === formPaneName(form);
  const shown = notice !== undefined && own(notice.pane) && !editedSinceNotice ? notice : undefined;
  const shownPending = pending !== undefined && own(pending.pane) ? pending : undefined;
  return (
    <div data-role="form" data-form={form.id} style={formStyle}>
      <header style={formHeaderStyle}>
        <span data-role="form-breadcrumb" title={form.breadcrumb} style={inspectorBreadcrumbStyle}>
          {form.breadcrumb}
        </span>
        <span data-role="form-title" title={form.title} style={inspectorTitleStyle}>
          {form.title}
        </span>
        <span data-role="form-owner" style={formOwnerStyle}>
          {form.owner_line}
        </span>
        {form.purpose !== undefined && (
          <p data-role="form-purpose" style={formPurposeStyle}>
            {form.purpose}
          </p>
        )}
        {form.note !== undefined && (
          <div data-role="form-note" style={formNoteStyle}>
            {form.note}
          </div>
        )}
      </header>
      {form.sections.map((section, index) => (
        <section
          key={section.heading ?? `section-${index}`}
          data-role="form-section"
          style={sectionStyle}
        >
          {section.heading !== undefined && (
            <h3 data-role="form-section-heading" style={sectionHeadingStyle}>
              {section.heading}
            </h3>
          )}
          {section.fields.map((field) =>
            isVisible(field, values) ? (
              <Field
                key={field.id}
                field={field}
                value={values[field.id]}
                error={shown?.field_errors.find((entry) => entry.field === field.id)?.message}
                onChange={(value) => change(field.id, value)}
              />
            ) : null,
          )}
        </section>
      ))}
      {shownPending !== undefined && (
        <div data-role="form-pending" style={formPendingStyle}>
          {shownPending.text}
        </div>
      )}
      {shown !== undefined && (
        <div data-role="form-notice" data-tone={shown.tone} style={formNoticeStyle(shown.tone)}>
          <span data-role="form-notice-text" style={formNoticeTextStyle(shown.tone)}>
            {shown.text}
          </span>
          {shown.detail !== undefined && (
            <span data-role="form-notice-detail" style={formNoticeDetailStyle}>
              {shown.detail}
            </span>
          )}
          {shown.actions.length > 0 && (
            <div data-role="form-notice-actions" style={formNoticeActionsStyle}>
              {shown.actions.map((action) => (
                <InspectorAction
                  key={action.id}
                  action={action}
                  onAction={(chosen) => onAction?.(chosen, values)}
                />
              ))}
            </div>
          )}
        </div>
      )}
      <div data-role="form-buttons" style={formButtonsStyle}>
        {form.buttons.map((action) => (
          <InspectorAction
            key={action.id}
            action={action}
            onAction={(chosen) => onAction?.(chosen, values)}
          />
        ))}
      </div>
    </div>
  );
}

/** One field: its label, its required mark, its input, its help and, when it was refused, why. */
function Field({
  field,
  value,
  error,
  onChange,
}: {
  field: FormField;
  value: unknown;
  /** Why the field's value was refused, when a notice named this field (§5.9). */
  error?: string;
  onChange: (value: unknown) => void;
}) {
  return (
    <div data-role="form-field" data-field={field.id} style={fieldStyle}>
      <div data-role="field-label-line" style={fieldLabelLineStyle}>
        <span data-role="field-label" style={fieldLabelStyle}>
          {field.label}
        </span>
        {field.required ? (
          <span data-role="field-required" style={fieldRequiredStyle}>
            {" *"}
          </span>
        ) : null}
      </div>
      <FieldInput field={field} value={value} onChange={onChange} />
      <span data-role="field-help" style={fieldHelpStyle}>
        {field.help}
      </span>
      {error !== undefined && (
        <span data-role="field-error" style={fieldErrorStyle}>
          {error}
        </span>
      )}
    </div>
  );
}

/**
 * The input of one field, drawn as its kind: a checkbox, a choice, a list, an area or a text input.
 * A field the user types into (text, number, secret, object) keeps its native `input` handler and
 * carries the same `onChange`, so React sees a controlled field with a change handler; a checkbox
 * and a choice are picked, not typed, and report through `onChange` (specification 25 §5.4).
 */
function FieldInput({
  field,
  value,
  onChange,
}: {
  field: FormField;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  switch (field.kind) {
    case "boolean":
      return (
        <input
          data-role="field-input"
          data-field={field.id}
          data-kind={field.kind}
          type="checkbox"
          checked={value === true}
          onChange={(event) => onChange(event.target.checked)}
        />
      );
    case "choice":
      return (
        <select
          data-role="field-input"
          data-field={field.id}
          data-kind={field.kind}
          value={textValue(value)}
          onChange={(event) => onChange(event.target.value)}
          style={fieldInputStyle}
        >
          {field.options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      );
    case "secret":
      return (
        <input
          data-role="field-input"
          data-field={field.id}
          data-kind={field.kind}
          type="password"
          value={textValue(value)}
          placeholder={field.placeholder}
          onInput={(event) => onChange(event.currentTarget.value)}
          onChange={(event) => onChange(event.currentTarget.value)}
          style={fieldInputStyle}
        />
      );
    case "list":
      return <ListEditor field={{ ...field, value }} onChange={onChange} />;
    case "object":
      return (
        <textarea
          data-role="field-input"
          data-field={field.id}
          data-kind={field.kind}
          value={objectText(value)}
          placeholder={field.placeholder}
          onInput={(event) => onChange(event.currentTarget.value)}
          onChange={(event) => onChange(event.currentTarget.value)}
          style={fieldTextareaStyle}
        />
      );
    default:
      return (
        <input
          data-role="field-input"
          data-field={field.id}
          data-kind={field.kind}
          type="text"
          value={textValue(value)}
          placeholder={field.placeholder}
          onInput={(event) => onChange(event.currentTarget.value)}
          onChange={(event) => onChange(event.currentTarget.value)}
          style={fieldInputStyle}
        />
      );
  }
}
