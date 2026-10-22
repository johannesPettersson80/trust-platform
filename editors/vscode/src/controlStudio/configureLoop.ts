import * as vscode from "vscode";
import { getBinaryPath } from "../binary";
import { createEngineClient } from "./engine/engineClient";
import { controlStudioEngine } from "./engine/enginePanelRoute";
import { refusalText, saveSetupDraft, setupStep } from "./loopSetup/setupEngine";
import { nativeSetupPrompts } from "./nativeSetupPrompts";
import type { SetupPromptPresenter } from "./setupPromptTypes";
import type { SetupStepRequest } from "./generated/SetupStepRequest";
import type { SetupStepResult } from "./generated/SetupStepResult";
import type { ConfiguredLoopRequest } from "./generated/ProjectLoopsResult";

/** The step-by-step setup's result: the request that opens the loop the engine saved, the cancel, or the engine's refusal. */
export type ConfigureResolution =
  | { kind: "accepted"; request: ConfiguredLoopRequest }
  | { kind: "cancelled" }
  | { kind: "missing"; reason: string };

/**
 * The step-by-step setup (B1.3, C12). The engine names each question, its choices and its checks, in its own order; the presenter shows
 * each one and returns the answer; the engine answers the next question, or the finished draft and the sentence the engineer confirms
 * before the save. The engine builds and writes the loop, or refuses it with its sentence. This module keeps no question and no rule.
 */
export async function configureControlLoop(
  context: vscode.ExtensionContext,
  runtimeId: string,
  presenter: SetupPromptPresenter = nativeSetupPrompts
): Promise<ConfigureResolution> {
  try {
    const folders = vscode.workspace.workspaceFolders ?? [];
    const folder = folders.length === 1 ? folders[0] : (await vscode.window.showQuickPick(
      folders.map(item => ({ label: item.name, description: item.uri.fsPath, folder: item })),
      { title: "Configure control loop", placeHolder: "Choose the project containing your controller" }
    ))?.folder;
    if (!folder) return { kind: "cancelled" };
    const engine = controlStudioEngine(context);
    const base = { projectRoot: folder.uri.fsPath, runtimeId, languageTool: getBinaryPath(context, "trust-lsp", "server.path"), back: false };
    let reply = await setupStep(engine, base);
    for (;;) {
      if ("failure" in reply) return { kind: "missing", reason: refusalText(reply.failure) };
      const result: SetupStepResult = reply.result;
      if (!result.question) return await confirmAndSave(context, base, result);
      const given = await presenter.ask(result.question, {
        previous: result.previous ?? undefined, refusal: result.refusal ?? undefined, canGoBack: result.canGoBack,
      });
      if (given.kind === "cancelled") return { kind: "cancelled" };
      // An answer is any JSON value: the text typed, the value of the item chosen, or the text of the file (`AnswerValue.value`).
      const request: SetupStepRequest = given.kind === "back"
        ? { ...base, session: result.session, back: true, ...(given.text === undefined ? {} : { answer: { value: given.text } }) }
        : { ...base, session: result.session, answer: { value: given.answer } };
      reply = await setupStep(engine, request);
    }
  } catch (error) {
    return { kind: "missing", reason: error instanceof Error ? error.message : "Loop setup failed. No runtime values were changed." };
  }
}

/** The finished setup: the engineer confirms the sentence the engine gives, and the engine saves the draft (B1.3). */
async function confirmAndSave(context: vscode.ExtensionContext, base: { projectRoot: string; runtimeId: string; languageTool?: string },
  result: SetupStepResult): Promise<ConfigureResolution> {
  if (!result.draft || !result.summary) return { kind: "missing", reason: result.refusal ?? "The engine has no loop to save." };
  const action = await vscode.window.showInformationMessage(result.summary, { modal: true }, "Save loop");
  if (action !== "Save loop") return { kind: "cancelled" };
  const outcome = await saveSetupDraft(createEngineClient(controlStudioEngine(context)), {
    projectRoot: base.projectRoot, runtimeId: base.runtimeId, languageTool: base.languageTool, draft: result.draft,
  });
  if (outcome.kind === "refused") return { kind: "missing", reason: refusalText(outcome.failure) };
  if (!outcome.openRequest) return { kind: "missing", reason: "The engine named no loop to open." };
  return { kind: "accepted", request: outcome.openRequest };
}
