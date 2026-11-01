import { openHmiTexts } from "./textFiles";
// The inspector's view of an equipment instance and the change of its choices, one project
// transaction each (specification 39, §21.3). No call in this bridge targets a PLC.
import * as vscode from "vscode";
import type { HostToWebview } from "./messages";
import { applyEquipmentChanges, plantManifest, sameSetting, type EquipmentChanges } from "./equipmentEdits";
export { sameSetting, type EquipmentChanges } from "./equipmentEdits";

/** A setting with its real value, in its unit. */
export interface SettingValue {
  member: string;
  title: string;
  unit?: string | null;
  min?: number | null;
  max?: number | null;
  step?: number | null;
  choices?: Array<{ value: boolean | number | string; title: string }>;
  help?: string | null;
  key?: boolean;
  kind: "bool" | "integer" | "real" | "time" | "text" | "other";
  value: boolean | number | string | null;
  /** Where the value comes from: the design, the declaration, or the block's default. */
  source: "design" | "declaration" | "default";
}

/** A hardware terminal with its connected signal and the signals it may take. */
export interface TerminalState {
  member: string;
  title: string;
  direction: "input" | "output";
  /** The member's declared type, `BOOL` or `LREAL`. */
  type?: string | null;
  required: boolean;
  value?: string | null;
  error?: string | null;
  choices: Array<{ path: string; address?: string; type?: string; direction?: string }>;
  /** What to declare when no PLC signal fits, as the language server words it; absent while one fits. */
  help?: string | null;
}

/** The inspector's hardware summary (§21.3 item 2). */
export type HardwareStatus = "none" | "connected" | "partly" | "not-connected" | "check";

export interface EquipmentDetails {
  revision: number;
  instance: string;
  block: string;
  title: string;
  /** Declared in PLC code. */
  declared: boolean;
  /** Its choices are the builder's: new, or declared by Create PLC code. */
  owned: boolean;
  settings: SettingValue[];
  dependencies: Array<{
    member: string;
    title: string;
    interface: string;
    required: boolean;
    /** It drives an input of the equipment it is connected to: an output, not a measurement. */
    drives?: boolean;
    value?: string | null;
    choices: Array<{ path: string; type: string }>;
  }>;
  terminals: TerminalState[];
  hardware: HardwareStatus;
  bindings: Array<{ path: string; address: string; direction: "input" | "output" | "memory"; type?: string }>;
  /** The picture of the instance's definition, which the project may change when it owns the
   * definition (§21.9). */
  picture?: EquipmentPicture;
}

/** A definition's picture: the symbol it shows and the member driving each of its inputs. */
export interface EquipmentPicture {
  editable: boolean;
  symbol?: string;
  inputs?: Array<{ property: string; member?: string | null; choices: string[] }>;
}

interface Answer extends Partial<EquipmentDetails> {
  ok: boolean;
  errors?: Array<{ message: string }>;
  error?: string;
}

