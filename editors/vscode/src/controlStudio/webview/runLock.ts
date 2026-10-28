export type RunLockControl = HTMLButtonElement | HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;

/**
 * Every control that edits what a running operation reads: all fields, state-design text areas and the dialogs that
 * edit limits, requirements and channels. The running action's own Abort or Cancel control is not listed, so the
 * engineer can always stop the work.
 */
export function runLockedControls(root: HTMLElement): RunLockControl[] {
  return Array.from(root.querySelectorAll<RunLockControl>(
    "input,select,textarea,[data-studio-limits],[data-studio-requirements],[data-engineer-channels],[data-identify-import],[data-studio-reset-chart]"));
}
