export interface PlantContractMessage {
  contract: "trust.plant";
  schema_version: 1;
  operation: string;
  project_id: string;
  [key: string]: unknown;
}

export type PlantContractDecodeResult =
  | { kind: "accepted"; message: PlantContractMessage }
  | {
      kind: "status_unavailable";
      reason:
        | "unsupported_schema"
        | "malformed_payload"
        | "wrong_project"
        | "unsupported_variant";
    };

export type PlantContractErrorDecodeResult =
  | { kind: "accepted_error"; code: string; message: PlantContractMessage }
  | { kind: "status_unavailable"; reason: "malformed_error" | "wrong_project" };

const CAPABILITY_FIELDS = new Set([
  "contract",
  "schema_version",
  "operation",
  "project_id",
  "model_kinds",
  "limits",
  "operations",
]);

const READ_FIELDS = new Set([
  "contract",
  "schema_version",
  "operation",
  "project_id",
  "configuration_revision",
  "active_configuration_revision",
  "saved_configuration",
  "plants",
  "restart_required",
]);

const VALIDATE_FIELDS = new Set([
  "contract",
  "schema_version",
  "operation",
  "project_id",
  "valid",
  "issues",
  "candidate_revision",
]);

const MUTATION_FIELDS = new Set([
  "contract",
  "schema_version",
  "operation",
  "operation_id",
  "project_id",
  "configuration_revision",
  "active_configuration_revision",
  "plants",
  "affected_plant_ids",
  "restart_required",
  "plant_count",
]);

const DESCRIPTOR_FIELDS = new Set([
  "id",
  "display_name",
  "description",
  "model",
  "enabled",
  "command_sources",
  "feedback_targets",
  "saved_order",
]);

const ISSUE_FIELDS = new Set(["code", "severity", "path", "plant_id"]);

const STATUS_FIELDS = new Set([
  "contract",
  "schema_version",
  "operation",
  "project_id",
  "available",
  "active_configuration_revision",
  "simulation_time_ns",
  "effective_seed",
  "random_algorithm",
  "random_algorithm_version",
  "runtime_version",
  "time_scale",
  "restart_policy",
  "plants",
]);

const STATUS_PLANT_FIELDS = new Set([
  "id",
  "model",
  "operating_state",
  "enabled",
  "step_ns",
  "simulation_time_ns",
  "state",
  "command",
  "applied_input",
  "ideal_output",
  "measured_feedback",
  "published_feedback",
  "actuators",
  "active_uncertainties",
  "active_dropouts",
  "pending_event_count",
  "delay_buffer_depth",
  "accumulated_remainder_ns",
  "last_substeps",
  "current_fault",
]);

const ACTUATOR_FIELDS = new Set([
  "applied",
  "saturation_active",
  "slew_limit_active",
  "deadband_active",
  "stuck",
]);

const TRACE_FIELDS = new Set([
  "contract",
  "schema_version",
  "operation",
  "project_id",
  "plant_id",
  "active_configuration_revision",
  "capacity",
  "retained_row_count",
  "dropped_row_count",
  "configuration_digest",
  "effective_seed",
  "random_algorithm",
  "random_algorithm_version",
  "runtime_version",
  "restart_policy",
  "rows",
]);

const TRACE_ROW_FIELDS = new Set([
  "sequence",
  "plant_id",
  "model",
  "simulation_time_ns",
  "state",
  "command",
  "applied_input",
  "ideal_output",
  "measured_feedback",
  "queued_feedback",
  "fired_events",
]);

const TRACE_CLEAR_FIELDS = new Set([
  "contract",
  "schema_version",
  "operation",
  "project_id",
  "operation_id",
  "plant_id",
  "active_configuration_revision",
  "prior_row_count",
  "prior_dropped_row_count",
]);

function record(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function stringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((entry) => typeof entry === "string");
}

function hasOnlyFields(value: Record<string, unknown>, fields: ReadonlySet<string>): boolean {
  return Object.keys(value).every((key) => fields.has(key));
}

function nonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

function revision(value: unknown): value is string {
  return nonEmptyString(value) && value.startsWith("sha256:") && value.length > "sha256:".length;
}

function optionalRevision(value: unknown): boolean {
  return value === undefined || revision(value);
}

function finiteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function nonNegativeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function unsignedDecimal(value: unknown, maximum: bigint): value is string {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]*)$/.test(value)) return false;
  try {
    return BigInt(value) <= maximum;
  } catch {
    return false;
  }
}

function numericVector(value: unknown): boolean {
  return Array.isArray(value) && value.every(finiteNumber);
}

