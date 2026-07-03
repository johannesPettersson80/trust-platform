import * as vscode from "vscode";

import { isRecord } from "./contracts";
import {
  HmiPageSchema,
  HmiSchemaResult,
  LayoutFile,
  LayoutOverrides,
  LayoutWidgetOverride,
} from "./types";

const HMI_LAYOUT_FILE = [".vscode", "trust-hmi-layout.json"] as const;

export async function layoutFileUri(workspaceUri: vscode.Uri): Promise<vscode.Uri> {
  const root = vscode.Uri.joinPath(workspaceUri, HMI_LAYOUT_FILE[0]);
  await vscode.workspace.fs.createDirectory(root);
  return vscode.Uri.joinPath(workspaceUri, ...HMI_LAYOUT_FILE);
}

export async function loadLayoutOverrides(
  workspaceUri: vscode.Uri
): Promise<LayoutOverrides> {
  try {
    const fileUri = await layoutFileUri(workspaceUri);
    const bytes = await vscode.workspace.fs.readFile(fileUri);
    const parsed = JSON.parse(Buffer.from(bytes).toString("utf8")) as LayoutFile;
    if (!parsed || parsed.version !== 1 || !isRecord(parsed.widgets)) {
      return {};
    }
    return normalizeLayoutOverrides(parsed.widgets);
  } catch {
    return {};
  }
}

export async function saveLayoutOverrides(
  workspaceUri: vscode.Uri,
  nextOverrides: LayoutOverrides
): Promise<void> {
  const folderUri = vscode.Uri.joinPath(workspaceUri, HMI_LAYOUT_FILE[0]);
  const fileUri = await layoutFileUri(workspaceUri);
  await vscode.workspace.fs.createDirectory(folderUri);
  const payload: LayoutFile = {
    version: 1,
    widgets: nextOverrides,
    updated_at: new Date().toISOString(),
  };
  const text = `${JSON.stringify(payload, null, 2)}\n`;
  await vscode.workspace.fs.writeFile(fileUri, Buffer.from(text, "utf8"));
}

export function normalizeLayoutOverrides(value: unknown): LayoutOverrides {
  if (!isRecord(value)) {
    return {};
  }
  const result: LayoutOverrides = {};
  for (const [widgetPath, rawOverride] of Object.entries(value)) {
    if (!isRecord(rawOverride)) {
      continue;
    }
    const normalized: LayoutWidgetOverride = {};
    if (typeof rawOverride.label === "string" && rawOverride.label.trim()) {
      normalized.label = rawOverride.label.trim();
    }
    if (typeof rawOverride.page === "string" && rawOverride.page.trim()) {
      normalized.page = rawOverride.page.trim();
    }
    if (typeof rawOverride.group === "string" && rawOverride.group.trim()) {
      normalized.group = rawOverride.group.trim();
    }
    if (typeof rawOverride.widget === "string" && rawOverride.widget.trim()) {
      normalized.widget = rawOverride.widget.trim();
    }
    if (typeof rawOverride.unit === "string" && rawOverride.unit.trim()) {
      normalized.unit = rawOverride.unit.trim();
    }
    if (typeof rawOverride.order === "number" && Number.isFinite(rawOverride.order)) {
      normalized.order = rawOverride.order;
    }
    if (typeof rawOverride.min === "number" && Number.isFinite(rawOverride.min)) {
      normalized.min = rawOverride.min;
    }
    if (typeof rawOverride.max === "number" && Number.isFinite(rawOverride.max)) {
      normalized.max = rawOverride.max;
    }
    if (Object.keys(normalized).length > 0) {
      result[widgetPath] = normalized;
    }
  }
  return result;
}

export function validateLayoutSavePayload(payload: unknown): LayoutOverrides {
  if (!isRecord(payload) || !isRecord(payload.widgets)) {
    throw new Error("payload.widgets must be an object");
  }
  const parsed = normalizeLayoutOverrides(payload.widgets);
  for (const [widgetPath, override] of Object.entries(parsed)) {
    if (!widgetPath.trim()) {
      throw new Error("widget path must not be empty");
    }
    if (override.order !== undefined && !Number.isInteger(override.order)) {
      throw new Error(`order for '${widgetPath}' must be an integer`);
    }
    if (override.page !== undefined && !/^[A-Za-z0-9._-]+$/.test(override.page)) {
      throw new Error(`page for '${widgetPath}' contains unsupported characters`);
    }
  }
  return parsed;
}

export function applyLayoutOverrides(
  schema: HmiSchemaResult,
  localOverrides: LayoutOverrides
): HmiSchemaResult {
  const widgets = schema.widgets.map((widget) => {
    const override = localOverrides[widget.path];
    if (!override) {
      return { ...widget };
    }
    return {
      ...widget,
      label: override.label ?? widget.label,
      page: override.page ?? widget.page,
      group: override.group ?? widget.group,
      order: override.order ?? widget.order,
      widget: override.widget ?? widget.widget,
      unit: override.unit ?? widget.unit,
      min: override.min ?? widget.min,
      max: override.max ?? widget.max,
    };
  });

  widgets.sort((left, right) => {
    if (left.page !== right.page) {
      return left.page.localeCompare(right.page);
    }
    if (left.group !== right.group) {
      return left.group.localeCompare(right.group);
    }
    if (left.order !== right.order) {
      return left.order - right.order;
    }
    return left.label.localeCompare(right.label);
  });

  const pageMap = new Map<string, HmiPageSchema>(
    schema.pages.map((page) => [
      page.id,
      {
        ...page,
        kind: normalizePageKind(page.kind),
      },
    ])
  );
  const maxExistingOrder = schema.pages.reduce(
    (max, page) => Math.max(max, Number.isFinite(page.order) ? page.order : max),
    0
  );
  let nextSyntheticOrder = maxExistingOrder + 10;
  for (const widget of widgets) {
    if (!pageMap.has(widget.page)) {
      pageMap.set(widget.page, {
        id: widget.page,
        title: titleCase(widget.page),
        order: nextSyntheticOrder,
        kind: "dashboard",
        sections: [],
        bindings: [],
        signals: [],
      });
      nextSyntheticOrder += 10;
    }
  }

  const pages = Array.from(pageMap.values()).sort(
    (left, right) => left.order - right.order || left.id.localeCompare(right.id)
  );

  return {
    ...schema,
    pages,
    widgets,
  };
}

function normalizePageKind(value: string | null | undefined): string {
  const kind = typeof value === "string" ? value.trim().toLowerCase() : "";
  if (kind === "process" || kind === "trend" || kind === "alarm") {
    return kind;
  }
  return "dashboard";
}

function titleCase(value: string): string {
  return value
    .split(/[_\-.]+/)
    .filter((part) => part.length > 0)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}
