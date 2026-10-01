import * as assert from "assert";
import type { ParameterBinding } from "../../controlStudio/parameterBinding";

const binding: ParameterBinding = { loop_id: "engineer-loop", task_name: "ControlTask",
  parameters: [{ symbol: "P1.Gain", iec_type: "REAL", unit: "N·m/rad", convention: "proportional gain", minimum: 0, maximum: 20 }],
  mode: { symbol: "P1.Enabled", manual_value: false } };

suite("Generic parameter application", () => {
  test("rolls back the exact retained values and confirms them after execution", async () => {
    const { ParameterApplicationClient } = require("../../controlStudio/parameterApplication");
    let rolledBack = false;
    let rollbackRequest: any;
    let rollbackTransactionId: string | null = null;
    const request = async (operation: string, params: any) => {
      if (operation === "control.parameters.rollback") { rollbackRequest = params; rollbackTransactionId = params.transaction_id; rolledBack = true; return { status: "pending", confirmed: false }; }
      return { supports_request_id: true, transaction_id: rolledBack ? rollbackTransactionId : null,
        program_revision: "program-1", binding_revision: "binding-1", active_revision: rolledBack ? "active-1" : "active-2",
        values: { "P1.Gain": rolledBack ? 1 : 2 }, status: "applied", previous_revision: rolledBack ? null : "active-1",
        previous_values: rolledBack ? null : { "P1.Gain": 1 }, reason: null };
    };
    const client = new ParameterApplicationClient(binding, "simulation", request);
    const review = await client.reviewRollback?.();
    assert.deepStrictEqual(review?.values, { "P1.Gain": 1 }, "rollback review must retain original runtime values");
    const result = await client.rollback(review, false);
    assert.deepStrictEqual(result.values, { "P1.Gain": 1 });
    assert.strictEqual(rollbackRequest.previous_revision, "active-1");
    assert.strictEqual(rollbackRequest.expected_active_revision, "active-2");
    assert.deepStrictEqual(rollbackRequest.expected_values, { "P1.Gain": 2 });
    assert.strictEqual(rollbackRequest.values, undefined);
  });

  test("reviews exact REAL values and confirms observed task-boundary adoption", async () => {
    let api: any;
    try { api = require("../../controlStudio/parameterApplication"); } catch { api = undefined; }
    const calls: { operation: string; params: any }[] = [];
    let applied = false;
    let applyTransactionId: string | null = null;
    const status = () => ({ program_revision: "program-1", binding_revision: "binding-1", active_revision: applied ? "active-2" : "active-1",
      supports_request_id: true, transaction_id: applied ? applyTransactionId : null,
      values: { "P1.Gain": applied ? Math.fround(0.1) : 1 }, status: applied ? "applied" : "idle",
      previous_revision: applied ? "active-1" : null, previous_values: applied ? { "P1.Gain": 1 } : null, reason: null });
    const request = async (operation: string, params: any) => {
      calls.push({ operation, params });
      if (operation === "control.parameters.apply") { applyTransactionId = params.transaction_id; applied = true; return { status: "pending", confirmed: false }; }
      return status();
    };
    const client = api ? new api.ParameterApplicationClient(binding, "simulation", request) : undefined;
    const review = await client?.review({ "P1.Gain": 0.1 });
    assert.deepStrictEqual(review?.values, { "P1.Gain": Math.fround(0.1) }, "review must show exact IEC REAL value");
    const result = await client.apply(review, false);
    assert.strictEqual(result.status, "applied");
    const sent = calls.find(call => call.operation === "control.parameters.apply")?.params;
    assert.deepStrictEqual(sent.expected_values, { "P1.Gain": 1 });
    assert.strictEqual(sent.expected_program_revision, "program-1");
    assert.strictEqual(sent.expected_binding_revision, "binding-1");
    assert.strictEqual(sent.expected_active_revision, "active-1");
    assert.strictEqual(calls[calls.length - 1].operation, "control.parameters.status");
  });
});