function actuatorStatus(value: unknown): boolean {
  const actuator = record(value);
  return Boolean(
    actuator &&
      hasOnlyFields(actuator, ACTUATOR_FIELDS) &&
      Object.keys(actuator).length === ACTUATOR_FIELDS.size &&
      finiteNumber(actuator.applied) &&
      typeof actuator.saturation_active === "boolean" &&
      typeof actuator.slew_limit_active === "boolean" &&
      typeof actuator.deadband_active === "boolean" &&
      typeof actuator.stuck === "boolean"
  );
}

function plantStatus(value: unknown): boolean {
  const status = record(value);
  return Boolean(
    status &&
      hasOnlyFields(status, STATUS_PLANT_FIELDS) &&
      Object.keys(status).length === STATUS_PLANT_FIELDS.size &&
      nonEmptyString(status.id) &&
      nonEmptyString(status.model) &&
      (status.operating_state === "running" || status.operating_state === "faulted") &&
      typeof status.enabled === "boolean" &&
      unsignedDecimal(status.step_ns, (1n << 128n) - 1n) &&
      unsignedDecimal(status.simulation_time_ns, (1n << 128n) - 1n) &&
      numericVector(status.state) &&
      numericVector(status.command) &&
      numericVector(status.applied_input) &&
      numericVector(status.ideal_output) &&
      numericVector(status.measured_feedback) &&
      numericVector(status.published_feedback) &&
      Array.isArray(status.actuators) &&
      status.actuators.every(actuatorStatus) &&
      stringArray(status.active_uncertainties) &&
      stringArray(status.active_dropouts) &&
      nonNegativeInteger(status.pending_event_count) &&
      nonNegativeInteger(status.delay_buffer_depth) &&
      unsignedDecimal(status.accumulated_remainder_ns, (1n << 128n) - 1n) &&
      nonNegativeInteger(status.last_substeps) &&
      (status.current_fault === null || typeof status.current_fault === "string")
  );
}

function activeStatusResponse(message: Record<string, unknown>): boolean {
  return Boolean(
    hasOnlyFields(message, STATUS_FIELDS) &&
      Object.keys(message).length === STATUS_FIELDS.size &&
      message.available === true &&
      revision(message.active_configuration_revision) &&
      unsignedDecimal(message.simulation_time_ns, (1n << 128n) - 1n) &&
      unsignedDecimal(message.effective_seed, (1n << 64n) - 1n) &&
      nonEmptyString(message.random_algorithm) &&
      nonNegativeInteger(message.random_algorithm_version) &&
      nonEmptyString(message.runtime_version) &&
      finiteNumber(message.time_scale) &&
      message.time_scale > 0 &&
      nonEmptyString(message.restart_policy) &&
      Array.isArray(message.plants) &&
      message.plants.every(plantStatus)
  );
}

function traceRow(value: unknown, plantId: string): value is Record<string, unknown> {
  const row = record(value);
  return Boolean(
    row &&
      hasOnlyFields(row, TRACE_ROW_FIELDS) &&
      Object.keys(row).length === TRACE_ROW_FIELDS.size &&
      unsignedDecimal(row.sequence, (1n << 64n) - 1n) &&
      row.plant_id === plantId &&
      nonEmptyString(row.model) &&
      unsignedDecimal(row.simulation_time_ns, (1n << 128n) - 1n) &&
      numericVector(row.state) &&
      numericVector(row.command) &&
      numericVector(row.applied_input) &&
      numericVector(row.ideal_output) &&
      numericVector(row.measured_feedback) &&
      numericVector(row.queued_feedback) &&
      stringArray(row.fired_events)
  );
}

function traceResponse(message: Record<string, unknown>): boolean {
  if (
    !hasOnlyFields(message, TRACE_FIELDS) ||
    Object.keys(message).length !== TRACE_FIELDS.size ||
    !nonEmptyString(message.plant_id) ||
    !revision(message.active_configuration_revision) ||
    !nonNegativeInteger(message.capacity) ||
    !nonNegativeInteger(message.retained_row_count) ||
    !nonNegativeInteger(message.dropped_row_count) ||
    !revision(message.configuration_digest) ||
    !unsignedDecimal(message.effective_seed, (1n << 64n) - 1n) ||
    !nonEmptyString(message.random_algorithm) ||
    !nonNegativeInteger(message.random_algorithm_version) ||
    !nonEmptyString(message.runtime_version) ||
    !nonEmptyString(message.restart_policy) ||
    !Array.isArray(message.rows) ||
    message.rows.length !== message.retained_row_count ||
    message.retained_row_count > message.capacity ||
    !message.rows.every((row) => traceRow(row, message.plant_id as string))
  ) {
    return false;
  }
  for (let index = 1; index < message.rows.length; index += 1) {
    const prior = message.rows[index - 1] as Record<string, string>;
    const current = message.rows[index] as Record<string, string>;
    if (
      BigInt(current.sequence) <= BigInt(prior.sequence) ||
      BigInt(current.simulation_time_ns) < BigInt(prior.simulation_time_ns)
    ) {
      return false;
    }
  }
  return true;
}

