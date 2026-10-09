import * as vscode from "vscode";
import { applyProjectEdit, keepEquipmentSettings, refusal, type ProjectEditAnswer } from "../hmiBuilder/editor/equipmentEdits";
import { openHmiTexts } from "../hmiBuilder/editor/textFiles";
import { decodePidLaw } from "./controllerLaw";
import type { ControlLoopDefinition } from "./loopDefinition";
import { setupProjectId } from "./loopSetup/loopSetupPanel";
import type { LoopMessageScope } from "./messages";
import { connectRuntimeParameters } from "./runtimeParameterConnection";

/** The record Keep in project writes and its numeric fields the loop binds. */
export interface KeepableRecord { record: string; fields: string[] }

/** What Keep in project wrote: the fields whose values changed in the project, and those the equipment has no setting for. */
export interface KeptParameters { kept: string[]; unknown: string[] }

type KeepAnswer = ProjectEditAnswer & { kept?: string[]; generated?: { instance: string; settings: Record<string, number> } };

/** The loop's parameter record when it has one, such as the record Control.PID is configured from. */
export function keepableRecord(loop: ControlLoopDefinition): KeepableRecord | undefined {
  const law = decodePidLaw(loop.controller.law, loop.parameterBinding);
  if (!law || !loop.parameterBinding) return undefined;
  const prefix = `${law.parameterRecordSymbol}.`;
  const fields = loop.parameterBinding.parameters.map(parameter => parameter.symbol)
    .filter(symbol => symbol.startsWith(prefix) && !symbol.slice(prefix.length).includes("."))
    .map(symbol => symbol.slice(prefix.length));
  return fields.length ? { record: law.parameterRecordSymbol, fields } : undefined;
}

/** The workspace folder of a loop's project: the one whose setup scope the loop carries, else the first. */
export function loopProjectFolder(loop: ControlLoopDefinition): vscode.Uri | undefined {
  const folders = vscode.workspace.workspaceFolders ?? [];
  return (folders.find(folder => setupProjectId(folder.uri) === loop.projectId) ?? folders[0])?.uri;
}

/**
 * Keep in project (Specification 35 §17.17): the values the record holds in the running PLC become its initial values in the
 * project, so a restart or a new download keeps the applied tuning. A record written by hand is edited in its declaration; one
 * Create PLC code declared is kept through its equipment's settings, so the generated declarations stay the generator's own.
 */
export async function keepAppliedParameters(loop: ControlLoopDefinition, running: Readonly<Record<string, number>>,
  folder: vscode.Uri): Promise<KeptParameters> {
  const keepable = keepableRecord(loop);
  if (!keepable) throw new Error("Keep in project writes a parameter record, such as the one Control.PID is configured from; this loop binds none.");
  const values = Object.fromEntries(keepable.fields.flatMap(field => {
    const value = running[`${keepable.record}.${field}`];
    return typeof value === "number" && Number.isFinite(value) ? [[field, value]] : [];
  }));
  if (!Object.keys(values).length) throw new Error(`The runtime reports no values of ${keepable.record}.`);
  const answer = await vscode.commands.executeCommand<KeepAnswer>("trust-lsp.controlKeepParameters", {
    root_uri: folder.toString(), open: openHmiTexts(folder), record: keepable.record, values });
  if (answer?.generated) {
    const field = (member: string) => member.slice(member.lastIndexOf(".") + 1);
    const result = await keepEquipmentSettings(folder, answer.generated.instance, answer.generated.settings);
    return { kept: result.kept.map(field), unknown: result.unknown.map(field) };
  }
  if (!answer?.ok) throw new Error(refusal(answer, `${keepable.record} could not be kept in the project.`));
  await applyProjectEdit(answer);
  return { kept: answer.kept ?? Object.keys(values), unknown: [] };
}

/** The words that report a keep. */
export function keptReason(record: string, result: KeptParameters): string {
  const skipped = result.unknown.length ? ` The equipment has no setting for ${result.unknown.join(", ")}; copy it into the PLC code by hand.` : "";
  return result.kept.length ? `Kept in the project: ${result.kept.join(", ")} of ${record}. A restart or a new download keeps them.${skipped}`
    : `The project already has these values of ${record}.${skipped}`;
}

/** A webview request to keep the applied values, in the loop scope of the open study. */
export function decodeKeepCommand(value: unknown, expected: LoopMessageScope): string | undefined {
  if (!record(value) || value.kind !== "control-studio.application.keep" || !record(value.payload) || Object.keys(value.payload).length !== 0
    || typeof value.messageId !== "string" || !value.messageId || value.projectId !== expected.projectId
    || value.editorSessionId !== expected.editorSessionId || value.loopId !== expected.loopId
    || value.expectedLoopRevision !== expected.loopRevision) return undefined;
  return value.messageId;
}

export interface KeepInProjectPorts {
  runningValues(loop: ControlLoopDefinition): Promise<Readonly<Record<string, number>>>;
  folder(loop: ControlLoopDefinition): vscode.Uri | undefined;
  keep(loop: ControlLoopDefinition, running: Readonly<Record<string, number>>, folder: vscode.Uri): Promise<KeptParameters>;
}

const runtimePorts: KeepInProjectPorts = {
  runningValues: async loop => (await (await connectRuntimeParameters(loop)).client.status()).values,
  folder: loopProjectFolder,
  keep: keepAppliedParameters,
};

/** Owns one Keep in project at a time for an open study; the answer is posted as an engineer update. */
export class KeepInProjectCoordinator {
  private busy = false;
  constructor(private readonly ports: KeepInProjectPorts = runtimePorts) {}

  async handle(value: unknown, scope: LoopMessageScope, loop: ControlLoopDefinition | undefined,
    post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!decodeKeepCommand(value, scope)) return false;
    const answer = (status: "kept" | "keep-failed", reason: string, result?: KeptParameters) =>
      post({ status, applicationAction: "keep", reason, ...(result ? { kept: result.kept, unknown: result.unknown } : {}) });
    if (this.busy) { await answer("keep-failed", "Keep in project is already writing the project."); return true; }
    const keepable = loop ? keepableRecord(loop) : undefined;
    const folder = loop ? this.ports.folder(loop) : undefined;
    if (!loop || !keepable || !folder) {
      await answer("keep-failed", !folder ? "Open the project folder to keep the values in it."
        : "Keep in project writes a parameter record, such as the one Control.PID is configured from; this loop binds none.");
      return true;
    }
    this.busy = true;
    try {
      const result = await this.ports.keep(loop, await this.ports.runningValues(loop), folder);
      await answer("kept", keptReason(keepable.record, result), result);
    } catch (error) {
      await answer("keep-failed", `Not kept: ${error instanceof Error ? error.message : String(error)}`);
    } finally { this.busy = false; }
    return true;
  }
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
