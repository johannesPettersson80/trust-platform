import * as fs from "fs";
import * as path from "path";

/**
 * The crash dumps that VS Code's crash reporter wrote below `folder`, a session's `--crash-reporter-directory`
 * (specification 22, "Acceptance journeys on the packaged extension"): each `.dmp` file at any depth, since Crashpad
 * keeps them in folders of its own, named from `out` with `/`, in order; none when the folder does not exist.
 */
export function crashDumps(folder: string, out: string): string[] {
  if (!fs.existsSync(folder)) {
    return [];
  }
  return fs
    .readdirSync(folder, { recursive: true, encoding: "utf8" })
    .filter((entry) => entry.endsWith(".dmp"))
    .map((entry) => path.relative(out, path.join(folder, entry)).split(path.sep).join("/"))
    .sort();
}

/** A failed session's result: its failure, followed by the crash dumps its VS Code left when there are any. */
export function withCrashDumps(failure: string, dumps: readonly string[]): string {
  return dumps.length ? `${failure}; crash dumps: ${dumps.join(", ")}` : failure;
}
