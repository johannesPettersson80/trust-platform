// Pure model for the bundled starter examples (§0.5.12) — NO vscode import, so it's unit-testable and the
// manifest shape is guarded standalone. The vscode-facing copy/open flow lives in ../examples.ts.

export type ExampleHardware = "none" | "twincat" | "raspberrypi" | string;

export interface ExampleEntry {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly path: string; // folder name within media/examples
  readonly hardware: ExampleHardware;
  readonly tags?: ReadonlyArray<string>;
}

export interface ExamplePick {
  readonly label: string;
  readonly description: string; // the hardware-requirement badge
  readonly detail: string;
  readonly id: string;
  readonly path: string;
}

// The user-facing hardware-requirement badge shown next to each example.
export function hardwareBadge(hardware: ExampleHardware): string {
  switch (hardware) {
    case "none":
      return "No hardware";
    case "twincat":
      return "Requires TwinCAT";
    case "raspberrypi":
      return "Requires Raspberry Pi";
    default:
      return `Requires ${hardware}`;
  }
}

export function exampleQuickPickItems(
  entries: ReadonlyArray<ExampleEntry>
): ExamplePick[] {
  return entries.map((entry) => ({
    label: entry.title,
    description: hardwareBadge(entry.hardware),
    detail: entry.description,
    id: entry.id,
    path: entry.path,
  }));
}

// Validate a parsed manifest: an array of entries with the required string fields and a folder path.
export function parseManifest(raw: unknown): ExampleEntry[] {
  if (!Array.isArray(raw)) {
    throw new Error("examples manifest must be an array");
  }
  return raw.map((value, index) => {
    if (!value || typeof value !== "object") {
      throw new Error(`examples manifest entry ${index} is not an object`);
    }
    const entry = value as Record<string, unknown>;
    for (const field of ["id", "title", "description", "path", "hardware"]) {
      if (typeof entry[field] !== "string" || !(entry[field] as string).length) {
        throw new Error(`examples manifest entry ${index} missing string '${field}'`);
      }
    }
    return {
      id: entry.id as string,
      title: entry.title as string,
      description: entry.description as string,
      path: entry.path as string,
      hardware: entry.hardware as string,
      tags: Array.isArray(entry.tags) ? (entry.tags as string[]) : undefined,
    };
  });
}