function traceClearResponse(message: Record<string, unknown>): boolean {
  return Boolean(
    hasOnlyFields(message, TRACE_CLEAR_FIELDS) &&
      Object.keys(message).length === TRACE_CLEAR_FIELDS.size &&
      nonEmptyString(message.operation_id) &&
      nonEmptyString(message.plant_id) &&
      revision(message.active_configuration_revision) &&
      nonNegativeInteger(message.prior_row_count) &&
      nonNegativeInteger(message.prior_dropped_row_count)
  );
}

function plantDescriptor(value: unknown): boolean {
  const descriptor = record(value);
  return Boolean(
    descriptor &&
      hasOnlyFields(descriptor, DESCRIPTOR_FIELDS) &&
      Object.keys(descriptor).length === DESCRIPTOR_FIELDS.size &&
      nonEmptyString(descriptor.id) &&
      nonEmptyString(descriptor.display_name) &&
      (descriptor.description === null || typeof descriptor.description === "string") &&
      nonEmptyString(descriptor.model) &&
      typeof descriptor.enabled === "boolean" &&
      stringArray(descriptor.command_sources) &&
      stringArray(descriptor.feedback_targets) &&
      Number.isSafeInteger(descriptor.saved_order) &&
      (descriptor.saved_order as number) >= 0
  );
}

function descriptorArray(value: unknown): boolean {
  return Array.isArray(value) && value.every(plantDescriptor);
}

function validationIssue(value: unknown): boolean {
  const issue = record(value);
  return Boolean(
    issue &&
      hasOnlyFields(issue, ISSUE_FIELDS) &&
      nonEmptyString(issue.code) &&
      nonEmptyString(issue.severity) &&
      nonEmptyString(issue.path) &&
      (issue.plant_id === undefined || nonEmptyString(issue.plant_id))
  );
}

function configurationResponse(message: Record<string, unknown>): boolean {
  switch (message.operation) {
    case "plant.read":
      return Boolean(
        hasOnlyFields(message, READ_FIELDS) &&
          revision(message.configuration_revision) &&
          optionalRevision(message.active_configuration_revision) &&
          record(message.saved_configuration) &&
          descriptorArray(message.plants) &&
          typeof message.restart_required === "boolean"
      );
    case "plant.validate":
      return Boolean(
        hasOnlyFields(message, VALIDATE_FIELDS) &&
          typeof message.valid === "boolean" &&
          Array.isArray(message.issues) &&
          message.issues.every(validationIssue) &&
          revision(message.candidate_revision)
      );
    case "plant.save":
    case "plant.remove":
      return Boolean(
        hasOnlyFields(message, MUTATION_FIELDS) &&
          nonEmptyString(message.operation_id) &&
          revision(message.configuration_revision) &&
          optionalRevision(message.active_configuration_revision) &&
          descriptorArray(message.plants) &&
          stringArray(message.affected_plant_ids) &&
          message.affected_plant_ids.every(nonEmptyString) &&
          typeof message.restart_required === "boolean" &&
          Number.isSafeInteger(message.plant_count) &&
          (message.plant_count as number) >= 0 &&
          message.plant_count === (message.plants as unknown[]).length
      );
    default:
      return false;
  }
}

/** Decodes untrusted runtime/CLI data before it can replace confirmed canvas state. */
export function decodePlantContractResponse(
  value: unknown,
  expectedProjectId?: string
): PlantContractDecodeResult {
  const message = record(value);
  if (!message) return { kind: "status_unavailable", reason: "malformed_payload" };
  if (message.schema_version !== 1) {
    return { kind: "status_unavailable", reason: "unsupported_schema" };
  }
  if (
    message.contract !== "trust.plant" ||
    typeof message.operation !== "string" ||
    typeof message.project_id !== "string" ||
    !message.project_id.startsWith("project:sha256:")
  ) {
    return { kind: "status_unavailable", reason: "malformed_payload" };
  }
  if (expectedProjectId && message.project_id !== expectedProjectId) {
    return { kind: "status_unavailable", reason: "wrong_project" };
  }
  if (message.operation === "plant.capabilities") {
    const limits = record(message.limits);
    if (
      !hasOnlyFields(message, CAPABILITY_FIELDS) ||
      !stringArray(message.model_kinds) ||
      !stringArray(message.operations) ||
      !limits ||
      Object.keys(limits).some((key) => key !== "max_trace_rows_per_plant") ||
      !Number.isSafeInteger(limits.max_trace_rows_per_plant) ||
      (limits.max_trace_rows_per_plant as number) < 0
    ) {
      return { kind: "status_unavailable", reason: "malformed_payload" };
    }
    return { kind: "accepted", message: message as unknown as PlantContractMessage };
  }
  if (message.operation === "plant.status") {
    return activeStatusResponse(message)
      ? { kind: "accepted", message: message as unknown as PlantContractMessage }
      : { kind: "status_unavailable", reason: "malformed_payload" };
  }
  if (message.operation === "plant.trace.read") {
    return traceResponse(message)
      ? { kind: "accepted", message: message as unknown as PlantContractMessage }
      : { kind: "status_unavailable", reason: "malformed_payload" };
  }
  if (message.operation === "plant.trace.clear") {
    return traceClearResponse(message)
      ? { kind: "accepted", message: message as unknown as PlantContractMessage }
      : { kind: "status_unavailable", reason: "malformed_payload" };
  }
  if (!["plant.read", "plant.validate", "plant.save", "plant.remove"].includes(message.operation)) {
    return { kind: "status_unavailable", reason: "unsupported_variant" };
  }
  if (!configurationResponse(message)) {
    return { kind: "status_unavailable", reason: "malformed_payload" };
  }
  return { kind: "accepted", message: message as unknown as PlantContractMessage };
}

