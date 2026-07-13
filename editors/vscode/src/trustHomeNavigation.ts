import * as vscode from "vscode";

export async function hasHmiDescriptor(): Promise<boolean> {
  const found = await vscode.workspace.findFiles(
    "**/hmi/*.toml",
    "**/node_modules/**",
    1
  );
  return found.length > 0;
}

// HMI is adaptive: open when a descriptor exists, otherwise scaffold then open.
export async function openOrCreateHmi(): Promise<void> {
  if (await hasHmiDescriptor()) {
    await vscode.commands.executeCommand("trust-lsp.hmi.openPreview");
    return;
  }
  await vscode.commands.executeCommand("trust-lsp.hmi.init");
  await vscode.commands.executeCommand("trust-lsp.hmi.openPreview");
}

export async function newDiagramMenu(): Promise<void> {
  const pick = await vscode.window.showQuickPick(
    [
      { label: "UML Statechart", command: "trust-lsp.statechart.new" },
      { label: "Blockly program", command: "trust-lsp.blockly.new" },
      { label: "Ladder Logic", command: "trust-lsp.ladder.new" },
      { label: "Sequential Function Chart (SFC)", command: "trust-lsp.sfc.new" },
    ],
    { title: "truST — New diagram", placeHolder: "Choose a visual editor" }
  );
  if (pick) {
    await vscode.commands.executeCommand(pick.command);
  }
}
