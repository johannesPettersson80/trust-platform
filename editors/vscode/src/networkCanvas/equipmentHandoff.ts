// Equipment I/O editing uses the same unsaved project transaction as the HMI.
// Protocol channels remain owned by Devices & Connections.
import * as vscode from "vscode";
import { EquipmentEditorBridge, type EquipmentChanges } from "../hmiBuilder/editor/equipmentDetails";

function record(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
export class EquipmentHardwareHandoff {
  private bridge?: EquipmentEditorBridge;
  private instance?: string;
  private page?: vscode.Uri;
  private generation = 0;
  constructor(private readonly post: (message: unknown) => void) {}
  dispose(): void {
    this.generation++;
    this.bridge?.dispose();
    this.bridge = undefined;
    this.instance = undefined;
    this.page = undefined;
  }
  async open(request: unknown): Promise<void> {
    if (!record(request) || !record(request.equipment)) return;
    const focus = request.equipment;
    if (typeof focus.project !== "string" || typeof focus.page !== "string" || typeof focus.instance !== "string")
      return;
    const project = vscode.Uri.parse(focus.project),
      page = vscode.Uri.parse(focus.page);
    const folder = vscode.workspace.getWorkspaceFolder(project);
    if (
      !folder ||
      page.scheme !== project.scheme ||
      page.authority !== project.authority ||
      !page.path.startsWith(`${project.path}/hmi/`) ||
      !page.path.endsWith(".toml")
    ) {
      throw new Error("Select equipment in an HMI page belonging to the open project.");
    }
    this.dispose();
    const generation = this.generation;
    const document = await vscode.workspace.openTextDocument(page);
    if (generation !== this.generation) return;
    this.page = page;
    this.instance = focus.instance;
    this.bridge = new EquipmentEditorBridge(project, document, (message) => {
      if (generation !== this.generation) return;
      if (message.type === "equipmentDetails") this.post({ ...message, type: "equipmentHardwareDetails" });
      if (message.type === "equipmentConfigured") this.post({ ...message, type: "equipmentHardwareConfigured" });
    });
    this.post({ type: "equipmentHardwareFocus", instance: this.instance });
    await this.bridge.inspect(this.instance);
  }
  async handle(message: unknown): Promise<boolean> {
    if (!record(message)) return false;
    if (message.type === "ready") {
      if (this.instance) {
        this.post({ type: "equipmentHardwareFocus", instance: this.instance });
        await this.bridge?.inspect(this.instance);
      }
      return false;
    }
    if (message.type === "equipmentHardwareRefresh") {
      if (this.instance) await this.bridge?.inspect(this.instance);
      return true;
    }
    if (message.type === "equipmentHardwareClose") {
      this.dispose();
      return true;
    }
    if (message.type === "equipmentHardwareReturn") {
      if (this.page) await vscode.commands.executeCommand("vscode.openWith", this.page, "trust-lsp.hmiBuilder");
      return true;
    }
    if (message.type !== "equipmentHardwareApply") return false;
    if (
      !this.bridge ||
      !this.instance ||
      message.instance !== this.instance ||
      typeof message.revision !== "number" ||
      !record(message.signals)
    )
      return true;
    const signals = message.signals;
    if (Object.keys(signals).length > 64 || Object.values(signals).some((value) => typeof value !== "string"))
      return true;
    await this.bridge.configure(
      this.instance,
      { signals: signals as NonNullable<EquipmentChanges["signals"]> },
      message.revision,
    );
    return true;
  }
}
