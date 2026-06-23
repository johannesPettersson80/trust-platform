import type { RuntimeCredentialChannel } from "../runtimeTarget";

// v4 (spec §0.4/§10.2): the add taxonomy. A protocol's intent category + the file it writes.
export type CommCategory = "field_device" | "supervisory_service" | "peer_link";

// Device archetype (drive/sensor/remote-io…) for field_device protocols. Each preselects a
// transport + field defaults + icon. Backend-owned (comm.schema profiles[]); never UI-only.
export interface CommProfileSchema {
  id: string;
  title: string;
  icon?: string;
  defaults?: Record<string, unknown>;
}

export interface CommSchemaResponse {
  schema_version: number;
  family?: string; // v4: removed (was the misleading "io"); category moves per-protocol
  protocols: CommProtocolSchema[];
}

export interface CommProtocolSchema {
  id: string;
  driver: string;
  title: string;
  purpose: string;
  // v4 (§10.2): add taxonomy + universal writer. Optional until Codex lands them.
  category?: CommCategory;
  config_home?: "io.toml" | "runtime.toml" | "ads.toml" | string;
  profiles?: CommProfileSchema[];
  apply_mode?: "native" | "snippet" | "file" | string;
  lifecycle_effect: string;
  supports_test: boolean;
  supports_multi_instance: boolean;
  actions: string[];
  fields: CommFieldSchema[];
  instances?: CommConfiguredInstance[];
}

export interface CommFieldSchema {
  id: string;
  label: string;
  type: string;
  required: boolean;
  advanced: boolean;
  secret: boolean;
  help: string;
  default?: unknown;
  validation?: unknown;
  options?: string[];
}

export interface CommConfiguredInstance {
  id: string;
  driver: string;
  display_name: string;
  params: Record<string, unknown>;
}

export interface CommApplyResponse {
  schema_version: number;
  protocol: string;
  driver: string;
  action: string;
  applied: boolean;
  lifecycle_effect: string;
  message: string;
  config_path?: string;
  instance_id?: string;
  field_errors?: Array<{ field: string; message: string }>;
  snippet?: string;
}

export interface CommFieldValidationError {
  field: string;
  message: string;
}

export interface SchemaFormRenderOptions {
  readonly submitMessageType?: string;
  readonly testMessageType?: string;
  readonly clientErrorMessageType?: string;
}

export function renderSchemaForm(
  schema: CommProtocolSchema,
  applyResult?: CommApplyResponse
): string {
  const fieldErrors = new Map(
    (applyResult?.field_errors ?? []).map((error) => [error.field, error.message])
  );
  const instances = schema.instances ?? [];
  const supportsInstances = schema.supports_multi_instance && schema.actions.includes("add");
  const canEdit = instances.length > 0 && schema.actions.includes("edit");
  const canRemove = instances.length > 0 && schema.actions.includes("remove");
  const canDisable = instances.length > 0 && schema.actions.includes("disable");
  const primaryAction =
    schema.apply_mode === "snippet"
      ? "validate"
      : instances.length > 0 && supportsInstances
        ? "add"
        : "upsert";
  const primaryLabel =
    schema.apply_mode === "snippet"
      ? "Generate snippet"
      : instances.length > 0 && supportsInstances
        ? `Add new ${schema.title}`
        : "Apply";
  return `<section class="schema-form" data-schema-protocol="${escapeAttribute(schema.id)}">
    <h4>${escapeHtml(schema.title)} setup</h4>
    <p class="setup-purpose">${escapeHtml(schema.purpose)}</p>
    <div class="setup-guidance">
      <span><strong>Preset:</strong> recommended defaults are already filled in.</span>
      <button type="button" class="secondary compact" data-schema-preset="defaults">Use defaults</button>
    </div>
    <p class="setup-next">${escapeHtml(whatHappensNext(schema))}</p>
    ${applyResult ? renderApplyResult(applyResult) : ""}
    <form data-action="commApply" data-protocol="${escapeAttribute(schema.id)}">
      ${
        instances.length > 0
          ? `<label class="field">
              <span>Configured instance</span>
              <select name="instance_id">
                <option value="" data-params="{}">New ${escapeHtml(schema.title)}</option>
                ${instances
                  .map(
                    (instance) =>
                      `<option value="${escapeAttribute(instance.id)}" data-params="${escapeAttribute(JSON.stringify(instance.params ?? {}))}">${escapeHtml(instance.display_name)}</option>`
                  )
                  .join("")}
              </select>
            </label>`
          : ""
      }
      <div class="form-grid">
        ${schema.fields.map((field) => renderField(field, fieldErrors.get(field.id))).join("")}
      </div>
      <div class="actions">
        <button type="submit" data-apply-action="${escapeAttribute(primaryAction)}">${escapeHtml(primaryLabel)}</button>
        ${
          canEdit
            ? `<button type="submit" class="secondary" data-apply-action="edit">Update selected</button>`
            : ""
        }
        <button type="submit" class="secondary" data-apply-action="validate">Validate only</button>
        ${
          schema.supports_test
            ? `<button type="submit" class="secondary" data-apply-action="test">Test connection</button>`
            : ""
        }
        ${
          canRemove
            ? `<button type="submit" class="secondary" data-apply-action="remove">Remove selected</button>`
            : ""
        }
        ${
          canDisable
            ? `<button type="submit" class="secondary" data-apply-action="disable">Disable selected</button>`
            : ""
        }
      </div>
    </form>
  </section>`;
}

