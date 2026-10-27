// The communication catalog of `comm.schema` and the result of `comm.apply` (specification 11), as
// the canvas's forms render them. Converting, checking and sending a form's values is the server's
// (`trust-lsp.devicesCanvasAct`; specification 25 §5.9).

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
  visible_when?: CommFieldVisibilityRule;
}

export interface CommFieldVisibilityRule {
  field: string;
  equals?: unknown;
}

export interface CommConfiguredInstance {
  id: string;
  driver: string;
  display_name: string;
  enabled?: boolean;
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
  /** The canvas node that draws the saved connection, which the canvas selects. */
  node_id?: string;
  field_errors?: Array<{ field: string; message: string }>;
  snippet?: string;
}

/**
 * The fields shown while the user types: each field whose `visible_when` predicate holds for the
 * values typed so far. The form uses this only to show or hide fields; on submit the server
 * evaluates the predicates again and ignores the values of hidden fields (specification 25 §5.9).
 */
export function visibleSchemaFields(
  schema: CommProtocolSchema,
  values: Record<string, unknown>
): CommFieldSchema[] {
  return schema.fields.filter((field) => isSchemaFieldVisible(field, values));
}

export function isSchemaFieldVisible(
  field: CommFieldSchema,
  values: Record<string, unknown>
): boolean {
  const rule = field.visible_when;
  if (!rule) {
    return true;
  }
  if (!rule.field) {
    return true;
  }
  if (Object.prototype.hasOwnProperty.call(rule, "equals")) {
    return String(values[rule.field] ?? "") === String(rule.equals ?? "");
  }
  return true;
}
