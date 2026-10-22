import { randomUUID } from "crypto";
import type { ConfiguredLoopRequest, LoopDefinition } from "./generated/ProjectLoopsResult";
import type { LoopMessageScope } from "./messages";
import type { ControlStudioEngine } from "./engine/engineClient";
import type { SetupDiscoverRequest } from "./generated/SetupDiscoverRequest";
import { engineAnswer } from "./loopSetup/setupEngine";

/** A binding selection that opens a new loop session: its session identifier, and the controller family it chose (B1.3). */
export interface StudioBindingOpen {
  sessionId: string;
  /** The controller family the engineer chose; the engine checks it against the selected loop's adapter (C0). */
  family?: string;
}

/** The engine that discovers and selects the loops of the panel's project (B1.2, B1.3), and the project folder it answers for. */
export interface EngineBindingPort {
  readonly engine: ControlStudioEngine;
  readonly projectRoot: string;
  /** The language tool that reads the compiled project, when the extension names it (§0.1a). */
  readonly languageTool?: string;
}

/** How many ids of handled messages the route keeps to answer a repeated message once; the oldest is forgotten first. */
const HANDLED_MESSAGES_KEPT = 4096;

/**
 * The binding page of the loop's project, on the engine (B1.2, B1.3): the engine discovers the loops of the project and selects the
 * binding; the page shows its answer and its refusals. The route holds no rule of its own.
 */
export class ThreePageBindingRoute {
  /** The ids of the messages already handled, newest last: a message the page sends twice is answered once. Only the last ones are kept. */
  private readonly handled = new Set<string>();
  private selecting = false;
  private family: string | undefined;
  constructor(private readonly loop: LoopDefinition, private readonly scope: LoopMessageScope,
    private readonly current: () => boolean,
    private readonly open: (request: ConfiguredLoopRequest, sessionId: string, family?: string) => Promise<void>,
    private readonly port: EngineBindingPort) {}

  /**
   * Lists the project's loops (B1.3). With the page's mapping, the engine also answers which loop that mapping matches and whether its
   * signals changed (B3.6); the route forwards the mapping as the page sent it and the engine's answer as it came. The request names the
   * loop the panel shows, so that a listing without a mapping is answered with the choices of that loop's own mapping, the other loops
   * that bind the same plant signals (B1.2, B3.6).
   */
  async publish(post: (payload: Record<string, unknown>) => Promise<void>, mapping?: SetupDiscoverRequest["mapping"]): Promise<void> {
    if (!this.current()) return;
    const answer = await engineAnswer(() => this.port.engine.setupDiscover({ projectRoot: this.port.projectRoot, runtimeId: this.loop.runtimeId,
      loopId: this.loop.id, ...(this.port.languageTool ? { languageTool: this.port.languageTool } : {}), ...(mapping ? { mapping } : {}) }));
    if (!this.current()) return;
    // The page shows the engine's whole sentence (A11.1); the host adds no words of its own to it.
    if ("failure" in answer) await post({ status: "failed", reason: answer.failure.detail || answer.failure.message });
    else await post({ status: "bindings", targets: answer.result.loops, rejected: answer.result.rejected,
      ...(answer.result.mapping ? { mapping: answer.result.mapping } : {}) });
  }

  async handle(value: unknown, post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!record(value) || !["control-studio.binding.list", "control-studio.binding.select"].includes(String(value.kind))) return false;
    if (value.schemaVersion !== 1 || value.projectId !== this.scope.projectId || value.editorSessionId !== this.scope.editorSessionId
      || value.loopId !== this.scope.loopId || value.expectedLoopRevision !== this.scope.loopRevision || !record(value.payload)
      || typeof value.messageId !== "string" || !value.messageId || !this.current()) return true;
    if (this.handled.has(value.messageId) || this.selecting) return true;
    this.handled.add(value.messageId);
    if (this.handled.size > HANDLED_MESSAGES_KEPT) this.handled.delete(this.handled.values().next().value as string);
    if (value.kind === "control-studio.binding.list") {
      // The page's JSON: the engine decodes and checks the mapping, so its shape is the engine's, not the page's to restate.
      await this.publish(post, record(value.payload.mapping) ? value.payload.mapping as unknown as SetupDiscoverRequest["mapping"] : undefined);
      return true;
    }
    const payload = value.payload;
    try {
      await this.selectWithEngine(payload, post);
    } catch (error) {
      if (this.current()) await post({ status: "failed", page: payload.page, reason: reason(error) });
    } finally { this.selecting = false; }
    return true;
  }

  /**
   * Selects a binding through the engine (B1.3, C0): the engine checks the selection and the family against the loops it lists and answers
   * the open request of the selected loop, which opens as a new session. A refusal is posted to the page with its next action.
   */
  private async selectWithEngine(payload: Record<string, unknown>, post: (payload: Record<string, unknown>) => Promise<void>): Promise<void> {
    this.selecting = true;
    const answer = await engineAnswer(() => this.port.engine.setupDiscover({
      projectRoot: this.port.projectRoot,
      runtimeId: this.loop.runtimeId,
      ...(this.port.languageTool ? { languageTool: this.port.languageTool } : {}),
      selection: payload.selection as SetupDiscoverRequest["selection"],
      family: payload.family as string | undefined,
    }));
    if (!this.current()) return;
    if ("failure" in answer) { await post({ status: "failed", page: payload.page, reason: answer.failure.detail || answer.failure.message }); return; }
    const selected = answer.result.selectedRequest;
    if (!selected) { await post({ status: "failed", page: payload.page, reason: "The engine did not name the selected loop." }); return; }
    // The engine checked the family with the selection above; the page's family is kept as the engine accepted it.
    this.family = typeof payload.family === "string" ? payload.family : undefined;
    if (!this.current()) return;
    await this.open(selected, randomUUID(), this.family);
  }
}

function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function reason(error: unknown): string { return error instanceof Error ? error.message : String(error); }