export function schemaFormClientScript(
  options: SchemaFormRenderOptions = {}
): string {
  const submitMessageType = JSON.stringify(options.submitMessageType ?? "commApply");
  const testMessageType = JSON.stringify(options.testMessageType ?? "commTest");
  const clientErrorMessageType = JSON.stringify(
    options.clientErrorMessageType ?? "commApplyClientError"
  );
  return `
    function fillSchemaFormWithParams(form, params) {
      for (const input of form.querySelectorAll("[data-field-id]")) {
        const fieldId = input.dataset.fieldId;
        const fieldType = input.dataset.fieldType;
        if (!fieldId) continue;
        let value = Object.prototype.hasOwnProperty.call(params, fieldId)
          ? params[fieldId]
          : undefined;
        if (value === undefined) {
          try {
            value = JSON.parse(input.dataset.fieldDefault || "null");
          } catch (error) {
            value = "";
          }
        }
        if (fieldType === "boolean") {
          input.checked = value === true || value === "true";
        } else if (fieldType === "json_array" || fieldType === "json_object") {
          input.value = JSON.stringify(value ?? (fieldType === "json_array" ? [] : {}), null, 2);
        } else {
          input.value = value === null || value === undefined ? "" : String(value);
        }
      }
    }
    document.addEventListener("click", (event) => {
      const preset = event.target.closest("[data-schema-preset]");
      if (!preset) return;
      const form = preset.closest(".schema-form")?.querySelector("form[data-action='commApply']");
      if (!form) return;
      fillSchemaFormWithParams(form, {});
    });
    document.addEventListener("change", (event) => {
      const select = event.target.closest("select[name='instance_id']");
      if (!select) return;
      const form = select.closest("form[data-action='commApply']");
      if (!form) return;
      const selected = select.selectedOptions && select.selectedOptions[0];
      let params = {};
      try {
        params = JSON.parse(selected?.dataset?.params || "{}");
      } catch (error) {
        params = {};
      }
      fillSchemaFormWithParams(form, params);
    });
    document.addEventListener("submit", (event) => {
      const form = event.target.closest("form[data-action='commApply']");
      if (!form) return;
      event.preventDefault();
      const submitter = event.submitter;
      const params = {};
      const errors = [];
      for (const input of form.querySelectorAll("[data-field-id]")) {
        const fieldId = input.dataset.fieldId;
        const fieldType = input.dataset.fieldType;
        if (!fieldId) continue;
        if (fieldType === "boolean") {
          params[fieldId] = input.checked === true;
        } else if (fieldType === "number") {
          params[fieldId] = Number(input.value);
        } else if (fieldType === "json_array" || fieldType === "json_object") {
          try {
            params[fieldId] = JSON.parse(input.value || (fieldType === "json_array" ? "[]" : "{}"));
          } catch (error) {
            errors.push({ field: fieldId, message: fieldType === "json_array" ? "Enter a valid JSON array." : "Enter a valid JSON object." });
          }
        } else {
          params[fieldId] = input.value;
        }
      }
      if (errors.length > 0) {
        vscode.postMessage({
          type: ${clientErrorMessageType},
          protocol: form.dataset.protocol,
          fieldErrors: errors,
        });
        return;
      }
      vscode.postMessage({
        type: (submitter?.dataset?.applyAction || "upsert") === "test" ? ${testMessageType} : ${submitMessageType},
        protocol: form.dataset.protocol,
        action: submitter?.dataset?.applyAction || "upsert",
        instanceId: form.elements.namedItem("instance_id")?.value || undefined,
        params,
      });
    });`;
}