suite("Control Studio transfer boundary decoding", () => {
  const member = (symbol: string) => ({ symbol, iec_type: "LREAL" as const, unit: "1", minimum: -100, maximum: 100 });
  const transferBinding = { loop_id: "loop", task_name: "Task", mode: { symbol: "P1.Manual", manual_value: true },
    states: [member("P1.Ref"), member("P1.ManualCommand")], guards: [member("P1.Gain"), member("P1.PV")], output: member("P1.MV") };
  const values = { "P1.Manual": true, "P1.Ref": 0, "P1.ManualCommand": 0, "P1.Gain": 1, "P1.PV": 0, "P1.MV": 0 };
  const status = () => ({ program_revision: "program", binding_revision: "binding", status: "applied", reason: null,
    values: { ...values }, supported_tracking: ["proportional"], transaction: { id: "transfer", program_revision: "program",
      binding_revision: "binding", status: "applied", reason: null, observed_values: { ...values }, confirmation: "output-image",
      initial_values: { ...values }, applied_values: { "P1.Ref": 0, "P1.ManualCommand": 0 } } });
  test("accepts coherent native transfer evidence", async () => {
    const { RuntimeTransferClient } = require("../../controlStudio/runtimeTransfer");
    const value = status();
    const client = new RuntimeTransferClient(transferBinding, "program", async () => value);
    assert.deepStrictEqual(await client.status(), value);
  });
  test("accepts and confirms the further controllers' modes switched with the loop (Specification 35 §17.11)", async () => {
    const { RuntimeTransferClient } = require("../../controlStudio/runtimeTransfer");
    const binding = { ...transferBinding, additional_modes: [{ symbol: "P1.OuterManual", manual_value: true }] };
    const auto = { ...values, "P1.Manual": false, "P1.OuterManual": false };
    let applied = false, transaction = "";
    const request = async (operation: string, params: any) => {
      if (operation === "control.transfer.apply") { applied = true; transaction = params.transaction_id; return { status: "pending", confirmed: false }; }
      const now = applied ? auto : { ...values, "P1.OuterManual": true };
      return { program_revision: "program", binding_revision: "binding", status: applied ? "applied" : "idle", reason: null, values: { ...now },
        transaction: applied ? { id: transaction, program_revision: "program", binding_revision: "binding", status: "applied", reason: null,
          observed_values: { ...auto }, confirmation: "output-image", initial_values: { ...values, "P1.OuterManual": true },
          applied_values: { "P1.Ref": 0, "P1.ManualCommand": 0 } } : null };
    };
    const client = new RuntimeTransferClient(binding, "program", request);
    assert.strictEqual((await client.status()).values["P1.OuterManual"], true, "the outer controller's mode is part of the transfer status");
    const outcome = await client.apply({ "P1.Ref": 0, "P1.ManualCommand": 0 }, false, 0.1);
    assert.strictEqual(outcome.status, "applied");
  });

  test("rejects malformed optional tracking evidence before using it", async () => {
    const { RuntimeTransferClient } = require("../../controlStudio/runtimeTransfer");
    for (const mutate of [
      (value: any) => { value.transaction.initial_values["P1.Manual"] = 1; },
      (value: any) => { value.transaction.initial_values["P1.Gain"] = "1"; },
      (value: any) => { delete value.transaction.initial_values["P1.MV"]; },
      (value: any) => { value.transaction.applied_values["P1.Ref"] = Infinity; },
      (value: any) => { delete value.transaction.applied_values["P1.ManualCommand"]; },
      (value: any) => { value.transaction.applied_values["P1.Gain"] = 2; },
      (value: any) => { value.supported_tracking = "proportional"; },
    ]) {
      const value = status(); mutate(value);
      const client = new RuntimeTransferClient(transferBinding, "program", async () => value);
      await assert.rejects(() => client.status(), /invalid/, "Malformed optional transfer evidence must not be accepted as native data");
    }
  });
});