/** Decodes a rejected control response without treating technical detail as UI copy. */
export function decodePlantContractError(
  value: unknown,
  expectedProjectId?: string
): PlantContractErrorDecodeResult {
  const error = record(value);
  const result = record(error?.result);
  if (!error || !result || typeof error.code !== "string") {
    return { kind: "status_unavailable", reason: "malformed_error" };
  }
  if (expectedProjectId && result.project_id !== expectedProjectId) {
    return { kind: "status_unavailable", reason: "wrong_project" };
  }
  if (
    result.contract !== "trust.plant" ||
    result.schema_version !== 1 ||
    !nonEmptyString(result.operation) ||
    !nonEmptyString(result.project_id) ||
    !result.project_id.startsWith("project:sha256:")
  ) {
    return { kind: "status_unavailable", reason: "malformed_error" };
  }

  const common = new Set(["contract", "schema_version", "operation", "project_id"]);
  const genericCodes = new Set([
    "unsupported_schema",
    "unsupported_operation",
    "malformed_request",
    "missing_auth_token",
    "invalid_auth_token",
    "insufficient_role",
    "validation_failed",
    "idempotency_conflict",
    "runtime_conflict",
    "io_failure",
  ]);
  let valid = genericCodes.has(error.code) && hasOnlyFields(result, common);

  if (error.code === "revision_conflict" && result.operation === "plant.save") {
    const fields = new Set([...common, "configuration_revision", "current_plant"]);
    valid =
      hasOnlyFields(result, fields) &&
      Object.keys(result).length === fields.size &&
      revision(result.configuration_revision) &&
      (result.current_plant === null || plantDescriptor(result.current_plant));
  } else if (
    ["revision_conflict", "plant_not_found", "status_unavailable"].includes(error.code) &&
    result.operation === "plant.trace.clear"
  ) {
    const fields = new Set([...common, "plant_id"]);
    valid =
      hasOnlyFields(result, fields) &&
      Object.keys(result).length === fields.size &&
      nonEmptyString(result.plant_id);
  } else if (error.code === "status_unavailable" && result.operation === "plant.status") {
    const fields = new Set([
      ...common,
      "requested_plant_id",
      "available",
      "reason_code",
    ]);
    valid =
      hasOnlyFields(result, fields) &&
      Object.keys(result).length === fields.size &&
      (result.requested_plant_id === null || nonEmptyString(result.requested_plant_id)) &&
      result.available === false &&
      nonEmptyString(result.reason_code);
  } else if (error.code === "status_unavailable" && result.operation === "plant.trace.read") {
    const fields = new Set([...common, "plant_id", "available", "reason_code"]);
    valid =
      hasOnlyFields(result, fields) &&
      Object.keys(result).length === fields.size &&
      nonEmptyString(result.plant_id) &&
      result.available === false &&
      nonEmptyString(result.reason_code);
  } else if (error.code === "trace_disabled" && result.operation === "plant.trace.read") {
    const fields = new Set([...common, "plant_id", "trace_enabled"]);
    valid =
      hasOnlyFields(result, fields) &&
      Object.keys(result).length === fields.size &&
      nonEmptyString(result.plant_id) &&
      result.trace_enabled === false;
  } else if (error.code === "plant_not_found") {
    valid = false;
  }

  return valid
    ? {
        kind: "accepted_error",
        code: error.code,
        message: result as unknown as PlantContractMessage,
      }
    : { kind: "status_unavailable", reason: "malformed_error" };
}
