// Deploy and Roll back from the editor (specification 39, §16.2 item 8): what they come to, as the language server
// answers them (`trust-lsp.hmiDeploy`, which carries out the transaction), and the run target the extension passes it.

/** A problem with a file and its line. */
export interface BundleProblem {
  file?: string;
  line?: number;
  message: string;
}

export type DeployOutcome =
  { ok: true; version: string; target: string } | { ok: false; reason: string; problems?: BundleProblem[] };

/** The run target the extension has selected and started, with its administrator access (its control token). */
export interface DeployTarget {
  endpoint: string;
  token?: string;
  label: string;
  simulator: boolean;
}

export interface DeployDeps {
  /** The selected run target, undefined when none runs. */
  target(): Promise<DeployTarget | undefined>;
}
