import * as assert from "assert";

import {
  decodePlantContractError,
  decodePlantContractResponse,
} from "../../networkCanvas/plant/contract";
import {
  applyPlantContractDecode,
  type PlantViewState,
} from "../../networkCanvas/plant/state";

suite("Plant service contract decoder", () => {
  test("accepts a complete capabilities envelope without adding UI strings", () => {
    const input = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.capabilities",
      project_id: "project:sha256:abc",
      model_kinds: ["static_gain", "first_order"],
      limits: { max_trace_rows_per_plant: 100_000 },
      operations: ["plant.capabilities", "plant.read"],
    };

    const decoded = decodePlantContractResponse(input);

    assert.strictEqual(decoded.kind, "accepted");
    if (decoded.kind === "accepted") {
      assert.strictEqual(decoded.message, input);
      assert.strictEqual(decoded.message.operation, "plant.capabilities");
      assert.strictEqual(decoded.message["label"], undefined);
    }
  });

  // Specification 47 §20.1: the editor's answers are rendered as Rust gives them; the decoder only
  // admits their envelope.
  test("accepts the plant editor's answers and fails closed on a malformed one", () => {
    const common = {
      contract: "trust.plant",
      schema_version: 1,
      project_id: "project:sha256:abc",
    };
    const capabilities = {
      ...common,
      operation: "plant.capabilities",
      model_kinds: ["first_order"],
      limits: { max_trace_rows_per_plant: 100_000 },
      operations: ["plant.capabilities", "plant.edit", "plant.signals"],
      editor: { models: [] },
    };
    assert.strictEqual(decodePlantContractResponse(capabilities).kind, "accepted");
    assert.deepStrictEqual(
      decodePlantContractResponse({ ...capabilities, editor: [] }),
      { kind: "status_unavailable", reason: "malformed_payload" }
    );
    const edit = {
      ...common,
      operation: "plant.edit",
      draft: { plantId: "tank" },
      outcome: { kind: "applied" },
      issues: [],
      can_save: true,
      view: { model: { kind: "first_order" } },
      candidate: { id: "tank" },
    };
    assert.strictEqual(decodePlantContractResponse(edit).kind, "accepted");
    const { outcome: _outcome, ...withoutOutcome } = edit;
    assert.strictEqual(decodePlantContractResponse(withoutOutcome).kind, "accepted", "a plain check has no outcome");
    const { view: _view, ...withoutView } = edit;
    assert.deepStrictEqual(
      decodePlantContractResponse(withoutView),
      { kind: "status_unavailable", reason: "malformed_payload" }
    );
    assert.deepStrictEqual(
      decodePlantContractResponse({ ...edit, label: "extra" }),
      { kind: "status_unavailable", reason: "malformed_payload" }
    );
    const signals = { ...common, operation: "plant.signals", signals: { status: "ready", commands: [], feedbacks: [] } };
    assert.strictEqual(decodePlantContractResponse(signals).kind, "accepted");
    assert.deepStrictEqual(
      decodePlantContractResponse({ ...signals, signals: null }),
      { kind: "status_unavailable", reason: "malformed_payload" }
    );
  });

  test("fails closed for unsupported schemas and wrong project identities", () => {
    assert.deepStrictEqual(
      decodePlantContractResponse({
        contract: "trust.plant",
        schema_version: 2,
        operation: "plant.capabilities",
        project_id: "project:sha256:abc",
      }),
      { kind: "status_unavailable", reason: "unsupported_schema" }
    );
    assert.deepStrictEqual(
      decodePlantContractResponse(
        {
          contract: "trust.plant",
          schema_version: 1,
          operation: "plant.capabilities",
          project_id: "project:sha256:other",
          model_kinds: [],
          limits: { max_trace_rows_per_plant: 100_000 },
          operations: [],
        },
        "project:sha256:expected"
      ),
      { kind: "status_unavailable", reason: "wrong_project" }
    );
  });

  test("decodes read validate save and remove variants with revisions and descriptors", () => {
    const descriptor = {
      id: "tank",
      display_name: "Tank",
      description: null,
      model: "first_order",
      enabled: true,
      command_sources: ["%QW0"],
      feedback_targets: ["%IW0"],
      saved_order: 0,
    };
    const common = {
      contract: "trust.plant",
      schema_version: 1,
      project_id: "project:sha256:abc",
    } as const;
    const read = {
      ...common,
      operation: "plant.read",
      configuration_revision: "sha256:old",
      saved_configuration: { simulation: { enabled: true } },
      plants: [descriptor],
      restart_required: false,
    };
    const validate = {
      ...common,
      operation: "plant.validate",
      valid: true,
      issues: [],
      candidate_revision: "sha256:candidate",
    };
    const save = {
      ...common,
      operation: "plant.save",
      operation_id: "save-1",
      configuration_revision: "sha256:new",
      active_configuration_revision: "sha256:old",
      plants: [descriptor],
      affected_plant_ids: ["tank"],
      restart_required: true,
      plant_count: 1,
    };
    const remove = {
      ...common,
      operation: "plant.remove",
      operation_id: "remove-1",
      configuration_revision: "sha256:removed",
      active_configuration_revision: "sha256:old",
      plants: [],
      affected_plant_ids: ["tank"],
      restart_required: true,
      plant_count: 0,
    };

    for (const value of [read, validate, save, remove]) {
      const decoded = decodePlantContractResponse(value, common.project_id);
      assert.strictEqual(decoded.kind, "accepted", value.operation);
      if (decoded.kind === "accepted") assert.strictEqual(decoded.message, value);
    }
    assert.strictEqual(
      decodePlantContractResponse({ ...read, configuration_revision: "not-a-revision" }).kind,
      "status_unavailable"
    );
    assert.strictEqual(
      decodePlantContractResponse({ ...save, unknown: true }).kind,
      "status_unavailable"
    );
  });

  test("decodes exact active status and rejects availability contradictions", () => {
    const active = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.status",
      project_id: "project:sha256:abc",
      available: true,
      active_configuration_revision: "sha256:active",
      simulation_time_ns: "2000000",
      effective_seed: "42",
      random_algorithm: "chacha8",
      random_algorithm_version: 1,
      runtime_version: "0.24.64",
      time_scale: 1,
      restart_policy: "manual",
      plants: [
        {
          id: "tank",
          model: "first_order",
          operating_state: "running",
          enabled: true,
          step_ns: "1000000",
          simulation_time_ns: "2000000",
          state: [1.25],
          command: [2],
          applied_input: [1.8],
          ideal_output: [1.4],
          measured_feedback: [1.38],
          published_feedback: [1.38],
          actuators: [
            {
              position: 2.0,
              applied: 1.8,
              saturation_active: false,
              slew_limit_active: true,
              deadband_active: false,
              stiction_active: true,
              stuck: false,
            },
          ],
          active_uncertainties: ["gain"],
          active_dropouts: [],
          pending_event_count: 0,
          delay_buffer_depth: 2,
          accumulated_remainder_ns: "0",
          last_substeps: 1,
          current_fault: null,
        },
      ],
    };

    const decoded = decodePlantContractResponse(active, active.project_id);
    assert.strictEqual(decoded.kind, "accepted");
    if (decoded.kind === "accepted") assert.strictEqual(decoded.message, active);

    assert.deepStrictEqual(
      decodePlantContractResponse({ ...active, available: false }),
      { kind: "status_unavailable", reason: "malformed_payload" }
    );
    assert.strictEqual(
      decodePlantContractResponse({
        ...active,
        plants: [{ ...active.plants[0], measured_feedback: [Number.NaN] }],
      }).kind,
      "status_unavailable"
    );
  });

  test("decodes immutable trace snapshots and explicit clear results", () => {
    const common = {
      contract: "trust.plant",
      schema_version: 1,
      project_id: "project:sha256:abc",
      plant_id: "tank",
    } as const;
    const trace = {
      ...common,
      operation: "plant.trace.read",
      active_configuration_revision: "sha256:active",
      capacity: 100,
      retained_row_count: 1,
      dropped_row_count: 0,
      configuration_digest: "sha256:config",
      effective_seed: "42",
      random_algorithm: "chacha8",
      random_algorithm_version: 1,
      runtime_version: "0.24.64",
      restart_policy: "manual",
      rows: [
        {
          sequence: "7",
          plant_id: "tank",
          model: "first_order",
          simulation_time_ns: "10000000",
          state: [1],
          command: [2],
          applied_input: [1.8],
          ideal_output: [1.4],
          measured_feedback: [1.38],
          queued_feedback: [1.38],
          fired_events: ["bias"],
        },
      ],
    };
    const clear = {
      ...common,
      operation: "plant.trace.clear",
      operation_id: "clear-1",
      active_configuration_revision: "sha256:active",
      prior_row_count: 1,
      prior_dropped_row_count: 0,
    };

    for (const value of [trace, clear]) {
      const decoded = decodePlantContractResponse(value, common.project_id);
      assert.strictEqual(decoded.kind, "accepted", value.operation);
      if (decoded.kind === "accepted") assert.strictEqual(decoded.message, value);
    }
    assert.strictEqual(
      decodePlantContractResponse({ ...trace, rows: [{ ...trace.rows[0], sequence: "07" }] }).kind,
      "status_unavailable"
    );
    assert.strictEqual(
      decodePlantContractResponse({ ...clear, rows: [] }).kind,
      "status_unavailable"
    );
  });

  test("decodes every stable error code and structured recovery payload", () => {
    const base = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.validate",
      project_id: "project:sha256:abc",
    };
    const stableCodes = [
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
    ];
    for (const code of stableCodes) {
      const decoded = decodePlantContractError(
        { code, result: base },
        base.project_id
      );
      assert.strictEqual(decoded.kind, "accepted_error", code);
    }

    const revisionConflict = {
      ...base,
      operation: "plant.save",
      configuration_revision: "sha256:current",
      current_plant: null,
    };
    const statusUnavailable = {
      ...base,
      operation: "plant.status",
      requested_plant_id: "tank",
      available: false,
      reason_code: "runtime_evidence_unavailable",
    };
    const traceDisabled = {
      ...base,
      operation: "plant.trace.read",
      plant_id: "tank",
      trace_enabled: false,
    };
    const plantNotFound = {
      ...base,
      operation: "plant.trace.clear",
      plant_id: "tank",
    };
    for (const [code, result] of [
      ["revision_conflict", revisionConflict],
      ["status_unavailable", statusUnavailable],
      ["trace_disabled", traceDisabled],
      ["plant_not_found", plantNotFound],
    ] as const) {
      assert.strictEqual(
        decodePlantContractError({ code, result }, base.project_id).kind,
        "accepted_error",
        code
      );
    }

    assert.deepStrictEqual(
      decodePlantContractError(
        { code: "status_unavailable", result: { ...statusUnavailable, plants: [] } },
        base.project_id
      ),
      { kind: "status_unavailable", reason: "malformed_error" }
    );
    assert.deepStrictEqual(
      decodePlantContractError(
        { code: "io_failure", result: { ...base, project_id: "project:sha256:other" } },
        base.project_id
      ),
      { kind: "status_unavailable", reason: "wrong_project" }
    );
  });

  test("clears rejected live evidence without discarding saved state or edits", () => {
    const saved = {
      contract: "trust.plant",
      schema_version: 1,
      operation: "plant.read",
      project_id: "project:sha256:abc",
    } as const;
    const live = { ...saved, operation: "plant.status" };
    const trace = { ...saved, operation: "plant.trace.read" };
    const edits = { tank: { gain: 2 } };
    const state: PlantViewState = {
      confirmedSaved: saved,
      unsavedEdits: edits,
      liveStatus: live,
      trace,
      availability: "available",
    };

    const next = applyPlantContractDecode(state, {
      kind: "status_unavailable",
      reason: "malformed_payload",
    });

    assert.strictEqual(next.confirmedSaved, saved);
    assert.strictEqual(next.unsavedEdits, edits);
    assert.strictEqual(next.liveStatus, undefined);
    assert.strictEqual(next.trace, undefined);
    assert.strictEqual(next.availability, "unavailable");
    assert.strictEqual(next.unavailableReason, "malformed_payload");
  });
});