export class EquipmentEditorBridge {
  private selected: string | undefined;
  private sequence = 0;
  private disposed = false;
  private readonly manifests = new Map<number, string | undefined>();
  private timer: ReturnType<typeof setTimeout> | undefined;
  private readonly change: vscode.Disposable;
  constructor(
    private readonly project: vscode.Uri,
    private readonly document: vscode.TextDocument,
    private readonly post: (message: HostToWebview) => void,
  ) {
    this.change = vscode.workspace.onDidChangeTextDocument((event) => {
      if (!this.selected || !event.document.uri.path.startsWith(`${project.path}/`)) return;
      if (!event.document.uri.path.endsWith(".st") && !event.document.uri.path.endsWith("/plant.toml")) return;
      if (this.timer) clearTimeout(this.timer);
      this.timer = setTimeout(() => {
        if (this.selected) void this.inspect(this.selected);
      }, 350);
    });
  }
  dispose(): void {
    this.disposed = true;
    this.change.dispose();
    if (this.timer) clearTimeout(this.timer);
  }
  private manifest(): Promise<string | undefined> {
    return plantManifest(this.project);
  }
  /** `block` names the type of an instance just placed, before its page is saved (§21.3 item 3). */
  async inspect(instance: string, block?: string): Promise<void> {
    this.selected = instance;
    const request = ++this.sequence;
    try {
      const manifest = await this.manifest();
      const answer = await vscode.commands.executeCommand<Answer>("trust-lsp.hmiEquipment", {
        instance,
        root_uri: this.project.toString(),
        open: openHmiTexts(this.project),
        manifest,
        ...(block ? { block } : {}),
      });
      if (this.disposed || request !== this.sequence) return;
      if (!answer?.ok)
        throw new Error(answer?.errors?.[0]?.message ?? answer?.error ?? "Equipment configuration is unavailable.");
      this.manifests.set(request, manifest);
      if (this.manifests.size > 8) this.manifests.delete(this.manifests.keys().next().value!);
      this.post({ type: "equipmentDetails", instance, details: { ...answer, revision: request } as EquipmentDetails });
    } catch (error) {
      if (!this.disposed && request === this.sequence)
        this.post({
          type: "equipmentDetails",
          instance,
          error: error instanceof Error ? error.message : String(error),
        });
    }
  }
  async configure(instance: string, changes: EquipmentChanges, revision: number): Promise<void> {
    const error = await this.applyChanges(instance, changes, revision);
    if (this.disposed) return;
    this.post({ type: "equipmentConfigured", instance, ...(error ? { error } : {}) });
    if (!error) await this.inspect(instance);
  }
  /** Keeps settings that the running PLC has in the project (§21.4 item 6): those that differ from
   * the project's values are applied as one project edit, as a change in the inspector is. */
  async keep(
    instance: string,
    values: Record<string, boolean | number | string | null>,
  ): Promise<{ ok: boolean; kept?: string[]; error?: string }> {
    const request = ++this.sequence;
    try {
      const manifest = await this.manifest();
      const answer = await vscode.commands.executeCommand<Answer>("trust-lsp.hmiEquipment", {
        instance,
        root_uri: this.project.toString(),
        open: openHmiTexts(this.project),
        manifest,
      });
      if (!answer?.ok)
        throw new Error(answer?.errors?.[0]?.message ?? answer?.error ?? "Equipment configuration is unavailable.");
      if (!answer.owned)
        return { ok: false, error: "This equipment is declared in your PLC code: copy the values there." };
      this.manifests.set(request, manifest);
      const settings: Record<string, boolean | number | string> = {};
      for (const setting of answer.settings ?? []) {
        const value = values[setting.member];
        if (value === undefined || value === null || (await sameSetting(setting.value, value))) continue;
        settings[setting.member] = value;
      }
      const kept = Object.keys(settings);
      if (kept.length === 0) return { ok: true, kept };
      const error = await this.applyChanges(instance, { settings }, request);
      if (error) return { ok: false, error };
      if (!this.disposed && this.selected === instance) await this.inspect(instance);
      return { ok: true, kept };
    } catch (error) {
      return { ok: false, error: error instanceof Error ? error.message : String(error) };
    }
  }
  /** Applies changes made against the project as `revision` read it; answers why not, if not. */
  private async applyChanges(
    instance: string,
    changes: EquipmentChanges,
    revision: number,
  ): Promise<string | undefined> {
    try {
      const manifest = await this.manifest();
      if (!this.manifests.has(revision) || this.manifests.get(revision) !== manifest)
        throw new Error(
          "Project settings changed while you were editing. Refresh to review the current values; your draft is still here.",
        );
      await applyEquipmentChanges(this.project, instance, changes, manifest, this.document);
      return undefined;
    } catch (error) {
      return error instanceof Error ? error.message : String(error);
    }
  }
}