export function shouldBlockSecretApply(
  schema: CommProtocolSchema,
  values: Record<string, unknown>,
  credentialChannel: RuntimeCredentialChannel
): boolean {
  if (credentialChannel !== "untrusted_remote_plain_tcp") {
    return false;
  }
  return schema.fields.some((field) => {
    if (!field.secret) {
      return false;
    }
    const value = values[field.id];
    return typeof value === "string" && value.trim().length > 0;
  });
}

export function validateSchemaValues(
  schema: CommProtocolSchema,
  values: Record<string, unknown>
): CommFieldValidationError[] {
  const errors: CommFieldValidationError[] = [];
  for (const field of schema.fields) {
    const value = values[field.id];
    if (field.required && valueIsEmpty(value)) {
      errors.push({ field: field.id, message: "This field is required." });
      continue;
    }
    if (valueIsEmpty(value)) {
      continue;
    }
    if (field.options && typeof value === "string" && !field.options.includes(value)) {
      errors.push({ field: field.id, message: "Choose a listed value." });
    }
    if (field.type === "number" && typeof value !== "number") {
      errors.push({ field: field.id, message: "Enter a whole number." });
    }
    if (field.type === "endpoint" && typeof value === "string") {
      const validation = isRecord(field.validation) ? field.validation : undefined;
      if (
        validation?.kind === "socket_addr" &&
        !looksLikeSocketAddress(value)
      ) {
        errors.push({
          field: field.id,
          message: "Use an IP address and port, for example 127.0.0.1:502.",
        });
      } else if (!looksLikeEndpoint(value)) {
        errors.push({ field: field.id, message: "Use host:port, for example 127.0.0.1:502." });
      }
    }
    if (field.type === "json_array" && !Array.isArray(value)) {
      errors.push({ field: field.id, message: "Enter a JSON array." });
    }
    if (
      field.type === "json_object" &&
      (value === null || typeof value !== "object" || Array.isArray(value))
    ) {
      errors.push({ field: field.id, message: "Enter a JSON object." });
    }
    const validation = isRecord(field.validation) ? field.validation : undefined;
    if (
      validation?.kind === "integer_range" &&
      typeof value === "number" &&
      (value < Number(validation.min) || value > Number(validation.max))
    ) {
      errors.push({
        field: field.id,
        message: `Enter a value from ${String(validation.min)} to ${String(validation.max)}.`,
      });
    }
  }
  return errors;
}

function renderApplyResult(result: CommApplyResponse): string {
  const status = applyResultClass(result);
  const snippet = result.snippet
    ? `<pre class="config-snippet"><code>${escapeHtml(result.snippet)}</code></pre>`
    : "";
  return `<p class="apply-result ${escapeAttribute(status)}">${escapeHtml(result.message)}${
    result.config_path ? ` <code>${escapeHtml(result.config_path)}</code>` : ""
  }</p>${snippet}`;
}

function applyResultClass(result: CommApplyResponse): string {
  switch (result.lifecycle_effect) {
    case "blocked":
      return "error";
    case "restart_required":
    case "deploy_required":
    case "validate_only":
      return "pending";
    case "test_ok":
    case "applied_live":
      return "connected";
    default:
      return result.applied ? "connected" : "degraded";
  }
}

function renderField(field: CommFieldSchema, error: string | undefined): string {
  const value = field.default;
  const required = field.required ? "required" : "";
  const advanced = field.advanced ? " advanced" : "";
  const errorHtml = error ? `<span class="field-error">${escapeHtml(error)}</span>` : "";
  const example = exampleForField(field);
  return `<label class="field${advanced}" data-field="${escapeAttribute(field.id)}">
    <span>${escapeHtml(field.label)}${field.required ? " *" : ""}</span>
    ${renderInput(field, value, required)}
    <small>${escapeHtml(field.help)}${example ? ` <span class="field-example">Example: ${escapeHtml(example)}</span>` : ""}</small>
    ${errorHtml}
  </label>`;
}

function renderInput(
  field: CommFieldSchema,
  value: unknown,
  required: string
): string {
  const fieldId = escapeAttribute(field.id);
  const defaultValue = valueToInput(value);
  if (field.options && field.options.length > 0) {
    return `<select data-field-id="${fieldId}" data-field-type="${escapeAttribute(field.type)}" data-field-default="${escapeAttribute(JSON.stringify(value ?? ""))}" ${required}>
      ${field.options
        .map(
          (option) =>
            `<option value="${escapeAttribute(option)}" ${option === defaultValue ? "selected" : ""}>${escapeHtml(option)}</option>`
        )
        .join("")}
    </select>`;
  }
  if (field.type === "boolean") {
    return `<input data-field-id="${fieldId}" data-field-type="boolean" data-field-default="${escapeAttribute(JSON.stringify(value === true))}" type="checkbox" ${value === true ? "checked" : ""} />`;
  }
  if (field.type === "number") {
    return `<input data-field-id="${fieldId}" data-field-type="number" data-field-default="${escapeAttribute(JSON.stringify(value ?? null))}" type="number" value="${escapeAttribute(defaultValue)}" ${required} />`;
  }
  if (field.type === "json_array" || field.type === "json_object") {
    const fallback = field.type === "json_array" ? [] : {};
    return `<textarea data-field-id="${fieldId}" data-field-type="${escapeAttribute(field.type)}" data-field-default="${escapeAttribute(JSON.stringify(value ?? fallback))}" ${required}>${escapeHtml(
      JSON.stringify(value ?? (field.type === "json_array" ? [] : {}), null, 2)
    )}</textarea>`;
  }
  const inputType = field.secret ? "password" : "text";
  return `<input data-field-id="${fieldId}" data-field-type="${escapeAttribute(field.type)}" data-field-default="${escapeAttribute(JSON.stringify(value ?? ""))}" type="${inputType}" value="${escapeAttribute(defaultValue)}" ${required} />`;
}

function valueIsEmpty(value: unknown): boolean {
  return value === null || value === undefined || (typeof value === "string" && value.trim().length === 0);
}

function looksLikeEndpoint(value: string): boolean {
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    return false;
  }
  const lastColon = trimmed.lastIndexOf(":");
  if (lastColon <= 0 || lastColon === trimmed.length - 1) {
    return false;
  }
  const host = trimmed.slice(0, lastColon).trim();
  if (host.length === 0 || /\s/.test(host)) {
    return false;
  }
  const port = Number(trimmed.slice(lastColon + 1));
  return Number.isInteger(port) && port > 0 && port <= 65535;
}

function looksLikeSocketAddress(value: string): boolean {
  const trimmed = value.trim();
  const lastColon = trimmed.lastIndexOf(":");
  if (lastColon <= 0 || lastColon === trimmed.length - 1) {
    return false;
  }
  const host = trimmed.slice(0, lastColon);
  const port = Number(trimmed.slice(lastColon + 1));
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    return false;
  }
  if (host.startsWith("[") && host.endsWith("]")) {
    return host.length > 2;
  }
  return /^(\d{1,3}\.){3}\d{1,3}$/.test(host);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function valueToInput(value: unknown): string {
  if (value === null || value === undefined) {
    return "";
  }
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

function whatHappensNext(schema: CommProtocolSchema): string {
  const test = schema.supports_test
    ? "Test checks reachability before you apply. "
    : "Test is not required for this local or hardware-backed driver. ";
  switch (schema.lifecycle_effect) {
    case "restart_required":
      return `${test}Apply writes the runtime I/O config; restart the runtime for the driver to become live.`;
    case "deploy_required":
      return "This setup generates deployable configuration; apply it on the runtime host before expecting a live connection.";
    default:
      return `${test}Apply validates and writes the runtime-owned communication configuration.`;
  }
}

function exampleForField(field: CommFieldSchema): string {
  if (field.default === null || field.default === undefined) {
    return "";
  }
  if (field.type === "json_array" || field.type === "json_object") {
    return JSON.stringify(field.default);
  }
  if (typeof field.default === "string") {
    return field.default;
  }
  if (typeof field.default === "number" || typeof field.default === "boolean") {
    return String(field.default);
  }
  return JSON.stringify(field.default);
}

function escapeHtml(value: unknown): string {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeAttribute(value: unknown): string {
  return escapeHtml(value).replace(/'/g, "&#39;");
}
